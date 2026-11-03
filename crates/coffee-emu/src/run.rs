//! Batch scenario runs: as fast as the engine can answer.

use crate::link::EngineLink;
use crate::world::Sim;
use anyhow::{Context, Result};
use coffee_physics::{Actuators, Machine, ModelConfig};
use coffee_scenario::{Outcome, Report, Runner, World};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const DEFAULT_TRACE: &[&str] = &[
    "time",
    "boiler.temp",
    "boiler.water_temp",
    "probe.temp",
    "brew.water_temp",
    "portafilter.temp",
    "hydraulics.pressure",
    "pump.flow",
    "puck.flow",
    "cup.mass",
    "heater.power",
    "engine.state",
    "engine.faults",
    "engine.temp",
    "engine.setpoint",
    "engine.pressure",
    "engine.target_pressure",
    "engine.flow",
    "engine.weight",
    "engine.phase",
];

pub struct RunOpts {
    pub engine: PathBuf,
    pub trace: Option<PathBuf>,
    pub default_model: PathBuf,
    pub verbose_engine: bool,
}

pub fn load_model(
    scenario_path: &Path,
    machine: Option<&str>,
    default_model: &Path,
) -> Result<ModelConfig> {
    let path = match machine {
        Some(m) => {
            let rel = scenario_path.parent().unwrap_or(Path::new(".")).join(m);
            if rel.exists() { rel } else { PathBuf::from(m) }
        }
        None => default_model.to_path_buf(),
    };
    ModelConfig::load(&path)
}

pub fn run_file(path: &Path, opts: &RunOpts) -> Result<Report> {
    let src =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let sc = coffee_scenario::parse(&src).with_context(|| format!("in {}", path.display()))?;
    let mut cfg = load_model(path, sc.machine.as_deref(), &opts.default_model)?;
    for (p, v) in &sc.model {
        cfg.set_path(p, *v)
            .with_context(|| format!("model override `{p}`"))?;
    }
    let mut sim = Sim::new(Machine::new(cfg, sc.seed));
    let trace_cols: Vec<String> = if sc.trace.is_empty() {
        DEFAULT_TRACE.iter().map(|s| s.to_string()).collect()
    } else {
        sc.trace.clone()
    };
    let trace_every = sc.trace_every_s;
    let duration = sc.duration_s;
    let engine_args = sc.engine_args.clone();
    let mut runner = Runner::new(sc, &sim)?;

    let mut trace = match &opts.trace {
        Some(p) => {
            if let Some(d) = p.parent() {
                std::fs::create_dir_all(d).ok();
            }
            let mut f = std::io::BufWriter::new(std::fs::File::create(p)?);
            writeln!(f, "{}", trace_cols.join(","))?;
            Some(f)
        }
        None => None,
    };

    let mut link = EngineLink::spawn(
        &opts.engine,
        &engine_args,
        !opts.verbose_engine,
        sim.m.cfg.mains.frequency_hz.round() as u8,
    )?;
    sim.engine_connected = true;
    let mut next_trace = 0.0;
    loop {
        let s = sim.m.sensor_frame();
        let a = link.exchange(s)?;
        sim.status = a.status;
        sim.m.step_cycle(Actuators {
            heater: a.heater,
            pump: a.pump,
            valve: a.valve,
        });
        let t = sim.m.t;
        runner.step(t, &mut sim)?;
        if let Some(f) = trace.as_mut()
            && t >= next_trace
        {
            next_trace = t + trace_every;
            let row: Vec<String> = trace_cols
                .iter()
                .map(|c| match sim.get(c) {
                    Some(v) if v.is_finite() => format!("{v:.4}"),
                    _ => String::new(),
                })
                .collect();
            writeln!(f, "{}", row.join(","))?;
        }
        if runner.ended || t >= duration {
            break;
        }
    }
    link.close();
    let t = sim.m.t;
    Ok(runner.finish(t))
}

pub fn print_report(path: &Path, r: &Report, show_events: bool) {
    let mark = if r.passed() { "PASS" } else { "FAIL" };
    let name = if r.name.is_empty() {
        path.display().to_string()
    } else {
        r.name.clone()
    };
    println!("{mark}  {name}  ({:.0}s simulated)", r.sim_time_s);
    for e in &r.expects {
        match &e.outcome {
            Outcome::Pass => println!("   ok    L{:<3} {}", e.line, e.source),
            Outcome::Fail(why) => {
                println!("   FAIL  L{:<3} {}\n           {why}", e.line, e.source)
            }
            Outcome::Pending => println!("   ??    L{:<3} {}", e.line, e.source),
        }
    }
    if show_events {
        for (t, ev) in &r.events {
            println!("   {t:8.2}s  {ev}");
        }
    }
}

pub fn collect_scenarios(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = vec![];
    for p in paths {
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p)?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "scn"))
                .collect();
            v.sort();
            out.extend(v);
        } else {
            out.push(p.clone());
        }
    }
    Ok(out)
}
