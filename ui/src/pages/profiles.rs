use crate::api::{self, store};
use crate::chart::{Axis, Band, Chart, Line};
use coffee_api::ProfilePhase as Phase;
use coffee_api::{Ack, Profile};
use leptos::prelude::*;
use leptos::task::spawn_local;

fn exits_to_text(p: &Phase) -> String {
    use coffee_api::Exit::*;
    p.exit
        .iter()
        .map(|e| match e {
            TimeS(v) => format!("time_s {v}"),
            PressureAbove(v) => format!("pressure_above {v}"),
            PressureBelow(v) => format!("pressure_below {v}"),
            FlowAbove(v) => format!("flow_above {v}"),
            FlowBelow(v) => format!("flow_below {v}"),
            WeightAbove(v) => format!("weight_above {v}"),
            PumpedAbove(v) => format!("pumped_above {v}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn text_to_exits(s: &str) -> Result<Vec<coffee_api::Exit>, String> {
    use coffee_api::Exit::*;
    let mut out = vec![];
    for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let mut it = part.split_whitespace();
        let k = it.next().unwrap_or_default();
        let v: f32 = it
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| format!("`{part}`: expected `kind value`"))?;
        out.push(match k {
            "time_s" | "time" => TimeS(v),
            "pressure_above" => PressureAbove(v),
            "pressure_below" => PressureBelow(v),
            "flow_above" => FlowAbove(v),
            "flow_below" => FlowBelow(v),
            "weight_above" => WeightAbove(v),
            "pumped_above" => PumpedAbove(v),
            _ => return Err(format!("unknown exit `{k}`")),
        });
    }
    Ok(out)
}

/// Approximate the setpoint curve for preview: phases last as long as their
/// time exit (or 8 s if they exit on a sensor condition).
/// Setpoint curves for the target preview chart.
#[derive(Clone, PartialEq)]
struct Preview {
    pressure: Vec<(f64, f64)>,
    flow: Vec<(f64, f64)>,
    bands: Vec<Band>,
    end_s: f64,
}

fn preview(p: &Profile) -> Preview {
    let mut pres = vec![];
    let mut flow = vec![];
    let mut bands = vec![];
    let mut t = 0.0f64;
    let mut last_p = 0.0f64;
    let mut last_f = 0.0f64;
    let n = p.phases.len();
    for (i, ph) in p.phases.iter().enumerate() {
        let dur = ph
            .exit
            .iter()
            .find_map(|e| {
                if let coffee_api::Exit::TimeS(v) = e {
                    Some(*v as f64)
                } else {
                    None
                }
            })
            .unwrap_or(if i + 1 == n {
                p.stop
                    .max_time_s
                    .map(|m| (m as f64 - t).max(8.0))
                    .unwrap_or(20.0)
            } else {
                8.0
            });
        let steps = 40;
        let (start, is_p) = match ph.target {
            coffee_api::Target::Pressure(_) => (last_p, true),
            coffee_api::Target::Flow(_) => (last_f, false),
        };
        for k in 0..=steps {
            let dt = dur * k as f64 / steps as f64;
            let v = ph.setpoint(start as f32, dt as f32) as f64;
            if is_p {
                pres.push((t + dt, v));
                flow.push((t + dt, f64::NAN));
            } else {
                flow.push((t + dt, v));
                pres.push((t + dt, f64::NAN));
            }
        }
        if is_p {
            last_p = ph.target.value() as f64;
        } else {
            last_f = ph.target.value() as f64;
        }
        bands.push(Band {
            x0: t,
            x1: t + dur,
            label: ph.name.clone(),
        });
        t += dur;
    }
    Preview {
        pressure: pres,
        flow,
        bands,
        end_s: t.max(10.0),
    }
}

#[component]
pub fn ProfilesPage() -> impl IntoView {
    let s = store();
    api::refresh_profiles();
    let selected = RwSignal::new(None::<String>);
    let draft = RwSignal::new(None::<Profile>);

    Effect::new(move |_| {
        if draft.with(|d| d.is_none())
            && let Some(first) = s.profiles.with(|p| p.first().cloned())
        {
            selected.set(Some(first.name.clone()));
            draft.set(Some(first));
        }
    });

    let pick = move |p: Profile| {
        selected.set(Some(p.name.clone()));
        draft.set(Some(p));
    };

    let save = move |_| {
        let Some(p) = draft.get_untracked() else {
            return;
        };
        if let Err(e) = p.validate() {
            s.notify(e, false);
            return;
        }
        spawn_local(async move {
            match api::send::<_, Ack>("PUT", "/api/profile", &p).await {
                Ok(a) if a.ok => {
                    s.notify(format!("Saved and activated “{}”", p.name), true);
                    api::refresh_profiles_for(s);
                }
                Ok(a) => s.notify(a.error.unwrap_or_default(), false),
                Err(e) => s.notify(e, false),
            }
        });
    };

    let pv = Memo::new(move |_| draft.with(|d| d.as_ref().map(preview)));
    let pres = Signal::derive(move || {
        pv.with(|p| p.as_ref().map(|p| p.pressure.clone()).unwrap_or_default())
    });
    let flow =
        Signal::derive(move || pv.with(|p| p.as_ref().map(|p| p.flow.clone()).unwrap_or_default()));
    let bands = Signal::derive(move || {
        pv.with(|p| p.as_ref().map(|p| p.bands.clone()).unwrap_or_default())
    });
    let xr = Signal::derive(move || {
        (
            0.0,
            pv.with(|p| p.as_ref().map(|p| p.end_s).unwrap_or(30.0)),
        )
    });

    let edit = move |f: Box<dyn Fn(&mut Profile)>| {
        draft.update(|d| {
            if let Some(d) = d.as_mut() {
                f(d)
            }
        })
    };

    view! {
        <div class="grid" style="grid-template-columns: minmax(220px, 280px) minmax(0, 1fr)">
            <div class="card stack">
                <h2>"Library"</h2>
                {move || s.profiles.get().into_iter().map(|p| {
                    let name = p.name.clone();
                    let n2 = name.clone();
                    let desc = p.description.clone();
                    view! {
                        <div class="list-item" class:active=move || selected.get().as_deref() == Some(n2.as_str()) on:click=move |_| pick(p.clone())>
                            <div style="font-weight:600">{name}</div>
                            <div class="muted small">{desc}</div>
                        </div>
                    }
                }).collect_view()}
                <button class="btn small" on:click=move |_| {
                    let mut p = Profile::classic_9bar();
                    p.name = "New profile".into();
                    p.description = String::new();
                    selected.set(Some(p.name.clone()));
                    draft.set(Some(p));
                }>"+ New profile"</button>
            </div>

            <div class="stack">
                {move || draft.get().is_some().then(|| view! {
                    <div class="card stack">
                        <h2>"Editor"<span class="spacer"></span>
                            <button class="btn primary small" on:click=save>"Save & activate"</button>
                        </h2>
                        <div class="grid cols-3">
                            <label class="stack small"><span class="muted">"Name"</span>
                                <input prop:value=move || draft.with(|d| d.as_ref().map(|d| d.name.clone()).unwrap_or_default())
                                    on:change=move |ev| { let v = event_target_value(&ev); edit(Box::new(move |d| d.name = v.clone())) } />
                            </label>
                            <label class="stack small"><span class="muted">"Brew temp °C"</span>
                                <input type="number" step="0.5" prop:value=move || draft.with(|d| d.as_ref().and_then(|d| d.temperature_c).map(|v| v.to_string()).unwrap_or_default())
                                    on:change=move |ev| { let v = event_target_value(&ev).parse().ok(); edit(Box::new(move |d| d.temperature_c = v)) } />
                            </label>
                            <label class="stack small"><span class="muted">"Stop at weight g"</span>
                                <input type="number" step="0.5" prop:value=move || draft.with(|d| d.as_ref().and_then(|d| d.stop.weight_g).map(|v| v.to_string()).unwrap_or_default())
                                    on:change=move |ev| { let v = event_target_value(&ev).parse().ok(); edit(Box::new(move |d| d.stop.weight_g = v)) } />
                            </label>
                            <label class="stack small"><span class="muted">"Max time s"</span>
                                <input type="number" prop:value=move || draft.with(|d| d.as_ref().and_then(|d| d.stop.max_time_s).map(|v| v.to_string()).unwrap_or_default())
                                    on:change=move |ev| { let v = event_target_value(&ev).parse().ok(); edit(Box::new(move |d| d.stop.max_time_s = v)) } />
                            </label>
                            <label class="stack small"><span class="muted">"Target puck R (bar·s/ml)"</span>
                                <input type="number" step="0.1" prop:value=move || draft.with(|d| d.as_ref().and_then(|d| d.target_resistance).map(|v| v.to_string()).unwrap_or_default())
                                    on:change=move |ev| { let v = event_target_value(&ev).parse().ok(); edit(Box::new(move |d| d.target_resistance = v)) } />
                            </label>
                            <label class="stack small"><span class="muted">"Description"</span>
                                <input prop:value=move || draft.with(|d| d.as_ref().map(|d| d.description.clone()).unwrap_or_default())
                                    on:change=move |ev| { let v = event_target_value(&ev); edit(Box::new(move |d| d.description = v.clone())) } />
                            </label>
                        </div>

                        <div class="phase-editor">
                            <span class="hdr">"Phase"</span><span class="hdr">"Target"</span><span class="hdr">"Value"</span>
                            <span class="hdr">"Curve"</span><span class="hdr">"Over s"</span><span class="hdr">"Limit"</span>
                            <span class="hdr">"Exit when (any)"</span><span></span>
                            {move || {
                                let n = draft.with(|d| d.as_ref().map(|d| d.phases.len()).unwrap_or(0));
                                (0..n).map(|i| phase_row(draft, i)).collect_view()
                            }}
                        </div>
                        <div class="row">
                            <button class="btn small" on:click=move |_| edit(Box::new(|d| {
                                let mut ph = d.phases.last().cloned().unwrap_or_else(|| Profile::classic_9bar().phases[0].clone());
                                ph.name = format!("phase {}", d.phases.len() + 1);
                                d.phases.push(ph);
                            }))>"+ Add phase"</button>
                            <span class="muted small">"Limit = max flow (ml/s) on a pressure phase, max pressure (bar) on a flow phase. Exits: time_s, pressure_above/below, flow_above/below, weight_above, pumped_above."</span>
                        </div>
                    </div>

                    <div class="card">
                        <h2>"Target preview"</h2>
                        <Chart x=xr y_left=(0.0, 12.0) height=220.0 bands=bands lines=vec![
                            Line::new("var(--pressure)", Axis::Left, pres),
                            Line::new("var(--flow)", Axis::Left, flow),
                        ] />
                        <div class="legend">
                            <span><i style="--c: var(--pressure)"></i>"pressure target (bar)"</span>
                            <span><i style="--c: var(--flow)"></i>"flow target (ml/s)"</span>
                            <span class="faint">"Sensor-based exits are drawn as 8 s."</span>
                        </div>
                    </div>
                })}
            </div>
        </div>
    }
}

fn phase_row(draft: RwSignal<Option<Profile>>, i: usize) -> impl IntoView {
    let get = move |f: fn(&Phase) -> String| {
        move || {
            draft.with(|d| {
                d.as_ref()
                    .and_then(|d| d.phases.get(i))
                    .map(f)
                    .unwrap_or_default()
            })
        }
    };
    let set = move |f: Box<dyn Fn(&mut Phase)>| {
        draft.update(|d| {
            if let Some(ph) = d.as_mut().and_then(|d| d.phases.get_mut(i)) {
                f(ph)
            }
        })
    };
    let s = store();
    view! {
        <input prop:value=get(|p| p.name.clone()) on:change=move |ev| { let v = event_target_value(&ev); set(Box::new(move |p| p.name = v.clone())) } />
        <select prop:value=get(|p| match p.target { coffee_api::Target::Pressure(_) => "pressure".into(), _ => "flow".into() })
            on:change=move |ev| { let v = event_target_value(&ev); set(Box::new(move |p| {
                let x = p.target.value();
                p.target = if v == "pressure" { coffee_api::Target::Pressure(x) } else { coffee_api::Target::Flow(x) };
            })) }>
            <option value="pressure">"pressure"</option>
            <option value="flow">"flow"</option>
        </select>
        <input type="number" step="0.1" prop:value=get(|p| p.target.value().to_string())
            on:change=move |ev| { let v: f32 = event_target_value(&ev).parse().unwrap_or(0.0); set(Box::new(move |p| {
                p.target = match p.target { coffee_api::Target::Pressure(_) => coffee_api::Target::Pressure(v), _ => coffee_api::Target::Flow(v) };
            })) } />
        <select prop:value=get(|p| format!("{:?}", p.transition.curve))
            on:change=move |ev| { let v = event_target_value(&ev); set(Box::new(move |p| {
                use coffee_api::Curve::*;
                p.transition.curve = match v.as_str() { "Linear" => Linear, "EaseIn" => EaseIn, "EaseOut" => EaseOut, "EaseInOut" => EaseInOut, _ => Instant };
            })) }>
            <option value="Instant">"instant"</option>
            <option value="Linear">"linear"</option>
            <option value="EaseIn">"ease in"</option>
            <option value="EaseOut">"ease out"</option>
            <option value="EaseInOut">"ease in-out"</option>
        </select>
        <input type="number" step="0.5" prop:value=get(|p| p.transition.over_s.to_string())
            on:change=move |ev| { let v = event_target_value(&ev).parse().unwrap_or(0.0); set(Box::new(move |p| p.transition.over_s = v)) } />
        <input type="number" step="0.1" placeholder="none" prop:value=get(|p| p.limit.map(|l| l.to_string()).unwrap_or_default())
            on:change=move |ev| { let v = event_target_value(&ev).parse().ok(); set(Box::new(move |p| p.limit = v)) } />
        <input prop:value=get(exits_to_text) placeholder="e.g. time_s 4, pressure_above 1.8"
            on:change=move |ev| match text_to_exits(&event_target_value(&ev)) {
                Ok(v) => set(Box::new(move |p| p.exit = v.clone())),
                Err(e) => s.notify(e, false),
            } />
        <button class="btn small ghost" title="Remove phase" on:click=move |_| draft.update(|d| if let Some(d) = d.as_mut() && i < d.phases.len() { d.phases.remove(i); })>"✕"</button>
    }
}
