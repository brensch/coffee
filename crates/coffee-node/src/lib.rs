//! The whole product, minus plumbing.
//!
//! A [`Node`] owns the engine and everything around it: shot recording, the
//! profile library, configuration, the HTTP API (as a pure function of
//! method/path/body), telemetry events, and the binary formats for the per-tick
//! log ([`tlog`]) and stored shots ([`shotpack`]). The PC binary
//! and the ESP32 firmware are thin shells around the same `Node`:
//!
//! | Shell        | Frames from              | Storage         | HTTP server        |
//! |--------------|--------------------------|-----------------|--------------------|
//! | PC           | emulator (stdio/TCP)     | files           | axum -> `handle_http` |
//! | ESP32        | real pins / USB bridge   | RAM (flash TODO)| picoserve -> `handle_http` (with Wi-Fi) |
//!
//! Anything that changes behaviour belongs here, not in a shell.

#![no_std]
extern crate alloc;

pub mod history;
pub mod shotpack;
pub mod tlog;

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use coffee_api::recorder::{Recorder, RecorderEvent};
use coffee_api::{Ack, Command, Info, ServerMsg, Shot, ShotMeta, Telemetry};
use coffee_core::engine::Command as EngCmd;
use coffee_core::{Engine, EngineConfig, Profile};
use coffee_io::{ActuatorFrame, SensorFrame};

/// Profiles every node ships with, compiled in so PC and ESP have the same library.
pub const BUILTIN_PROFILES: &[&str] = &[
    include_str!("../../../profiles/classic_9bar.json"),
    include_str!("../../../profiles/blooming.json"),
    include_str!("../../../profiles/flow_turbo.json"),
    include_str!("../../../profiles/magic_ristretto.json"),
];

const KEY_CONFIG: &str = "config.json";
const KEY_ACTIVE: &str = "active_profile";
const PREFIX_PROFILES: &str = "profiles/";
const PREFIX_SHOTS: &str = "shots/";

/// Key/value persistence. Keys look like paths (`profiles/x.json`).
pub trait Storage {
    fn load(&mut self, key: &str) -> Option<Vec<u8>>;
    fn save(&mut self, key: &str, data: &[u8]) -> Result<(), String>;
    /// Keys starting with `prefix`.
    fn list(&mut self, prefix: &str) -> Vec<String>;
}

impl<T: Storage + ?Sized> Storage for alloc::boxed::Box<T> {
    fn load(&mut self, key: &str) -> Option<Vec<u8>> {
        (**self).load(key)
    }
    fn save(&mut self, key: &str, data: &[u8]) -> Result<(), String> {
        (**self).save(key, data)
    }
    fn list(&mut self, prefix: &str) -> Vec<String> {
        (**self).list(prefix)
    }
}

/// In-memory storage (tests, emulator runs, and the ESP32 until flash storage lands).
#[derive(Default)]
pub struct MemStorage(pub BTreeMap<String, Vec<u8>>);

impl Storage for MemStorage {
    fn load(&mut self, key: &str) -> Option<Vec<u8>> {
        self.0.get(key).cloned()
    }
    fn save(&mut self, key: &str, data: &[u8]) -> Result<(), String> {
        self.0.insert(key.to_string(), data.to_vec());
        Ok(())
    }
    fn list(&mut self, prefix: &str) -> Vec<String> {
        self.0
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }
}

pub struct NodeOptions {
    pub info: Info,
    /// Shots kept in RAM (compressed, ~2.6 KB each) for the history API.
    pub max_shots: usize,
    /// Rolling telemetry history: samples kept and sample rate.
    pub history_capacity: usize,
    pub history_hz: f32,
    /// Also write each shot to storage.
    pub persist_shots: bool,
}

pub struct HttpResponse {
    pub status: u16,
    /// Always JSON.
    pub body: Vec<u8>,
}

/// An HTTP reply whose expensive part (JSON encoding of history, shots or the profile
/// library) hasn't been done yet. `Node::prepare_http` builds it while the caller holds the
/// node (cheap: owned copies only); `finish` encodes it after the node is released, so a big
/// request never holds up the control loop.
pub enum PreparedHttp {
    Done(HttpResponse),
    History(history::HistoryColumns),
    Shots(Vec<ShotMeta>),
    Shot(Shot),
    Profiles(Vec<Profile>),
}

impl PreparedHttp {
    pub fn finish(self) -> HttpResponse {
        match self {
            PreparedHttp::Done(r) => r,
            PreparedHttp::History(h) => json(200, &h),
            PreparedHttp::Shots(v) => json(200, &v),
            PreparedHttp::Shot(s) => json(200, &s),
            PreparedHttp::Profiles(v) => json(200, &v),
        }
    }
}

pub struct Node<S: Storage> {
    pub engine: Engine,
    pub storage: S,
    opts: NodeOptions,
    recorder: Recorder,
    /// Recent shots, compressed with `shotpack`.
    shots: Vec<(ShotMeta, Vec<u8>)>,
    history: history::History,
    library: Vec<Profile>,
    telemetry: Telemetry,
    inputs: u8,
    events: Vec<ServerMsg>,
    last_telemetry_t: f64,
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

impl<S: Storage> Node<S> {
    /// Build a node: built-in profiles, then user profiles and config from storage.
    pub fn new(mut storage: S, opts: NodeOptions) -> Self {
        let mut library: Vec<Profile> = BUILTIN_PROFILES
            .iter()
            .filter_map(|s| serde_json::from_str(s).ok())
            .collect();
        for key in storage.list(PREFIX_PROFILES) {
            if let Some(p) = storage
                .load(&key)
                .and_then(|b| serde_json::from_slice::<Profile>(&b).ok())
                .filter(|p| p.validate().is_ok())   // a corrupt or out-of-range stored profile is skipped
            {
                library.retain(|q| q.name != p.name);
                library.push(p);
            }
        }
        let cfg = storage
            .load(KEY_CONFIG)
            .and_then(|b| serde_json::from_slice::<EngineConfig>(&b).ok())
            .filter(|c| c.check().is_ok())        // out-of-range stored config: defaults
            .unwrap_or_default();
        let active = storage
            .load(KEY_ACTIVE)
            .and_then(|b| String::from_utf8(b).ok());
        let profile = active
            .and_then(|n| library.iter().find(|p| p.name == n).cloned())
            .or_else(|| library.first().cloned())
            .filter(|p| p.validate().is_ok())
            .unwrap_or_else(Profile::classic_9bar);
        let next_id = storage
            .list(PREFIX_SHOTS)
            .iter()
            .filter_map(|k| {
                k.trim_start_matches(PREFIX_SHOTS)
                    .trim_end_matches(".cshot")
                    .parse::<u32>()
                    .ok()
            })
            .max()
            .map(|m| m + 1)
            .unwrap_or(1);
        // Shots saved by earlier runs: the newest `max_shots` go back into the list.
        let mut keys: Vec<String> = storage
            .list(PREFIX_SHOTS)
            .into_iter()
            .filter(|k| k.ends_with(".cshot"))
            .collect();
        keys.sort();
        let skip = keys.len().saturating_sub(opts.max_shots);
        let shots: Vec<(ShotMeta, Vec<u8>)> = keys[skip..]
            .iter()
            .filter_map(|k| {
                let b = storage.load(k)?;
                let meta = shotpack::decode(&b)?.meta;
                Some((meta, b))
            })
            .collect();
        let history = history::History::new(opts.history_capacity, opts.history_hz);
        Self {
            engine: Engine::new(cfg, profile),
            storage,
            opts,
            recorder: Recorder::with_next_id(next_id),
            shots,
            history,
            library,
            telemetry: Telemetry::default(),
            inputs: 0,
            events: Vec::new(),
            last_telemetry_t: f64::NEG_INFINITY,
        }
    }

    pub fn info(&self) -> &Info {
        &self.opts.info
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    /// A new machine session (emulator handshake / bridge Hello): fresh engine
    /// state, same config and profile, ticked at the session's mains frequency.
    pub fn reset_session(&mut self, mains_hz: f32) {
        let mut cfg = self.engine.cfg.clone();
        cfg.mains_hz = mains_hz;
        self.engine = Engine::new(cfg, self.engine.profile.clone());
        self.last_telemetry_t = f64::NEG_INFINITY;
    }

    /// Replace the engine config (e.g. from a file on the command line). Not persisted.
    pub fn set_config(&mut self, cfg: EngineConfig) -> Result<(), &'static str> {
        self.engine.command(EngCmd::SetConfig(cfg))
    }

    /// Select a profile by name, or install one given directly. Not persisted.
    pub fn set_profile(&mut self, p: Profile) -> Result<(), &'static str> {
        self.engine.command(EngCmd::SetProfile(p))
    }

    /// Add read-only profiles (e.g. imported community libraries). Names already
    /// in the library (built-in or user-saved) win.
    pub fn extend_library(&mut self, profiles: impl IntoIterator<Item = Profile>) {
        for mut p in profiles {
            if let Some(existing) = self.library.iter().find(|q| q.name == p.name) {
                // Same name from a different author (e.g. Decent's and GaggiMate's
                // "Cremina"): keep both, disambiguated by author.
                if existing.author == p.author {
                    continue;
                }
                p.name = format!(
                    "{} ({})",
                    p.name,
                    p.author.as_deref().unwrap_or("community")
                );
                if self.library.iter().any(|q| q.name == p.name) {
                    continue;
                }
            }
            self.library.push(p);
        }
    }

    pub fn library(&self) -> &[Profile] {
        &self.library
    }

    /// One mains cycle.
    pub fn tick(&mut self, s: &SensorFrame) -> ActuatorFrame {
        let t = s.t_us as f64 * 1e-6;
        let a = self.engine.tick(s);
        self.inputs = s.inputs;
        let flushing = self.engine.profile.name == coffee_core::Profile::FLUSH_NAME;
        self.history.observe(t, &a, flushing);
        let last = self.engine.last_shot();
        let profile = &self.engine.profile;
        let ev = self.recorder.observe(t, &a, profile, last);
        let mut force = false;
        match ev {
            // Flushes aren't shots: keep them out of history.
            Some(RecorderEvent::Started { .. }) | Some(RecorderEvent::Ended(_))
                if profile.name == coffee_core::Profile::FLUSH_NAME => {}
            Some(RecorderEvent::Started { id }) => {
                self.events.push(ServerMsg::ShotStarted {
                    id,
                    profile: profile.name.clone(),
                });
                force = true;
            }
            Some(RecorderEvent::Ended(shot))
                if shot.meta.profile == coffee_core::Profile::FLUSH_NAME => {}
            Some(RecorderEvent::Ended(shot)) => {
                self.events.push(ServerMsg::ShotEnded(shot.meta.clone()));
                let packed = shotpack::encode(&shot);
                if self.opts.persist_shots {
                    let _ = self
                        .storage
                        .save(&format!("{PREFIX_SHOTS}{:05}.cshot", shot.meta.id), &packed);
                }
                self.shots.push((shot.meta, packed));
                if self.shots.len() > self.opts.max_shots {
                    self.shots.remove(0);
                }
                force = true;
            }
            None => {}
        }
        if force || t - self.last_telemetry_t >= 0.04 {
            self.last_telemetry_t = t;
            self.telemetry = Telemetry::from_frame(t, &a, s.inputs, &self.engine.profile);
            self.events
                .push(ServerMsg::Telemetry(self.telemetry.clone()));
        }
        a
    }

    /// Events produced since the last call (telemetry at ~25 Hz of machine time,
    /// shot start/end). Shells forward these to WebSocket clients.
    pub fn drain_events(&mut self) -> Vec<ServerMsg> {
        core::mem::take(&mut self.events)
    }

    // ------------------------------------------------------------------ API

    fn ack(r: Result<(), impl Into<String>>) -> HttpResponse {
        json(
            200,
            &match r {
                Ok(()) => Ack::ok(),
                Err(e) => Ack::err(e),
            },
        )
    }

    /// The entire HTTP API. Shells pass requests through verbatim.
    pub fn handle_http(&mut self, method: &str, path: &str, body: &[u8]) -> HttpResponse {
        self.prepare_http(method, path, body).finish()
    }

    /// Like `handle_http`, but leaves the JSON encoding of large replies to
    /// `PreparedHttp::finish`, to run after the caller releases the node.
    pub fn prepare_http(&mut self, method: &str, path: &str, body: &[u8]) -> PreparedHttp {
        let (path, query) = path.split_once('?').unwrap_or((path, ""));
        let param = |name: &str| {
            query.split('&').find_map(|kv| {
                kv.split_once('=')
                    .filter(|(k, _)| *k == name)
                    .map(|(_, v)| v)
            })
        };
        let r = match (method, path) {
            ("GET", "/api/telemetry/history") => {
                let secs = param("seconds")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(600.0);
                return PreparedHttp::History(self.history.columns(secs));
            }
            ("GET", "/api/info") => json(200, &self.opts.info),
            ("GET", "/api/status") => json(200, &self.telemetry),
            ("POST", "/api/command") => match serde_json::from_slice::<Command>(body) {
                Ok(c) => {
                    let r = self.command(c);
                    Self::ack(r)
                }
                Err(e) => json(400, &Ack::err(format!("bad command: {e}"))),
            },
            ("GET", "/api/profile") => json(200, &self.engine.profile),
            ("PUT", "/api/profile") => match serde_json::from_slice::<Profile>(body) {
                Ok(p) => {
                    let r = self.save_profile(p);
                    Self::ack(r)
                }
                Err(e) => json(400, &Ack::err(format!("bad profile: {e}"))),
            },
            ("GET", "/api/profiles") => return PreparedHttp::Profiles(self.library.clone()),
            ("GET", "/api/config") => json(200, &self.engine.cfg),
            ("PUT", "/api/config") => match serde_json::from_slice::<EngineConfig>(body) {
                Ok(c) => {
                    let r = self
                        .engine
                        .command(EngCmd::SetConfig(c.clone()))
                        .map_err(String::from)
                        .and_then(|_| {
                            let b = serde_json::to_vec(&c).map_err(|e| e.to_string())?;
                            self.storage.save(KEY_CONFIG, &b)
                        });
                    Self::ack(r)
                }
                Err(e) => json(400, &Ack::err(format!("bad config: {e}"))),
            },
            ("GET", "/api/shots") => {
                return PreparedHttp::Shots(self.shots.iter().rev().map(|s| s.0.clone()).collect());
            }
            ("GET", "/api/shot/current") => match self.recorder.current() {
                Some(s) => return PreparedHttp::Shot(s.clone()),
                None => json(404, &Ack::err("not brewing")),
            },
            ("GET", p) if p.starts_with("/api/shots/") => {
                let id = p.trim_start_matches("/api/shots/").parse::<u32>().ok();
                let found = id.and_then(|id| self.find_shot(id));
                match found {
                    Some(s) => return PreparedHttp::Shot(s),
                    None => json(404, &Ack::err("no such shot")),
                }
            }
            _ => json(404, &Ack::err("no such endpoint")),
        };
        PreparedHttp::Done(r)
    }

    pub fn command(&mut self, c: Command) -> Result<(), String> {
        let r = match c {
            Command::StartBrew { force } => {
                use coffee_io::EngineState as S;
                let st = self.engine.state();
                if !force && st != S::Ready {
                    let t = &self.telemetry;
                    let why = match st {
                        S::Heating | S::Boot => format!(
                            "not ready yet: water {:.0}°C of {:.0}, group {:.0}% warm",
                            t.water_c.or(t.temp_c).unwrap_or(0.0),
                            t.setpoint_c,
                            t.heat_soak * 100.0
                        ),
                        S::Fault => String::from("machine is in a fault state"),
                        other => format!("machine is busy ({})", other.name()),
                    };
                    return Err(why);
                }
                self.engine.command(EngCmd::StartBrew)
            }
            Command::StopBrew => self.engine.command(EngCmd::StopBrew),
            Command::ClearFaults => self.engine.command(EngCmd::ClearFaults),
            Command::SetBrewTemp { c } => self.engine.command(EngCmd::SetBrewTemp(c)),
            Command::SetSteamTemp { c } => self.engine.command(EngCmd::SetSteamTemp(c)),
            Command::SelectProfile { name } => {
                match self.library.iter().find(|p| p.name == name).cloned() {
                    Some(p) => {
                        let r = self.engine.command(EngCmd::SetProfile(p));
                        if r.is_ok() {
                            let _ = self.storage.save(KEY_ACTIVE, name.as_bytes());
                        }
                        r
                    }
                    None => Err("no such profile"),
                }
            }
            Command::SetReferenceShot { id } => return self.set_reference(id),
            Command::Flush { seconds } => self.engine.command(EngCmd::Flush(seconds)),
        };
        r.map_err(String::from)
    }

    fn find_shot(&mut self, id: u32) -> Option<Shot> {
        self.shots
            .iter()
            .find(|s| s.0.id == id)
            .and_then(|s| shotpack::decode(&s.1))
            .or_else(|| {
                let b = self.storage.load(&format!("{PREFIX_SHOTS}{id:05}.cshot"))?;
                shotpack::decode(&b)
            })
    }

    fn set_reference(&mut self, id: u32) -> Result<(), String> {
        let shot = self.find_shot(id).ok_or("no such shot")?;
        let r = shot
            .meta
            .mean_resistance
            .ok_or("that shot never reached extraction pressure")?;
        let mut p = self
            .library
            .iter()
            .find(|p| p.name == shot.meta.profile)
            .cloned()
            .ok_or("that shot's profile is no longer in the library")?;
        p.target_resistance = Some(r);
        if self.engine.profile.name == p.name {
            self.engine
                .command(EngCmd::SetProfile(p.clone()))
                .map_err(String::from)?;
        }
        self.library.retain(|q| q.name != p.name);
        self.library.push(p.clone());
        let b = serde_json::to_vec(&p).map_err(|e| e.to_string())?;
        self.storage
            .save(&format!("{PREFIX_PROFILES}{}.json", slug(&p.name)), &b)
    }

    fn save_profile(&mut self, p: Profile) -> Result<(), String> {
        self.engine
            .command(EngCmd::SetProfile(p.clone()))
            .map_err(String::from)?;
        self.library.retain(|q| q.name != p.name);
        self.library.push(p.clone());
        let b = serde_json::to_vec(&p).map_err(|e| e.to_string())?;
        self.storage
            .save(&format!("{PREFIX_PROFILES}{}.json", slug(&p.name)), &b)?;
        self.storage.save(KEY_ACTIVE, p.name.as_bytes())
    }
}

fn json<T: serde::Serialize + ?Sized>(status: u16, v: &T) -> HttpResponse {
    HttpResponse {
        status,
        body: serde_json::to_vec(v).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coffee_core::sensors;
    use coffee_io::input;

    fn node() -> Node<MemStorage> {
        Node::new(
            MemStorage::default(),
            NodeOptions {
                info: Info {
                    name: "t".into(),
                    version: "0".into(),
                    platform: "test".into(),
                    emulated: true,
                    lab_url: None,
                },
                max_shots: 10,
                persist_shots: true,
                history_capacity: 600,
                history_hz: 5.0,
            },
        )
    }

    fn frame(i: u32, temp: f32, inputs: u8) -> SensorFrame {
        let cfg = EngineConfig::default();
        SensorFrame {
            cycle: i,
            t_us: (i as u64 * 50_000 + 1) / 3, // 60 Hz crystal grid
            ntc_mv: sensors::ntc_mv_for_c(&cfg.ntc, temp).round() as u16,
            pressure_mv: sensors::pressure_to_mv(&cfg.pressure_sensor, 0.0, 0.0).round() as u16,
            scale_mg: 0,
            inputs,
        }
    }

    #[test]
    fn builtins_and_api_roundtrip() {
        let mut n = node();
        assert_eq!(n.library().len(), BUILTIN_PROFILES.len());
        let r = n.handle_http("GET", "/api/profiles", b"");
        assert_eq!(r.status, 200);
        let r = n.handle_http(
            "POST",
            "/api/command",
            br#"{"cmd":"select_profile","name":"Blooming"}"#,
        );
        assert!(
            core::str::from_utf8(&r.body)
                .unwrap()
                .contains("\"ok\":true")
        );
        assert_eq!(n.engine.profile.name, "Blooming");
        // Selection persists: a new node on the same storage starts on it.
        let st = core::mem::take(&mut n.storage);
        let n2 = Node::new(
            st,
            NodeOptions {
                info: n.opts.info.clone(),
                max_shots: 10,
                persist_shots: true,
                history_capacity: 600,
                history_hz: 5.0,
            },
        );
        assert_eq!(n2.engine.profile.name, "Blooming");
        assert_eq!(n.handle_http("GET", "/api/nope", b"").status, 404);
    }

    #[test]
    fn flush_runs_stops_and_is_not_a_shot() {
        let mut n = node();
        let mut i = 0;
        for _ in 0..300 {
            n.tick(&frame(i, 93.0, 0));
            i += 1;
        }
        let r = n.handle_http("POST", "/api/command", br#"{"cmd":"flush","seconds":2}"#);
        assert!(
            core::str::from_utf8(&r.body)
                .unwrap()
                .contains("\"ok\":true"),
            "{:?}",
            core::str::from_utf8(&r.body)
        );
        let mut saw_brewing = false;
        for _ in 0..1000 {
            let a = n.tick(&frame(i, 93.0, 0));
            saw_brewing |= a.status.state == coffee_io::EngineState::Brewing;
            i += 1;
        }
        assert!(saw_brewing);
        assert_eq!(
            n.recorder.next_id(),
            1,
            "a flush must not use up a shot number"
        );
        assert_eq!(
            n.engine.profile.name, "Classic 9 bar",
            "profile restored after flush"
        );
        assert_eq!(n.handle_http("GET", "/api/shots", b"").body, b"[]");
        assert!(
            !n.drain_events()
                .iter()
                .any(|e| matches!(e, ServerMsg::ShotEnded(_)))
        );
    }

    #[test]
    fn records_a_shot() {
        let mut n = node();
        let mut i = 0;
        for _ in 0..300 {
            n.tick(&frame(i, 93.0, 0));
            i += 1;
        }
        for _ in 0..200 {
            n.tick(&frame(i, 93.0, input::BREW));
            i += 1;
        }
        // Brew switch off, then the 4 s drip-settle window before the shot is filed.
        for _ in 0..600 {
            n.tick(&frame(i, 93.0, 0));
            i += 1;
        }
        let ev = n.drain_events();
        assert!(
            ev.iter()
                .any(|e| matches!(e, ServerMsg::ShotStarted { .. }))
        );
        assert!(ev.iter().any(|e| matches!(e, ServerMsg::ShotEnded(_))));
        let r = n.handle_http("GET", "/api/shots/1", b"");
        assert_eq!(r.status, 200);
        let h = n.handle_http("GET", "/api/telemetry/history?seconds=2", b"");
        let v: serde_json::Value = serde_json::from_slice(&h.body).unwrap();
        let k = v["t"].as_array().unwrap().len();
        assert!((9..=11).contains(&k), "2 s at 5 Hz, got {k}");
        assert!(n.storage.list("shots/").len() == 1);
        // Survives a restart: served from storage once it's gone from RAM.
        let st = core::mem::take(&mut n.storage);
        let mut n2 = Node::new(
            st,
            NodeOptions {
                info: n.opts.info.clone(),
                max_shots: 10,
                persist_shots: true,
                history_capacity: 600,
                history_hz: 5.0,
            },
        );
        assert_eq!(n2.handle_http("GET", "/api/shots/1", b"").status, 200);
        let list = String::from_utf8(n2.handle_http("GET", "/api/shots", b"").body).unwrap();
        assert!(
            list.contains("\"id\":1"),
            "stored shot missing from list after restart: {list}"
        );
    }
}
