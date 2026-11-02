//! The lockstep link: read a SensorFrame, tick the node, write an ActuatorFrame.

use crate::App;
use anyhow::{Context, Result, bail};
use coffee_api::ServerMsg;
use coffee_io::wire::{self, MAX_FRAME, Message, PROTOCOL_VERSION};
use std::io::{BufReader, BufWriter, Read, Write};
use std::time::{Duration, Instant};

pub fn run(app: App) -> Result<()> {
    let spec = app.args.emu.clone();
    if spec == "stdio" {
        let stdin = std::io::stdin().lock();
        let stdout = std::io::stdout().lock();
        return session(&app, BufReader::new(stdin), BufWriter::new(stdout));
    }
    let addr = spec
        .strip_prefix("tcp://")
        .context("--emu must be `stdio` or `tcp://host:port`")?
        .to_string();
    let mut announced = false;
    loop {
        match std::net::TcpStream::connect(&addr) {
            Ok(s) => {
                s.set_nodelay(true)?;
                eprintln!("[engine] connected to emulator at {addr}");
                announced = false;
                let r = BufReader::new(s.try_clone()?);
                if let Err(e) = session(&app, r, BufWriter::new(s)) {
                    eprintln!("[engine] link closed: {e:#}");
                }
            }
            Err(e) => {
                if !announced {
                    eprintln!("[engine] waiting for emulator at {addr} ({e})");
                    announced = true;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn read_msg(r: &mut impl Read) -> Result<Option<Message>> {
    let mut buf = [0u8; MAX_FRAME];
    match r.read_exact(&mut buf[..3]) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    if len > MAX_FRAME - 3 {
        bail!("frame too long ({len})");
    }
    r.read_exact(&mut buf[3..3 + len])?;
    let (m, _) = wire::decode(&buf[..3 + len]).map_err(|e| anyhow::anyhow!("decode: {e:?}"))?;
    Ok(Some(m))
}

fn write_msg(w: &mut impl Write, m: &Message) -> Result<()> {
    let mut buf = [0u8; MAX_FRAME];
    let n = wire::encode(m, &mut buf);
    w.write_all(&buf[..n])?;
    Ok(())
}

fn session(app: &App, mut r: impl Read, mut w: impl Write) -> Result<()> {
    let mut log = match &app.args.log {
        Some(p) => Some((
            BufWriter::new(
                std::fs::File::create(p).with_context(|| format!("creating {}", p.display()))?,
            ),
            coffee_node::tlog::LogWriter::new(100),
        )),
        None => None,
    };
    // Telemetry is produced at 25 Hz of *machine* time; when the emulator runs
    // faster than real time, also cap it in wall time so browsers keep up.
    let mut last_telemetry_wall = Instant::now() - Duration::from_secs(1);
    loop {
        let Some(msg) = read_msg(&mut r)? else {
            return Ok(());
        };
        match msg {
            Message::Hello { version } => {
                if version != PROTOCOL_VERSION {
                    bail!(
                        "protocol version mismatch: emulator {version}, engine {PROTOCOL_VERSION}"
                    );
                }
                app.node.lock().unwrap().reset_session();
                write_msg(
                    &mut w,
                    &Message::Hello {
                        version: PROTOCOL_VERSION,
                    },
                )?;
                w.flush()?;
            }
            Message::Sensors(s) => {
                let mut node = app.node.lock().unwrap();
                let a = node.tick(&s);
                if let Some((f, lw)) = log.as_mut() {
                    lw.push(
                        &s,
                        coffee_node::tlog::Outputs {
                            heater: a.heater,
                            pump: a.pump,
                            valve: a.valve,
                        },
                    );
                    let chunk = lw.take();
                    if !chunk.is_empty() {
                        f.write_all(&chunk)?;
                    }
                }
                let events = node.drain_events();
                drop(node);
                if app.tx.receiver_count() > 0 {
                    for ev in events {
                        if matches!(ev, ServerMsg::Telemetry(_)) {
                            if last_telemetry_wall.elapsed() < Duration::from_millis(30) {
                                continue;
                            }
                            last_telemetry_wall = Instant::now();
                        }
                        if let Ok(s) = serde_json::to_string(&ev) {
                            let _ = app.tx.send(s);
                        }
                    }
                }
                write_msg(&mut w, &Message::Actuators(a))?;
                w.flush()?;
            }
            Message::Bye => {
                if let Some((mut f, mut lw)) = log {
                    f.write_all(&lw.finish())?;
                    f.flush()?;
                }
                return Ok(());
            }
            Message::Actuators(_) => bail!("engine received an Actuators frame"),
        }
    }
}
