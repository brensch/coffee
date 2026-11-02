//! `coffee-emu serve`: a live, real-time (or faster) emulated machine with a lab
//! API for the UI. The engine connects over TCP exactly like it would to a USB
//! hardware bridge; unplugging it leaves the machine running unpowered-control.

use crate::link::EngineLink;
use crate::world::Sim;
use anyhow::{Context, Result};
use axum::extract::State;
use axum::extract::ws::{Message as WsMsg, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use coffee_api::Ack;
use coffee_api::lab::{KnobInfo, LabCommand, LabFrame, LabInfo, SignalInfo};
use coffee_physics::{Actuators, Machine, ModelConfig};
use coffee_scenario::{Outcome, Runner, Value, World};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

#[derive(Clone, Debug)]
pub struct LabOpts {
    pub model: PathBuf,
    pub engine_listen: String,
    pub http: String,
    pub speed: f32,
    pub seed: u64,
    pub scenarios_dir: PathBuf,
    pub spawn_engine: Option<PathBuf>,
    pub engine_http: String,
    pub ui: PathBuf,
    /// Hardware-in-the-loop: an ESP32 in bridge mode on this serial port.
    pub engine_serial: Option<String>,
}

struct ActiveScenario {
    runner: Runner,
    t0: f64,
    duration: f64,
    events_sent: usize,
}

struct Lab {
    sim: Sim,
    link: Option<EngineLink>,
    paused: bool,
    speed: f32,
    scenario: Option<ActiveScenario>,
    base_cfg: ModelConfig,
    reanchor: bool,
}

struct LabShared {
    lab: Mutex<Lab>,
    tx: broadcast::Sender<String>,
    info: LabInfo,
    opts: LabOpts,
}

type LabApp = Arc<LabShared>;

impl LabShared {
    fn send(&self, f: &LabFrame) {
        if let Ok(s) = serde_json::to_string(f) {
            let _ = self.tx.send(s);
        }
    }
}

fn list_scenarios(dir: &PathBuf) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension().is_some_and(|x| x == "scn"))
                        .then(|| p.file_stem().unwrap().to_string_lossy().to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub fn serve(opts: LabOpts) -> Result<()> {
    let cfg = ModelConfig::load(&opts.model)?;
    let signals: Vec<SignalInfo> = Sim::all_signals()
        .iter()
        .map(|(n, u)| SignalInfo {
            name: n.to_string(),
            unit: u.to_string(),
        })
        .collect();
    let knobs = Machine::KNOBS
        .iter()
        .map(|(p, h)| KnobInfo {
            path: p.to_string(),
            hint: h.to_string(),
        })
        .collect();
    let info = LabInfo {
        model_name: cfg.meta.name.clone(),
        signals,
        knobs,
        params: cfg.numeric_paths(),
        scenarios: list_scenarios(&opts.scenarios_dir),
    };
    let (tx, _) = broadcast::channel(256);
    let app: LabApp = Arc::new(LabShared {
        lab: Mutex::new(Lab {
            sim: Sim::new(Machine::new(cfg.clone(), opts.seed)),
            link: None,
            paused: false,
            speed: opts.speed,
            scenario: None,
            base_cfg: cfg,
            reanchor: true,
        }),
        tx,
        info,
        opts: opts.clone(),
    });

    let listener = std::net::TcpListener::bind(&opts.engine_listen)
        .with_context(|| format!("binding engine port {}", opts.engine_listen))?;
    listener.set_nonblocking(true)?;
    eprintln!(
        "[emu] waiting for an engine on tcp://{}",
        opts.engine_listen
    );

    let mut child = None;
    if let Some(engine) = &opts.spawn_engine {
        let c = std::process::Command::new(engine)
            .arg("--emu")
            .arg(format!("tcp://{}", opts.engine_listen))
            .arg("--http")
            .arg(&opts.engine_http)
            .arg("--lab-url")
            .arg(format!(
                "http://{}",
                opts.http.replace("0.0.0.0", "127.0.0.1")
            ))
            .arg("--data")
            .arg("data")
            .arg("--ui")
            .arg(&opts.ui)
            .spawn()
            .with_context(|| format!("spawning {}", engine.display()))?;
        eprintln!("[emu] spawned engine; open http://{}", opts.engine_http);
        child = Some(c);
    }

    let sim_app = app.clone();
    std::thread::Builder::new()
        .name("sim".into())
        .spawn(move || sim_loop(sim_app, listener))?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let res = rt.block_on(async move {
        let router = Router::new()
            .route("/lab/info", get(info_handler))
            .route("/lab/ws", get(ws))
            .route("/lab/command", post(command))
            .with_state(app.clone())
            .layer(CorsLayer::permissive());
        let l = tokio::net::TcpListener::bind(&opts.http).await?;
        eprintln!("[emu] lab API on http://{}", opts.http);
        tokio::select! {
            r = axum::serve(l, router) => r.map_err(anyhow::Error::from),
            _ = tokio::signal::ctrl_c() => Ok(()),
        }
    });
    if let Some(mut c) = child {
        let _ = c.kill();
    }
    res
}

fn step_once(app: &LabShared, lab: &mut Lab) {
    let s = lab.sim.m.sensor_frame();
    let act = match lab.link.as_mut().map(|l| l.exchange(s)) {
        Some(Ok(a)) => {
            lab.sim.status = a.status;
            Actuators {
                heater: a.heater,
                pump: a.pump,
                valve: a.valve,
            }
        }
        Some(Err(e)) => {
            eprintln!("[emu] engine link lost: {e:#}");
            lab.link = None;
            lab.sim.engine_connected = false;
            app.send(&LabFrame::Event {
                t: lab.sim.m.t,
                text: "engine disconnected".into(),
            });
            Actuators::default()
        }
        None => Actuators::default(),
    };
    lab.sim.m.step_half_cycle(act);

    let t = lab.sim.m.t;
    let mut finished = None;
    if let Some(sc) = lab.scenario.as_mut() {
        let rel = t - sc.t0;
        if let Err(e) = sc.runner.step(rel, &mut lab.sim) {
            app.send(&LabFrame::Event {
                t,
                text: format!("scenario error: {e:#}"),
            });
            finished = Some(true);
        }
        for (et, ev) in sc.runner.events.iter().skip(sc.events_sent) {
            app.send(&LabFrame::Event {
                t: sc.t0 + et,
                text: ev.clone(),
            });
        }
        sc.events_sent = sc.runner.events.len();
        if sc.runner.ended || rel >= sc.duration {
            finished = Some(false);
        }
    }
    if let Some(errored) = finished {
        let sc = lab.scenario.take().unwrap();
        let rel = t - sc.t0;
        let rep = sc.runner.finish(rel);
        let lines = rep
            .expects
            .iter()
            .map(|e| match &e.outcome {
                Outcome::Pass => format!("ok   {}", e.source),
                Outcome::Fail(w) => format!("FAIL {}  —  {w}", e.source),
                Outcome::Pending => format!("??   {}", e.source),
            })
            .collect();
        app.send(&LabFrame::ScenarioDone {
            name: rep.name.clone(),
            passed: rep.passed() && !errored,
            lines,
        });
    }
}

fn sim_loop(app: LabApp, listener: std::net::TcpListener) {
    let mut last_serial_try = Instant::now() - Duration::from_secs(10);
    let mut wall0 = Instant::now();
    let mut sim0 = 0.0;
    let mut last_frame = Instant::now();
    loop {
        {
            let mut lab = app.lab.lock().unwrap();
            if lab.link.is_none()
                && last_serial_try.elapsed() > Duration::from_secs(2)
                && let Some(path) = app.opts.engine_serial.clone()
            {
                last_serial_try = Instant::now();
                match EngineLink::from_serial(&path) {
                    Ok(l) => {
                        eprintln!("[emu] hardware engine connected on {path}");
                        lab.link = Some(l);
                        lab.sim.engine_connected = true;
                        app.send(&LabFrame::Event {
                            t: lab.sim.m.t,
                            text: format!("ESP32 bridge connected on {path}"),
                        });
                    }
                    Err(e) => eprintln!("[emu] serial engine not ready: {e:#}"),
                }
            }
            if lab.link.is_none()
                && let Ok((s, addr)) = listener.accept()
            {
                let _ = s.set_nonblocking(false);
                match EngineLink::from_tcp(s) {
                    Ok(l) => {
                        eprintln!("[emu] engine connected from {addr}");
                        lab.link = Some(l);
                        lab.sim.engine_connected = true;
                        app.send(&LabFrame::Event {
                            t: lab.sim.m.t,
                            text: "engine connected".into(),
                        });
                    }
                    Err(e) => eprintln!("[emu] engine handshake failed: {e:#}"),
                }
            }
            if lab.reanchor || lab.paused {
                wall0 = Instant::now();
                sim0 = lab.sim.m.t;
                lab.reanchor = false;
            }
            if !lab.paused {
                let target = sim0 + wall0.elapsed().as_secs_f64() * lab.speed as f64;
                let mut n = 0;
                while lab.sim.m.t < target && n < 4000 {
                    step_once(&app, &mut lab);
                    n += 1;
                }
                if n >= 4000 {
                    // Can't keep up at this speed: run as fast as possible instead.
                    wall0 = Instant::now();
                    sim0 = lab.sim.m.t;
                }
            }
            if last_frame.elapsed() >= Duration::from_millis(40) && app.tx.receiver_count() > 0 {
                last_frame = Instant::now();
                let values = app
                    .info
                    .signals
                    .iter()
                    .map(|s| lab.sim.get(&s.name).filter(|v| v.is_finite()))
                    .collect();
                app.send(&LabFrame::Truth {
                    t: lab.sim.m.t,
                    speed: lab.speed,
                    paused: lab.paused,
                    engine_connected: lab.link.is_some(),
                    values,
                });
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

async fn info_handler(State(app): State<LabApp>) -> Json<LabInfo> {
    let mut i = app.info.clone();
    i.scenarios = list_scenarios(&app.opts.scenarios_dir);
    Json(i)
}

fn parse_value(s: &str) -> Value {
    match s.trim().parse::<f64>() {
        Ok(n) => Value::Num(n),
        Err(_) => Value::Word(s.trim().to_string()),
    }
}

fn start_scenario(lab: &mut Lab, text: &str) -> Result<String> {
    let sc = coffee_scenario::parse(text)?;
    for (p, v) in &sc.model {
        lab.sim.m.cfg.set_path(p, *v)?;
    }
    let name = sc.name.clone();
    let duration = sc.duration_s;
    let runner = Runner::new(sc, &lab.sim)?;
    lab.scenario = Some(ActiveScenario {
        runner,
        t0: lab.sim.m.t,
        duration,
        events_sent: 0,
    });
    Ok(name)
}

async fn command(State(app): State<LabApp>, Json(cmd): Json<LabCommand>) -> Json<Ack> {
    let mut lab = app.lab.lock().unwrap();
    let t = lab.sim.m.t;
    let r: Result<Option<String>> = match cmd {
        LabCommand::Set { path, value } => {
            let v = parse_value(&value);
            lab.sim
                .set(&path, &v)
                .map(|_| Some(format!("set {path} = {value}")))
        }
        LabCommand::Pause => {
            lab.paused = true;
            Ok(Some("paused".into()))
        }
        LabCommand::Resume => {
            lab.paused = false;
            lab.reanchor = true;
            Ok(Some("resumed".into()))
        }
        LabCommand::Speed { x } => {
            lab.speed = x.clamp(0.05, 1000.0);
            lab.reanchor = true;
            Ok(Some(format!("speed {x}x")))
        }
        LabCommand::RunScenario { text } => {
            start_scenario(&mut lab, &text).map(|n| Some(format!("scenario started: {n}")))
        }
        LabCommand::RunNamedScenario { name } => {
            let p = app.opts.scenarios_dir.join(format!("{name}.scn"));
            std::fs::read_to_string(&p)
                .with_context(|| format!("reading {}", p.display()))
                .and_then(|text| start_scenario(&mut lab, &text))
                .map(|n| Some(format!("scenario started: {n}")))
        }
        LabCommand::Reset => {
            let cfg = lab.base_cfg.clone();
            lab.sim = Sim::new(Machine::new(cfg, app.opts.seed));
            lab.scenario = None;
            // Dropping the link makes the engine reconnect with a fresh state.
            lab.link = None;
            lab.reanchor = true;
            Ok(Some("machine reset".into()))
        }
    };
    match r {
        Ok(Some(text)) => {
            app.send(&LabFrame::Event { t, text });
            Json(Ack::ok())
        }
        Ok(None) => Json(Ack::ok()),
        Err(e) => Json(Ack::err(format!("{e:#}"))),
    }
}

async fn ws(State(app): State<LabApp>, up: WebSocketUpgrade) -> Response {
    up.on_upgrade(move |s| ws_session(app, s))
}

async fn ws_session(app: LabApp, mut sock: WebSocket) {
    let mut rx = app.tx.subscribe();
    loop {
        tokio::select! {
            m = rx.recv() => match m {
                Ok(s) => if sock.send(WsMsg::Text(s.into())).await.is_err() { return },
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return,
            },
            inc = sock.recv() => match inc {
                Some(Ok(WsMsg::Close(_))) | None | Some(Err(_)) => return,
                _ => {}
            }
        }
    }
}
