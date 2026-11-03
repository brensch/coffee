//! Emulator lab: ground truth, machine diagram, fault injection, scenarios.

use crate::api::{self, Store, store};
use crate::chart::{Axis, Chart, Line};
use crate::diagram::{Diagram, from_truth};
use coffee_api::lab::LabCommand;
use leptos::prelude::*;

fn set(path: &str, value: impl ToString) {
    api::lab_command(LabCommand::Set {
        path: path.into(),
        value: value.to_string(),
    });
}

#[component]
pub fn LabPage() -> impl IntoView {
    let s = store();
    view! {
        {move || if s.lab_connected.get() {
            view! { <LabConnected /> }.into_any()
        } else {
            view! {
                <div class="card empty stack">
                    <div style="font-size:18px;color:var(--text)">"No emulator connected"</div>
                    <div>"Experiments drive the emulated machine: inject faults, run scenarios, tune the physics, and compare ground truth with what the engine believes."</div>
                    <div>"Start the live emulator and the UI will find it automatically:"</div>
                    <pre class="mono" style="text-align:left;display:inline-block;margin:0 auto;padding:12px 16px;background:var(--raised);border-radius:10px">
"cargo build --release
./target/release/coffee-emu serve --spawn-engine"
                    </pre>
                    <div class="small">"Looking for the lab API at "<span class="mono">{move || s.lab_url.get().unwrap_or_else(|| "http://<host>:8081".into())}</span></div>
                </div>
            }.into_any()
        }}
    }
}

fn hist_series(s: Store, name: &'static str) -> Signal<Vec<(f64, f64)>> {
    Signal::derive(move || {
        let Some(i) = s.lab_info.with(|li| {
            li.as_ref()
                .and_then(|li| li.signals.iter().position(|x| x.name == name))
        }) else {
            return vec![];
        };
        s.lab_hist.with(|h| {
            h.iter()
                .map(|tr| (tr.t, tr.values.get(i).copied().unwrap_or(f64::NAN)))
                .collect()
        })
    })
}

#[component]
fn LabConnected() -> impl IntoView {
    let s = store();
    let speed = move || {
        s.lab_truth
            .with(|t| t.as_ref().map(|t| t.speed).unwrap_or(1.0))
    };
    let paused = move || s.lab_truth.with(|t| t.as_ref().is_some_and(|t| t.paused));
    let engine_on = move || {
        s.lab_truth
            .with(|t| t.as_ref().is_some_and(|t| t.engine_connected))
    };
    let simt = move || s.lab_truth.with(|t| t.as_ref().map(|t| t.t).unwrap_or(0.0));

    let xr = Signal::derive(move || {
        let end = simt();
        (end - 120.0, end)
    });
    let eng_temp =
        Signal::derive(move || s.live.with(|l| l.iter().map(|x| (x.t, x.temp)).collect()));
    let eng_p = Signal::derive(move || {
        s.live
            .with(|l| l.iter().map(|x| (x.t, x.pressure)).collect())
    });
    let eng_flow =
        Signal::derive(move || s.live.with(|l| l.iter().map(|x| (x.t, x.flow)).collect()));

    view! {
        <div class="card">
            <div class="row spread">
                <div class="row">
                    <span class="lab-tag">"LAB"</span>
                    <b>{move || s.lab_info.with(|i| i.as_ref().map(|i| i.model_name.clone())).unwrap_or_default()}</b>
                    <span class="muted num">{move || {
                        let t = simt();
                        format!("t = {:02}:{:04.1}", (t / 60.0) as u32, t % 60.0)
                    }}</span>
                    <span class="chip">{move || if engine_on() { "engine linked" } else { "engine unplugged" }}</span>
                    {move || (s.lab_val("wand.open") > 0.5).then(|| view! {
                        <button class="btn small danger" title="Water is escaping through the steam wand: pressure can't build"
                            on:click=|_| set("wand.open", 0)>"⚠ steam wand open: close"</button>
                    })}
                    {move || (s.lab_val("portafilter.attached") < 0.5).then(|| view! {
                        <button class="btn small danger" on:click=|_| set("portafilter.attached", 1)>"⚠ portafilter off: attach"</button>
                    })}
                    {move || (s.lab_val("puck.present") < 0.5).then(|| view! {
                        <button class="btn small danger" on:click=|_| set("puck.present", 1)>"⚠ no coffee in basket: add"</button>
                    })}
                </div>
                <div class="row">
                    {move || if paused() {
                        view! { <button class="btn small primary" on:click=|_| api::lab_command(LabCommand::Resume)>"▶ Resume"</button> }.into_any()
                    } else {
                        view! { <button class="btn small" on:click=|_| api::lab_command(LabCommand::Pause)>"⏸ Pause"</button> }.into_any()
                    }}
                    {[1.0f32, 5.0, 20.0, 100.0].into_iter().map(|x| view! {
                        <button class="btn small ghost" class:on=move || (speed() - x).abs() < 0.01
                            on:click=move |_| api::lab_command(LabCommand::Speed { x })>{format!("{x}×")}</button>
                    }).collect_view()}
                    <GrindMatch />
                    <button class="btn small danger" on:click=|_| api::lab_command(LabCommand::Reset)>"Reset machine"</button>
                </div>
            </div>
        </div>

        <div class="grid cols-2">
            <div class="card">
                <h2>"Machine"<span class="spacer"></span><span class="faint small">"ground truth"</span></h2>
                <Diagram data=from_truth(s) />
                <div class="row" style="margin-top:8px">
                    <Toggle label="Brew switch" path="switch.brew" signal="switch.brew" />
                    <Toggle label="Steam switch" path="switch.steam" signal="switch.steam" />
                    <button class="btn small" on:click=|_| set("wand.open", 1)>"Open wand"</button>
                    <button class="btn small" on:click=|_| set("wand.open", 0)>"Close wand"</button>
                    <button class="btn small" on:click=|_| set("puck.fresh", 1)>"Fresh puck"</button>
                    <button class="btn small" on:click=|_| set("cup.mass", 0)>"Empty cup"</button>
                </div>
            </div>
            <div class="stack">
                <div class="card">
                    <h2>"Temperature: truth vs what the engine sees"</h2>
                    <Chart x=xr y_left=(20.0, 150.0) height=190.0 lines=vec![
                        Line::new("var(--water)", Axis::Left, hist_series(s, "boiler.water_temp")),
                        Line::new("var(--weight)", Axis::Left, hist_series(s, "brew.water_temp")),
                        Line::new("var(--muted)", Axis::Left, hist_series(s, "boiler.temp")).dashed(),
                        Line::new("var(--temp)", Axis::Left, eng_temp),
                    ] />
                    <div class="legend">
                        <span><i style="--c: var(--water)"></i>"water (truth)"</span>
                        <span><i style="--c: var(--weight)"></i>"at the puck (truth)"</span>
                        <span><i class="dash" style="--c: var(--muted)"></i>"boiler body (truth)"</span>
                        <span><i style="--c: var(--temp)"></i>"engine water estimate"</span>
                    </div>
                </div>
                <div class="card">
                    <h2>"Hydraulics"</h2>
                    <Chart x=xr y_left=(0.0, 12.0) height=190.0 lines=vec![
                        Line::new("var(--muted)", Axis::Left, hist_series(s, "hydraulics.pressure")).dashed(),
                        Line::new("var(--pressure)", Axis::Left, eng_p),
                        Line::new("var(--flow)", Axis::Left, hist_series(s, "puck.flow")),
                        Line::new("var(--muted)", Axis::Left, eng_flow).dashed(),
                    ] />
                    <div class="legend">
                        <span><i class="dash" style="--c: var(--muted)"></i>"true pressure / est. pump flow"</span>
                        <span><i style="--c: var(--pressure)"></i>"engine pressure"</span>
                        <span><i style="--c: var(--flow)"></i>"puck flow (truth)"</span>
                    </div>
                </div>
            </div>
        </div>

        <div class="grid cols-2">
            <div class="card">
                <h2>"Faults & knobs"</h2>
                <Knobs />
            </div>
            <div class="card stack">
                <h2>"Scenarios"</h2>
                <Scenarios />
            </div>
        </div>

        <div class="grid cols-2">
            <div class="card">
                <h2>"Model parameters"</h2>
                <Params />
            </div>
            <div class="card">
                <h2>"All signals"</h2>
                <div class="truth-grid">
                    {move || {
                        let names: Vec<(String, String)> = s.lab_info.with(|i| i.as_ref().map(|i| i.signals.iter().map(|x| (x.name.clone(), x.unit.clone())).collect()).unwrap_or_default());
                        let vals = s.lab_truth.with(|t| t.as_ref().map(|t| t.values.clone()).unwrap_or_default());
                        names.into_iter().zip(vals).map(|((n, u), v)| view! {
                            <div><span>{n}</span><span class="num">{if v.is_finite() { format!("{v:.2} {u}") } else { "–".into() }}</span></div>
                        }).collect_view()
                    }}
                </div>
            </div>
        </div>
    }
}

/// Emulated coffee grind vs what the selected profile wants.
#[component]
fn GrindMatch() -> impl IntoView {
    let s = store();
    let wanted = move || {
        let name = s.tel.with(|t| t.as_ref().map(|t| t.profile.clone()).unwrap_or_default());
        s.profiles.with(|ps| ps.iter().find(|p| p.name == name).and_then(|p| p.grind))
    };
    view! {
        <span class="chip" title="Grind of the emulated coffee (puck resistance multiplier)">
            {move || format!("coffee grind ×{:.2}", s.lab_val("puck.grind"))}
        </span>
        <button class="btn small" class:on=move || s.barista_grind.get()
            title="When on, the emulated barista regrinds whenever you pick a profile"
            on:click=move |_| s.barista_grind.update(|b| *b = !*b)>
            {move || if s.barista_grind.get() { "Auto-grind: on" } else { "Auto-grind: off" }}
        </button>
        {move || wanted().map(|g| {
            let differs = (s.lab_val("puck.grind") - g as f64).abs() > 0.01;
            if differs {
                view! {
                    <button class="btn small" on:click=move |_| set("puck.grind", g)>{format!("Match profile grind (×{g:.2})")}</button>
                }
                .into_any()
            } else {
                view! { <span class="chip" style="color: var(--ok)">"✓ matches profile grind"</span> }.into_any()
            }
        })}
    }
}

#[component]
fn Toggle(label: &'static str, path: &'static str, signal: &'static str) -> impl IntoView {
    let s = store();
    let on = move || s.lab_val(signal) > 0.5;
    view! {
        <button class="btn small" class:on=on on:click=move |_| set(path, if on() { 0 } else { 1 })>
            {label}": "{move || if on() { "on" } else { "off" }}
        </button>
    }
}

#[component]
fn Knobs() -> impl IntoView {
    let s = store();
    view! {
        <div class="knobs">
            {move || s.lab_info.get().map(|i| i.knobs.into_iter().filter(|k| !k.path.starts_with("switch.")).map(|k| {
                let path = k.path.clone();
                if k.hint.contains('|') {
                    let opts: Vec<String> = k.hint.split('|').map(|x| x.trim().to_string()).collect();
                    view! {
                        <div class="knob">
                            <span class="k">{path.clone()}</span>
                            <div class="row">
                                {opts.into_iter().map(|o| {
                                    let p = path.clone();
                                    let o2 = o.clone();
                                    view! { <button class="btn small" on:click=move |_| set(&p, &o2)>{o}</button> }
                                }).collect_view()}
                            </div>
                        </div>
                    }.into_any()
                } else if k.hint.contains("action") {
                    view! {
                        <div class="knob">
                            <span class="k">{path.clone()}</span>
                            <button class="btn small" on:click=move |_| set(&path, 1)>"Trigger"</button>
                        </div>
                    }.into_any()
                } else {
                    let val = RwSignal::new(String::new());
                    view! {
                        <div class="knob">
                            <span class="k">{path.clone()}</span>
                            <div class="row">
                                <input style="flex:1;min-width:0" placeholder=k.hint.clone() prop:value=move || val.get() on:input=move |ev| val.set(event_target_value(&ev)) />
                                <button class="btn small" on:click=move |_| set(&path, val.get_untracked())>"Set"</button>
                            </div>
                        </div>
                    }.into_any()
                }
            }).collect_view())}
        </div>
    }
}

#[component]
fn Scenarios() -> impl IntoView {
    let s = store();
    let text = RwSignal::new(String::from(
        "name \"ad-hoc: probe falls off mid-shot\"\nduration 3min\nat 1s: brew on\nat 15s: set ntc.fault = detached\nat 40s: brew off\nexpect always boiler.temp < 172\n",
    ));
    view! {
        <div class="row">
            {move || s.lab_info.get().map(|i| i.scenarios.into_iter().map(|n| {
                let n2 = n.clone();
                view! { <button class="btn small" on:click=move |_| api::lab_command(LabCommand::RunNamedScenario { name: n2.clone() })>{n}</button> }
            }).collect_view())}
        </div>
        <textarea class="code" style="min-height:120px" prop:value=move || text.get() on:input=move |ev| text.set(event_target_value(&ev))></textarea>
        <div class="row">
            <button class="btn small primary" on:click=move |_| api::lab_command(LabCommand::RunScenario { text: text.get_untracked() })>"Run script"</button>
            <button class="btn small ghost" on:click=move |_| s.lab_events.set(vec![])>"Clear log"</button>
        </div>
        <div class="events">
            {move || s.lab_events.get().into_iter().rev().map(|(t, e, k)| {
                let cls = match k { 1 => "ev pass", -1 => "ev fail", _ => "ev" };
                view! { <div class=cls>{format!("{t:8.2}s  ")}<b>{e}</b></div> }
            }).collect_view()}
        </div>
    }
}

#[component]
fn Params() -> impl IntoView {
    let s = store();
    let filter = RwSignal::new(String::from("puck"));
    view! {
        <input style="width:100%" placeholder="filter, e.g. thermal" prop:value=move || filter.get() on:input=move |ev| filter.set(event_target_value(&ev)) />
        <div class="stack" style="margin-top:10px;max-height:320px;overflow:auto">
            {move || {
                let f = filter.get();
                s.lab_info.with(|i| i.as_ref().map(|i| i.params.iter().filter(|p| p.contains(&f)).take(40).cloned().collect::<Vec<_>>()).unwrap_or_default())
                    .into_iter().map(|p| {
                        let val = RwSignal::new(String::new());
                        let p2 = p.clone();
                        view! {
                            <div class="row">
                                <span class="mono small" style="flex:1">{p}</span>
                                <input style="width:110px" placeholder="value" prop:value=move || val.get() on:input=move |ev| val.set(event_target_value(&ev)) />
                                <button class="btn small" on:click=move |_| set(&p2, val.get_untracked())>"Set"</button>
                            </div>
                        }
                    }).collect_view()
            }}
        </div>
    }
}
