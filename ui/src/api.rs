//! HTTP + WebSocket plumbing and the app-wide reactive store.

use coffee_api::lab::{LabCommand, LabFrame, LabInfo};
use coffee_api::{Ack, Command, Info, Profile, ServerMsg, ShotMeta, Telemetry};
use futures::StreamExt;
use gloo_net::http::Request;
use gloo_net::websocket::{Message, futures::WebSocket};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Rolling live sample used by the charts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Live {
    pub t: f64,
    pub temp: f64,
    pub setpoint: f64,
    pub pressure: f64,
    pub target_pressure: f64,
    pub flow: f64,
    pub target_flow: f64,
    pub weight: f64,
    pub phase: Option<u8>,
    pub brewing: bool,
}

const LIVE_WINDOW_S: f64 = 600.0;

#[derive(Clone, Copy)]
pub struct Store {
    pub connected: RwSignal<bool>,
    pub info: RwSignal<Option<Info>>,
    pub tel: RwSignal<Option<Telemetry>>,
    pub live: RwSignal<Vec<Live>>,
    pub shots: RwSignal<Vec<ShotMeta>>,
    /// Shot to open when the History page shows next.
    pub selected_shot: RwSignal<Option<u32>>,
    pub profiles: RwSignal<Vec<Profile>>,
    pub toast: RwSignal<Option<(String, bool)>>,
    // lab
    pub lab_url: RwSignal<Option<String>>,
    pub lab_info: RwSignal<Option<LabInfo>>,
    pub lab_connected: RwSignal<bool>,
    pub lab_truth: RwSignal<Option<LabTruth>>,
    pub lab_hist: RwSignal<Vec<LabTruth>>,
    pub lab_events: RwSignal<Vec<(f64, String, i8)>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LabTruth {
    pub t: f64,
    pub speed: f32,
    pub paused: bool,
    pub engine_connected: bool,
    pub values: Vec<f64>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            connected: RwSignal::new(false),
            info: RwSignal::new(None),
            tel: RwSignal::new(None),
            live: RwSignal::new(vec![]),
            shots: RwSignal::new(vec![]),
            selected_shot: RwSignal::new(None),
            profiles: RwSignal::new(vec![]),
            toast: RwSignal::new(None),
            lab_url: RwSignal::new(None),
            lab_info: RwSignal::new(None),
            lab_connected: RwSignal::new(false),
            lab_truth: RwSignal::new(None),
            lab_hist: RwSignal::new(vec![]),
            lab_events: RwSignal::new(vec![]),
        }
    }

    pub fn notify(&self, text: impl Into<String>, ok: bool) {
        let text = text.into();
        self.toast.set(Some((text.clone(), ok)));
        let toast = self.toast;
        spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(3500).await;
            if toast.get_untracked().is_some_and(|(t, _)| t == text) {
                toast.set(None);
            }
        });
    }

    /// Index of a lab signal by name.
    pub fn lab_idx(&self, name: &str) -> Option<usize> {
        self.lab_info.with_untracked(|i| {
            i.as_ref()
                .and_then(|i| i.signals.iter().position(|s| s.name == name))
        })
    }

    pub fn lab_val(&self, name: &str) -> f64 {
        let Some(i) = self.lab_idx(name) else {
            return f64::NAN;
        };
        self.lab_truth.with(|t| {
            t.as_ref()
                .and_then(|t| t.values.get(i).copied())
                .unwrap_or(f64::NAN)
        })
    }
}

pub fn store() -> Store {
    expect_context::<Store>()
}

fn origin() -> (String, String) {
    let loc = web_sys::window().unwrap().location();
    let host = loc.host().unwrap_or_default();
    let proto = loc.protocol().unwrap_or_default();
    let ws = if proto == "https:" { "wss" } else { "ws" };
    (format!("{proto}//{host}"), format!("{ws}://{host}"))
}

pub async fn get<T: DeserializeOwned>(url: &str) -> Result<T, String> {
    let r = Request::get(url).send().await.map_err(|e| e.to_string())?;
    if !r.ok() {
        return Err(format!(
            "{} {}",
            r.status(),
            r.text().await.unwrap_or_default()
        ));
    }
    r.json::<T>().await.map_err(|e| e.to_string())
}

pub async fn send<B: Serialize, T: DeserializeOwned>(
    method: &str,
    url: &str,
    body: &B,
) -> Result<T, String> {
    let req = match method {
        "PUT" => Request::put(url),
        _ => Request::post(url),
    };
    let r = req
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    r.json::<T>().await.map_err(|e| e.to_string())
}

fn ack_toast(s: Store, r: Result<Ack, String>, ok_text: &str) {
    match r {
        Ok(a) if a.ok => {
            if !ok_text.is_empty() {
                s.notify(ok_text, true)
            }
        }
        Ok(a) => s.notify(a.error.unwrap_or_else(|| "failed".into()), false),
        Err(e) => s.notify(e, false),
    }
}

pub fn command(cmd: Command, ok_text: &'static str) {
    let s = store();
    spawn_local(async move {
        let r = send::<_, Ack>("POST", "/api/command", &cmd).await;
        ack_toast(s, r, ok_text);
    });
}

pub fn lab_command(cmd: LabCommand) {
    let s = store();
    let Some(base) = s.lab_url.get_untracked() else {
        s.notify("lab not connected", false);
        return;
    };
    spawn_local(async move {
        let r = send::<_, Ack>("POST", &format!("{base}/lab/command"), &cmd).await;
        ack_toast(s, r, "");
    });
}

pub fn refresh_shots() {
    refresh_shots_for(store());
}

/// For use outside a component (e.g. the WebSocket task), where there's no
/// reactive owner to look the store up from.
pub fn refresh_shots_for(s: Store) {
    spawn_local(async move {
        if let Ok(v) = get::<Vec<ShotMeta>>("/api/shots").await {
            s.shots.set(v);
        }
    });
}

pub fn refresh_profiles() {
    refresh_profiles_for(store());
}

pub fn refresh_profiles_for(s: Store) {
    spawn_local(async move {
        if let Ok(v) = get::<Vec<Profile>>("/api/profiles").await {
            s.profiles.set(v);
        }
    });
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct HistoryCols {
    t: Vec<f64>,
    temp: Vec<Option<f32>>,
    water: Vec<Option<f32>>,
    setpoint: Vec<Option<f32>>,
    pressure: Vec<Option<f32>>,
    target_pressure: Vec<Option<f32>>,
    flow: Vec<Option<f32>>,
    target_flow: Vec<Option<f32>>,
    weight: Vec<Option<f32>>,
    state: Vec<String>,
    phase: Vec<Option<u8>>,
}

impl HistoryCols {
    fn into_live(self) -> Vec<Live> {
        let g = |v: &Vec<Option<f32>>, i: usize| {
            v.get(i)
                .copied()
                .flatten()
                .map(|x| x as f64)
                .unwrap_or(f64::NAN)
        };
        (0..self.t.len())
            .map(|i| Live {
                t: self.t[i],
                temp: {
                    let w = g(&self.water, i);
                    if w.is_finite() { w } else { g(&self.temp, i) }
                },
                setpoint: g(&self.setpoint, i),
                pressure: g(&self.pressure, i),
                target_pressure: g(&self.target_pressure, i),
                flow: g(&self.flow, i),
                target_flow: g(&self.target_flow, i),
                weight: g(&self.weight, i),
                phase: self.phase.get(i).copied().flatten(),
                brewing: self.state.get(i).is_some_and(|s| s == "brewing"),
            })
            .collect()
    }
}

fn nan(x: Option<f32>) -> f64 {
    x.map(|v| v as f64).unwrap_or(f64::NAN)
}

fn on_telemetry(s: Store, t: Telemetry) {
    let brewing = t.state == "brewing";
    let sample = Live {
        t: t.t,
        temp: nan(t.water_c.or(t.temp_c)),
        setpoint: t.setpoint_c as f64,
        pressure: nan(t.pressure_bar),
        target_pressure: nan(t.target_pressure_bar),
        flow: t.flow_ml_s as f64,
        target_flow: nan(t.target_flow_ml_s),
        weight: nan(t.weight_g),
        phase: t.phase,
        brewing,
    };
    // A slightly older sample is normal right after connecting (the WS snapshot
    // predates the history fetch): skip it. Only a big jump back means the
    // engine restarted.
    let stale = s.live.with_untracked(|v| {
        v.last()
            .is_some_and(|l| sample.t <= l.t && sample.t > l.t - 5.0)
    });
    s.live.update(|v| {
        if stale {
            return;
        }
        if v.last().is_some_and(|l| sample.t <= l.t - 5.0) {
            v.clear(); // engine restarted
        }
        v.push(sample);
        let cut = sample.t - LIVE_WINDOW_S;
        if v.first().is_some_and(|f| f.t < cut) {
            let k = v.partition_point(|x| x.t < cut);
            v.drain(..k);
        }
    });
    s.tel.set(Some(t));
}

pub fn start_engine_stream(s: Store) {
    spawn_local(async move {
        if let Ok(info) = get::<Info>("/api/info").await {
            let lab = info.lab_url.clone().or_else(|| {
                let host = web_sys::window()?.location().hostname().ok()?;
                Some(format!("http://{host}:8081"))
            });
            s.info.set(Some(info));
            if let Some(l) = lab {
                start_lab(s, l);
            }
        }
    });
    refresh_shots_for(s);
    refresh_profiles_for(s);
    spawn_local(async move {
        let (_, ws_base) = origin();
        loop {
            match WebSocket::open(&format!("{ws_base}/api/ws")) {
                Ok(ws) => {
                    s.connected.set(true);
                    // Backfill the charts from the node's rolling history.
                    if let Ok(h) = get::<HistoryCols>("/api/telemetry/history?seconds=600").await {
                        s.live.set(h.into_live());
                    }
                    let (_, mut rx) = ws.split();
                    while let Some(Ok(Message::Text(txt))) = rx.next().await {
                        match serde_json::from_str::<ServerMsg>(&txt) {
                            Ok(ServerMsg::Telemetry(t)) => on_telemetry(s, t),
                            Ok(ServerMsg::ShotStarted { .. }) => {}
                            Ok(ServerMsg::ShotEnded(meta)) => {
                                let w = meta
                                    .weight_g
                                    .map(|w| format!(" · {w:.1} g"))
                                    .unwrap_or_default();
                                s.notify(
                                    format!("Shot #{} done: {:.1} s{w}", meta.id, meta.duration_s),
                                    true,
                                );
                                refresh_shots_for(s);
                            }
                            Ok(ServerMsg::Log { text }) => s.notify(text, true),
                            Err(_) => {}
                        }
                    }
                    s.connected.set(false);
                }
                Err(_) => s.connected.set(false),
            }
            gloo_timers::future::TimeoutFuture::new(1000).await;
        }
    });
}

/// Probe for an emulator lab API and stream ground truth from it.
pub fn start_lab(s: Store, base: String) {
    spawn_local(async move {
        loop {
            match get::<LabInfo>(&format!("{base}/lab/info")).await {
                Ok(info) => {
                    s.lab_info.set(Some(info));
                    s.lab_url.set(Some(base.clone()));
                    let ws_url = base.replacen("http", "ws", 1) + "/lab/ws";
                    if let Ok(ws) = WebSocket::open(&ws_url) {
                        s.lab_connected.set(true);
                        let (_, mut rx) = ws.split();
                        while let Some(Ok(Message::Text(txt))) = rx.next().await {
                            match serde_json::from_str::<LabFrame>(&txt) {
                                Ok(LabFrame::Truth {
                                    t,
                                    speed,
                                    paused,
                                    engine_connected,
                                    values,
                                }) => {
                                    let values =
                                        values.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect();
                                    let tr = LabTruth {
                                        t,
                                        speed,
                                        paused,
                                        engine_connected,
                                        values,
                                    };
                                    s.lab_hist.update(|h| {
                                        if h.last().is_some_and(|l| t < l.t - 5.0) {
                                            h.clear();
                                        }
                                        h.push(tr.clone());
                                        let cut = t - LIVE_WINDOW_S;
                                        let k = h.partition_point(|x| x.t < cut);
                                        h.drain(..k);
                                    });
                                    s.lab_truth.set(Some(tr));
                                }
                                Ok(LabFrame::Event { t, text }) => s.lab_events.update(|e| {
                                    e.push((t, text, 0));
                                    if e.len() > 300 {
                                        e.remove(0);
                                    }
                                }),
                                Ok(LabFrame::ScenarioDone {
                                    name,
                                    passed,
                                    lines,
                                }) => {
                                    let t = s.lab_truth.get_untracked().map(|x| x.t).unwrap_or(0.0);
                                    s.lab_events.update(|e| {
                                        e.push((
                                            t,
                                            format!(
                                                "scenario {} {}",
                                                if passed { "PASSED" } else { "FAILED" },
                                                name
                                            ),
                                            if passed { 1 } else { -1 },
                                        ));
                                        for l in lines {
                                            let k = if l.starts_with("ok") { 1 } else { -1 };
                                            e.push((t, l, k));
                                        }
                                    });
                                    s.notify(
                                        format!(
                                            "Scenario {}: {name}",
                                            if passed { "passed" } else { "failed" }
                                        ),
                                        passed,
                                    );
                                }
                                Err(_) => {}
                            }
                        }
                        s.lab_connected.set(false);
                    }
                }
                Err(_) => {
                    s.lab_connected.set(false);
                }
            }
            gloo_timers::future::TimeoutFuture::new(3000).await;
        }
    });
}
