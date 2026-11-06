//! `coffee-emu`: the emulator suite.
//!
//! ```text
//! coffee-emu run scenarios/shot_classic.scn --trace out/shot.csv
//! coffee-emu test scenarios/
//! coffee-emu serve --spawn-engine            # live lab + UI on :8080
//! coffee-emu fit --log real.csv --params thermal.boiler_mass_kg,thermal.loss_boiler_w_k
//! coffee-emu signals                          # what you can read/set
//! ```

mod fit;
mod link;
mod run;
mod serve;
mod world;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use coffee_physics::{Machine, ModelConfig};
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Espresso machine emulator suite")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run one scenario against a freshly spawned engine.
    Run {
        scenario: PathBuf,
        /// Engine binary (default: coffee-engine next to this executable).
        #[arg(long)]
        engine: Option<PathBuf>,
        /// Write a CSV trace of the run.
        #[arg(long)]
        trace: Option<PathBuf>,
        /// Model used when the scenario has no `machine` line.
        #[arg(long, default_value = "models/gaggia_classic_pro_e24_120v.toml")]
        model: PathBuf,
        /// Print the event timeline.
        #[arg(long)]
        events: bool,
        /// Show the engine's stderr.
        #[arg(long)]
        verbose: bool,
    },
    /// Run every scenario in the given files/directories. Exit code 1 on failure.
    Test {
        #[arg(default_value = "scenarios")]
        paths: Vec<PathBuf>,
        #[arg(long)]
        engine: Option<PathBuf>,
        /// Directory for per-scenario CSV traces.
        #[arg(long, default_value = "out/traces")]
        traces: PathBuf,
        #[arg(long, default_value = "models/gaggia_classic_pro_e24_120v.toml")]
        model: PathBuf,
    },
    /// Live emulated machine with the lab API for the UI.
    Serve {
        #[arg(long, default_value = "models/gaggia_classic_pro_e24_120v.toml")]
        model: PathBuf,
        /// Where the engine connects (`coffee-engine --emu tcp://...`).
        #[arg(long, default_value = "127.0.0.1:7878")]
        engine_listen: String,
        /// Lab API listen address.
        #[arg(long, default_value = "127.0.0.1:8081")]
        http: String,
        /// Simulation speed relative to wall clock.
        #[arg(long, default_value_t = 1.0)]
        speed: f32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value = "scenarios")]
        scenarios: PathBuf,
        /// Also start the engine (path optional; defaults to the sibling binary).
        #[arg(long, num_args = 0..=1, default_missing_value = "auto")]
        spawn_engine: Option<PathBuf>,
        /// UI/API address for a spawned engine.
        #[arg(long, default_value = "127.0.0.1:8080")]
        engine_http: String,
        #[arg(long, default_value = "ui/dist")]
        ui: PathBuf,
        /// Hardware-in-the-loop: ESP32 running `--features bridge` firmware on this port.
        #[arg(long)]
        engine_serial: Option<String>,
    },
    /// Replay a log open-loop and write simulated vs logged sensors.
    Replay {
        #[arg(long)]
        log: PathBuf,
        #[arg(long, default_value = "models/gaggia_classic_pro_e24_120v.toml")]
        model: PathBuf,
        #[arg(long, default_value = "out/replay.csv")]
        out: PathBuf,
    },
    /// Fit model parameters to a log.
    Fit {
        #[arg(long)]
        log: PathBuf,
        #[arg(long, default_value = "models/gaggia_classic_pro_e24_120v.toml")]
        model: PathBuf,
        /// Comma-separated model parameter paths.
        #[arg(long, value_delimiter = ',')]
        params: Vec<String>,
        /// Signal weights: temperature, pressure, weight.
        #[arg(long, default_value_t = 1.0)]
        w_temp: f64,
        #[arg(long, default_value_t = 1.0)]
        w_pressure: f64,
        #[arg(long, default_value_t = 0.0)]
        w_weight: f64,
        #[arg(long, default_value_t = 80)]
        iters: usize,
        /// Write the fitted model here.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Inspect a `.clog`, optionally exporting raw decoded columns as CSV.
    Log {
        file: PathBuf,
        #[arg(long)]
        csv: Option<PathBuf>,
    },
    /// Record the firmware's USB log stream to a `.clog` (Ctrl-C to stop).
    Capture {
        #[arg(long)]
        port: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Convert Decent Espresso `.tcl` profiles (espresso ones only) into coffee profiles.
    ImportDecent {
        files: Vec<PathBuf>,
        #[arg(long, default_value = "profiles/community/decent")]
        out: PathBuf,
    },
    /// Convert GaggiMate profile JSON files into coffee profiles.
    ImportGaggimate {
        files: Vec<PathBuf>,
        /// Output directory for converted profiles.
        #[arg(long, default_value = "profiles/imported")]
        out: PathBuf,
    },
    /// List readable signals, settable knobs and model parameters.
    Signals {
        #[arg(long, default_value = "models/gaggia_classic_pro_e24_120v.toml")]
        model: PathBuf,
    },
}

fn engine_or_default(e: Option<PathBuf>) -> PathBuf {
    match e {
        Some(p) if !p.as_os_str().is_empty() && p.as_os_str() != "auto" => p,
        _ => link::default_engine_path(),
    }
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Run {
            scenario,
            engine,
            trace,
            model,
            events,
            verbose,
        } => {
            let opts = run::RunOpts {
                engine: engine_or_default(engine),
                trace,
                default_model: model,
                verbose_engine: verbose,
            };
            let rep = run::run_file(&scenario, &opts)?;
            run::print_report(&scenario, &rep, events);
            if !rep.passed() {
                std::process::exit(1);
            }
        }
        Cmd::Test {
            paths,
            engine,
            traces,
            model,
        } => {
            let files = run::collect_scenarios(&paths)?;
            if files.is_empty() {
                bail!("no .scn files found");
            }
            let engine = engine_or_default(engine);
            let mut failed = 0;
            let t0 = std::time::Instant::now();
            for f in &files {
                let stem = f.file_stem().unwrap().to_string_lossy().to_string();
                let opts = run::RunOpts {
                    engine: engine.clone(),
                    trace: Some(traces.join(format!("{stem}.csv"))),
                    default_model: model.clone(),
                    verbose_engine: false,
                };
                match run::run_file(f, &opts) {
                    Ok(rep) => {
                        run::print_report(f, &rep, false);
                        if !rep.passed() {
                            failed += 1;
                        }
                    }
                    Err(e) => {
                        println!("ERROR {}: {e:#}", f.display());
                        failed += 1;
                    }
                }
            }
            println!(
                "\n{} scenarios, {} passed, {} failed in {:.1}s (traces in {})",
                files.len(),
                files.len() - failed,
                failed,
                t0.elapsed().as_secs_f64(),
                traces.display()
            );
            if failed > 0 {
                std::process::exit(1);
            }
        }
        Cmd::Serve {
            model,
            engine_listen,
            http,
            speed,
            seed,
            scenarios,
            spawn_engine,
            engine_http,
            ui,
            engine_serial,
        } => {
            serve::serve(serve::LabOpts {
                model,
                engine_listen,
                http,
                speed,
                seed,
                scenarios_dir: scenarios,
                spawn_engine: spawn_engine.map(|p| engine_or_default(Some(p))),
                engine_http,
                ui,
                engine_serial,
            })?;
        }
        Cmd::Replay { log, model, out } => {
            let cfg = ModelConfig::load(&model)?;
            let dec = fit::Decoder::new(&cfg);
            let rows = fit::load_log(&log, &dec)?;
            let sim = fit::simulate(&cfg, &dec, &rows, 1);
            let (cost, e) = fit::score(
                &sim,
                &rows,
                fit::Weights {
                    temp: 1.0,
                    pressure: 1.0,
                    weight: 1.0,
                },
            );
            if let Some(d) = out.parent() {
                std::fs::create_dir_all(d).ok();
            }
            let mut f = std::io::BufWriter::new(std::fs::File::create(&out)?);
            writeln!(f, "t_s,temp_log,temp_sim,p_log,p_sim,scale_log,scale_sim")?;
            let o = |x: Option<f64>| x.map(|v| format!("{v:.3}")).unwrap_or_default();
            for (i, r) in rows.iter().enumerate() {
                writeln!(
                    f,
                    "{:.3},{},{},{},{},{},{}",
                    r.t,
                    o(r.tc),
                    o(sim.tc[i]),
                    o(r.p),
                    o(sim.p[i]),
                    o(r.scale),
                    o(sim.scale[i])
                )?;
            }
            println!("replayed {} ticks -> {}", rows.len(), out.display());
            println!(
                "RMSE  temp {:?} C   pressure {:?} bar   weight {:?} g   (cost {cost:.3})",
                e[0], e[1], e[2]
            );
        }
        Cmd::Fit {
            log,
            model,
            params,
            w_temp,
            w_pressure,
            w_weight,
            iters,
            out,
        } => {
            if params.is_empty() {
                bail!(
                    "--params is required, e.g. --params thermal.boiler_mass_kg,thermal.loss_boiler_w_k"
                );
            }
            let cfg = ModelConfig::load(&model)?;
            let rows = fit::load_log(&log, &fit::Decoder::new(&cfg))?;
            let w = fit::Weights {
                temp: w_temp,
                pressure: w_pressure,
                weight: w_weight,
            };
            eprintln!(
                "fitting {} params over {} ticks ({:.0}s of log)",
                params.len(),
                rows.len(),
                rows.last().unwrap().t
            );
            let r = fit::fit(&cfg, &rows, &params, w, iters, 1, true)?;
            println!("cost {:.4} -> {:.4}", r.before.0, r.after.0);
            println!(
                "RMSE before: temp {:?} pressure {:?} weight {:?}",
                r.before.1[0], r.before.1[1], r.before.1[2]
            );
            println!(
                "RMSE after:  temp {:?} pressure {:?} weight {:?}",
                r.after.1[0], r.after.1[1], r.after.1[2]
            );
            for (p, a, b) in &r.params {
                println!("  {p:40} {a:>10.4} -> {b:>10.4}");
            }
            if let Some(o) = out {
                std::fs::write(&o, r.cfg.to_toml_string())?;
                println!("wrote {}", o.display());
            }
        }
        Cmd::Log { file, csv } => {
            let data = std::fs::read(&file)?;
            let frames = coffee_node::tlog::read(&data);
            let (Some(first), Some(last)) = (frames.first(), frames.last()) else {
                bail!("no log chunks in {}", file.display())
            };
            let secs = (last.0.t_us - first.0.t_us) as f64 * 1e-6;
            println!(
                "{}: {} ticks, {:.1} s, {} bytes ({:.2} B/tick, {:.0} B/s)",
                file.display(),
                frames.len(),
                secs,
                data.len(),
                data.len() as f64 / frames.len() as f64,
                data.len() as f64 / secs.max(1e-9)
            );
            if let Some(out) = csv {
                let mut f = std::io::BufWriter::new(std::fs::File::create(&out)?);
                writeln!(
                    f,
                    "t_s,cycle,heater,pump,valve,inputs,ntc_mv,pressure_mv,scale_mg"
                )?;
                for (s, o) in &frames {
                    writeln!(
                        f,
                        "{:.4},{},{},{},{},{},{},{},{}",
                        (s.t_us - first.0.t_us) as f64 * 1e-6,
                        s.cycle,
                        o.heater as u8,
                        o.pump as u8,
                        o.valve as u8,
                        s.inputs,
                        s.ntc_mv,
                        s.pressure_mv,
                        s.scale_mg
                    )?;
                }
                println!("wrote {}", out.display());
            }
        }
        Cmd::Capture { port, out } => {
            use std::io::Read;
            let mut p = serialport::new(&port, 921_600)
                .timeout(std::time::Duration::from_secs(3600))
                .open()?;
            let mut f = std::fs::File::create(&out)?;
            let mut buf = [0u8; 4096];
            let mut total = 0usize;
            eprintln!("capturing {port} -> {} (Ctrl-C to stop)", out.display());
            loop {
                let n = p.read(&mut buf)?;
                f.write_all(&buf[..n])?;
                total += n;
                if total % (64 * 1024) < n {
                    eprintln!("  {} KiB", total / 1024);
                }
            }
        }
        Cmd::ImportDecent { files, out } => {
            std::fs::create_dir_all(&out)?;
            let (mut ok, mut skipped) = (0, 0);
            for f in &files {
                let txt = std::fs::read_to_string(f)?;
                let bev = coffee_api::decent::beverage_type(&txt);
                // Decent labels its steaming profile "espresso"; it isn't one.
                if bev != "espresso" || txt.contains("profile_title {Steam only}") {
                    skipped += 1;
                    continue;
                }
                match coffee_api::decent::convert(&txt) {
                    Ok(imp) => {
                        let slug: String = imp
                            .profile
                            .name
                            .chars()
                            .map(|c| {
                                if c.is_ascii_alphanumeric() {
                                    c.to_ascii_lowercase()
                                } else {
                                    '_'
                                }
                            })
                            .collect();
                        let dest = out.join(format!("{slug}.json"));
                        std::fs::write(&dest, serde_json::to_vec_pretty(&imp.profile)?)?;
                        println!(
                            "ok    {:44} {} phases{}",
                            imp.profile.name,
                            imp.profile.phases.len(),
                            if imp.warnings.is_empty() {
                                String::new()
                            } else {
                                format!("  ({})", imp.warnings.join("; "))
                            }
                        );
                        ok += 1;
                    }
                    Err(e) => println!("FAIL  {}: {e}", f.display()),
                }
            }
            println!(
                "{ok} converted, {skipped} non-espresso skipped. Decent profiles are GPL-3.0 (decentespresso/de1app)."
            );
        }
        Cmd::ImportGaggimate { files, out } => {
            std::fs::create_dir_all(&out)?;
            let mut ok = 0;
            for f in &files {
                let res = std::fs::read_to_string(f)
                    .map_err(anyhow::Error::from)
                    .and_then(|s| {
                        serde_json::from_str::<coffee_api::gaggimate::GmProfile>(&s)
                            .map_err(anyhow::Error::from)
                    })
                    .and_then(|gm| coffee_api::gaggimate::convert(&gm).map_err(anyhow::Error::msg));
                match res {
                    Ok(imp) => {
                        let stem = f.file_stem().unwrap().to_string_lossy();
                        let dest = out.join(format!("gm_{stem}.json"));
                        std::fs::write(&dest, serde_json::to_vec_pretty(&imp.profile)?)?;
                        println!(
                            "ok    {} -> {} ({} phases)",
                            f.display(),
                            dest.display(),
                            imp.profile.phases.len()
                        );
                        for w in imp.warnings {
                            println!("      note: {w}");
                        }
                        ok += 1;
                    }
                    Err(e) => println!("FAIL  {}: {e:#}", f.display()),
                }
            }
            println!(
                "{ok}/{} converted. GaggiMate profiles are CC BY-NC-SA 4.0; keep that license on shared copies.",
                files.len()
            );
        }
        Cmd::Signals { model } => {
            let cfg = ModelConfig::load(&model)?;
            println!("# Signals (read in conditions, traces, UI)");
            for (n, u) in world::Sim::all_signals() {
                println!("  {n:28} {u}");
            }
            println!("\n# Knobs and faults (`set path = value`)");
            for (p, h) in Machine::KNOBS {
                println!("  {p:28} {h}");
            }
            println!("\n# Model parameters (`model path = x`, `set path = x`, fit --params)");
            for p in cfg.numeric_paths() {
                println!(
                    "  {p:40} {}",
                    cfg.get_path(&p).map(|v| format!("{v}")).unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}
