//! Animated machine diagram. Fed either from engine telemetry (what the machine
//! itself knows: works identically on the ESP32) or from emulator ground truth.
//! Anything a source can't know is `None` and drawn as unknown.

use crate::api::Store;
use leptos::prelude::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagramData {
    pub reservoir_ml: Option<f64>,
    pub pump_on: bool,
    pub pump_flow: Option<f64>,
    pub opv_flow: Option<f64>,
    pub pressure: Option<f64>,
    /// Boiler wall temperature (where the probe is).
    pub wall_c: Option<f64>,
    pub water_c: Option<f64>,
    pub water_ml: Option<f64>,
    pub heater_on: bool,
    /// 0..1 drive levels for the glow/pulse.
    pub heater_level: f64,
    pub pump_level: f64,
    /// Heater power (W) or duty (0..1) caption.
    pub heater_caption: String,
    pub probe_c: Option<f64>,
    pub probe_fault: bool,
    pub valve_open: bool,
    pub puck_wet: Option<f64>,
    pub puck_flow: Option<f64>,
    pub cup_g: Option<f64>,
    /// Group/portafilter temperature, and whether it's an estimate.
    pub group_c: Option<f64>,
    pub group_estimated: bool,
    pub steam_flow: Option<f64>,
    pub wand_open: Option<bool>,
    pub state: String,
    pub caption: String,
}

/// What the engine knows, from telemetry alone.
pub fn from_telemetry(s: Store) -> Signal<DiagramData> {
    Signal::derive(move || {
        s.tel.with(|t| {
            let Some(t) = t.as_ref() else {
                return DiagramData {
                    state: "offline".into(),
                    ..Default::default()
                };
            };
            let f = |x: Option<f32>| x.map(|v| v as f64);
            DiagramData {
                pump_on: t.pump || t.pump_duty > 0.01,
                pump_flow: Some(t.flow_ml_s as f64),
                pressure: f(t.pressure_bar),
                wall_c: f(t.temp_c),
                water_c: f(t.water_c),
                heater_on: t.heater_duty > 0.02,
                heater_level: t.heater_duty.clamp(0.0, 1.0) as f64,
                pump_level: t.pump_duty.clamp(0.0, 1.0) as f64,
                heater_caption: format!("heater {:.0}%", t.heater_duty * 100.0),
                probe_c: f(t.temp_c),
                probe_fault: t.faults.iter().any(|x| x.starts_with("ntc")),
                valve_open: t.valve,
                puck_flow: (t.valve && t.flow_ml_s > 0.2).then_some(t.flow_ml_s as f64),
                cup_g: f(t.weight_g),
                group_c: f(t.group_c),
                group_estimated: true,
                state: if t.state == "brewing" && t.profile == "Flush" {
                    "flushing".into()
                } else {
                    t.state.clone()
                },
                caption: "from telemetry: what the machine knows".into(),
                ..Default::default()
            }
        })
    })
}

/// Emulator ground truth.
pub fn from_truth(s: Store) -> Signal<DiagramData> {
    Signal::derive(move || {
        let v = |n: &str| {
            let x = s.lab_val(n);
            x.is_finite().then_some(x)
        };
        let on = |n: &str| v(n).unwrap_or(0.0) > 0.5;
        DiagramData {
            reservoir_ml: v("reservoir.ml"),
            pump_on: on("pump.on"),
            pump_flow: v("pump.flow"),
            opv_flow: v("opv.flow"),
            pressure: v("hydraulics.pressure"),
            wall_c: v("boiler.temp"),
            water_c: v("boiler.water_temp"),
            water_ml: v("boiler.water_ml"),
            heater_on: v("heater.power").unwrap_or(0.0) > 0.0,
            heater_level: (v("heater.power").unwrap_or(0.0) / 1370.0).clamp(0.0, 1.0),
            pump_level: if on("pump.on") { 1.0 } else { 0.0 },
            heater_caption: format!("{:.0} W", v("heater.power").unwrap_or(0.0)),
            probe_c: v("probe.temp"),
            probe_fault: false,
            valve_open: on("valve.open"),
            puck_wet: v("puck.wet"),
            puck_flow: v("puck.flow"),
            cup_g: v("cup.mass"),
            group_c: v("portafilter.temp"),
            group_estimated: false,
            steam_flow: v("steam.flow"),
            wand_open: Some(on("wand.open")),
            state: s
                .tel
                .with(|t| t.as_ref().map(|t| t.state.clone()).unwrap_or_default()),
            caption: "ground truth from the emulator".into(),
        }
    })
}

fn temp_hue(t: Option<f64>) -> Option<f64> {
    t.map(|t| 210.0 - 210.0 * ((t - 20.0) / 110.0).clamp(0.0, 1.0))
}

fn tint(t: Option<f64>) -> String {
    match temp_hue(t) {
        Some(h) => format!("--tint: hsl({h:.0} 80% 55%)"),
        None => "--tint: var(--border)".into(),
    }
}

fn num(v: Option<f64>, dp: usize) -> String {
    match v {
        Some(v) => format!("{:.*}", dp, if v.abs() < 0.05 { 0.0 } else { v }),
        None => "–".into(),
    }
}

#[component]
fn Stage(
    #[prop(into)] name: String,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] unit: String,
    #[prop(into)] sub: Signal<String>,
    #[prop(into)] style: Signal<String>,
    #[prop(into)] active: Signal<bool>,
    #[prop(optional, into)] badge: Option<Signal<Option<String>>>,
) -> impl IntoView {
    view! {
        <div class="stage" class:active=active style=style>
            <div class="stage-name">{name}{move || badge.and_then(|b| b.get()).map(|b| view! { <span class="stage-badge">{b}</span> })}</div>
            <div class="stage-value">{move || value.get()}<span class="stage-unit">{move || if value.get() == "–" { String::new() } else { unit.clone() }}</span></div>
            <div class="stage-sub">{sub}</div>
        </div>
    }
}

#[component]
fn Link(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] flowing: Signal<bool>,
    #[prop(optional)] warm: bool,
    /// Draw a ball valve in the pipe: bore along the pipe when open, across it when closed.
    #[prop(optional, into)] valve: Option<Signal<bool>>,
) -> impl IntoView {
    let has_valve = valve.is_some();
    let open = Signal::derive(move || valve.map(|v| v.get()).unwrap_or(true));
    view! {
        <div class="link" class:flowing=flowing class:warm=warm class:has-valve=has_valve
            class:valve-open=move || has_valve && open.get() class:valve-closed=move || has_valve && !open.get()>
            <div class="link-pipe">
                <div class="link-line"><i></i></div>
                {has_valve.then(|| view! {
                    <svg class="valve" viewBox="0 0 32 32" role="img"
                        aria-label=move || if open.get() { "valve open" } else { "valve closed" }>
                        <circle class="valve-body" cx="16" cy="16" r="13" />
                        <g class="valve-ball">
                            <rect class="valve-bore" x="4" y="11.5" width="24" height="9" rx="2" />
                        </g>
                    </svg>
                })}
            </div>
            <div class="link-label">
                {has_valve.then(|| view! { <span class="valve-dot"></span> })}
                {label}
            </div>
        </div>
    }
}

/// The water's journey: tank → pump → boiler → portafilter → cup.
#[component]
pub fn Diagram(#[prop(into)] data: Signal<DiagramData>) -> impl IntoView {
    let d = move || data.get();
    let flowing = Signal::derive(move || d().pump_flow.unwrap_or(0.0) > 0.05 || d().pump_on);
    let dripping = Signal::derive(move || d().puck_flow.unwrap_or(0.0) > 0.05);
    let valve = Signal::derive(move || d().valve_open);

    view! {
        <div class="flow">
            <Stage name="Tank"
                value=Signal::derive(move || d().reservoir_ml.map(|r| format!("{r:.0}")).unwrap_or_else(|| "–".into()))
                unit=" ml"
                sub=Signal::derive(move || match (d().reservoir_ml, d().opv_flow) {
                    (_, Some(o)) if o > 0.05 => format!("{o:.1} ml/s returning"),
                    (Some(_), _) => "water level".into(),
                    (None, _) => "level not sensed".into(),
                })
                style=Signal::derive(String::new)
                active=Signal::derive(|| false) />
            <Link label=Signal::derive(move || if flowing.get() { String::from("drawing") } else { String::new() }) flowing=flowing />
            <Stage name="Pump"
                value=Signal::derive(move || num(d().pump_flow, 1))
                unit=" ml/s"
                sub=Signal::derive(move || if d().pump_level > 0.0 { format!("{:.0}% strokes", d().pump_level * 100.0) } else { "idle".into() })
                style=Signal::derive(move || format!("--pulse: {:.2}", d().pump_level))
                active=Signal::derive(move || d().pump_on) />
            <Link label=Signal::derive(move || format!("{} bar", num(d().pressure, 1))) flowing=flowing />
            <Stage name="Boiler"
                value=Signal::derive(move || num(d().water_c, 1))
                unit="°C"
                sub=Signal::derive(move || {
                    let mut s = format!("wall {}° · heat {:.0}%", num(d().wall_c, 0), d().heater_level * 100.0);
                    if let Some(st) = d().steam_flow.filter(|x| *x > 0.01) { s.push_str(&format!(" · steam {st:.1} g/s")); }
                    s
                })
                style=Signal::derive(move || format!("{}; --glow: {:.2}", tint(d().water_c), d().heater_level))
                active=Signal::derive(move || d().heater_on)
                badge=Signal::derive(move || d().water_ml.filter(|w| *w < 130.0).map(|w| format!("{w:.0} ml"))) />
            <Link label=Signal::derive(move || String::from(if valve.get() { "valve open" } else { "valve closed" })) flowing=valve warm=true valve=valve />
            <Stage name="Portafilter"
                value=Signal::derive(move || d().group_c.map(|g| format!("{}{g:.0}", if d().group_estimated { "≈" } else { "" })).unwrap_or_else(|| "–".into()))
                unit="°C"
                sub=Signal::derive(move || {
                    let what = match d().puck_wet {
                        Some(w) => format!("puck {:.0}% wet", w * 100.0),
                        None if valve.get() && d().state == "flushing" => "flushing".into(),
                        None if valve.get() => "brewing".into(),
                        None => "idle".into(),
                    };
                    match (d().group_estimated, what.as_str()) {
                        (true, "idle") => "estimated".into(),
                        (true, _) => format!("estimated · {what}"),
                        (false, _) => what,
                    }
                })
                style=Signal::derive(move || tint(d().group_c))
                active=Signal::derive(move || d().valve_open) />
            <Link label=Signal::derive(move || d().puck_flow.filter(|f| *f > 0.05).map(|f| format!("{f:.1} ml/s")).unwrap_or_default()) flowing=dripping warm=true />
            <Stage name="Cup"
                value=Signal::derive(move || num(d().cup_g, 1))
                unit=" g"
                sub=Signal::derive(move || String::from(if dripping.get() { "filling" } else { "on the scale" }))
                style=Signal::derive(String::new)
                active=dripping />
        </div>
    }
}
