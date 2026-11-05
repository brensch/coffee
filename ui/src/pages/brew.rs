use crate::api::{self, Live, store};
use crate::chart::{Axis, Band, Chart, Line};
use crate::diagram::{Diagram, from_telemetry};
use coffee_api::Command;
use leptos::prelude::*;

fn fmt1(x: Option<f32>) -> String {
    // Avoid "-0.0" from a tared scale sitting at zero.
    x.map(|v| format!("{:.1}", if v.abs() < 0.05 { 0.0 } else { v }))
        .unwrap_or_else(|| "–".into())
}

/// "Flush to warm portafilter", shown whenever the estimate says it's too cold.
#[component]
fn FlushPrompt() -> impl IntoView {
    let s = store();
    view! {
        {move || cold_portafilter(s.tel.get().as_ref()).map(|note| view! {
            <button class="btn" title="Lock in the empty portafilter; 30 s of hot water warms it"
                on:click=move |_| api::command(Command::Flush { seconds: 30.0 }, "Flushing 30 s through the empty portafilter")>"Flush to warm portafilter"</button>
            <div class="muted small">{note}</div>
        })}
    }
}

/// Human description of a profile's grind (relative to your normal espresso grind).
pub fn grind_label(g: Option<f32>) -> String {
    match g {
        None => "Grind: not specified by this profile (start from your normal espresso grind)".into(),
        Some(g) => {
            let words = if g < 0.6 {
                "much coarser than normal"
            } else if g < 0.92 {
                "coarser than normal"
            } else if g <= 1.08 {
                "your normal espresso grind"
            } else if g <= 1.2 {
                "slightly finer than normal"
            } else {
                "finer than normal"
            };
            format!("Grind: {words} (×{g:.2})")
        }
    }
}

/// If the portafilter is still cold, how much cooler the first shot will land.
/// Fitted to the emulator (E24 model): ~0.2 C at the puck per C of portafilter
/// below ~78 C. Only as good as the model until calibrated (CALIBRATION.md #9).
fn cold_portafilter(t: Option<&coffee_api::Telemetry>) -> Option<String> {
    let t = t?;
    let g = t.group_c?;
    // Flushing while the boiler is still well short of temperature just pushes
    // cold water through: wait until the water is nearly there.
    if t.water_c.or(t.temp_c).is_none_or(|w| w < t.setpoint_c - 3.0) {
        return None;
    }
    if t.heat_soak >= 0.999 || t.heat_soak < 0.02 {
        return None;
    }
    // heat_soak = (g - room) / (warm - room)  =>  warm temperature.
    let warm = 22.0 + (g - 22.0) / t.heat_soak;
    let penalty = 0.2 * (warm - g);
    // Below ~1.5 C the difference is within shot-to-shot noise: stop nagging.
    (penalty >= 1.5).then(|| format!(
        "Portafilter ≈{g:.0}°C: a shot now would land ≈{penalty:.0}°C cooler at the puck. Flushing 30 s through the empty portafilter recovers most of that."
    ))
}

/// Why the machine isn't Ready yet, for the Brew button.
fn waiting_reason(t: Option<&coffee_api::Telemetry>) -> String {
    let Some(t) = t else {
        return "Connecting…".into();
    };
    let sp = t.setpoint_c;
    let water = t.water_c.or(t.temp_c).unwrap_or(0.0);
    if water < sp - 1.5 {
        format!("Heating water {water:.0} → {sp:.0}°C")
    } else if water > sp + 2.5 {
        format!("Settling {water:.1} → {sp:.0}°C")
    } else if t.heat_soak < 0.999 {
        let g = t.group_c.unwrap_or(0.0);
        let warm = if t.heat_soak > 0.02 {
            22.0 + (g - 22.0) / t.heat_soak
        } else {
            sp - 12.0
        };
        format!("Portafilter warming {g:.0} → {warm:.0}°C")
    } else {
        "Stabilising…".into()
    }
}

/// Phase bands from the recorded shot samples.
pub fn phase_bands(samples: &[(f64, u8)], names: &[String]) -> Vec<Band> {
    let mut out: Vec<Band> = vec![];
    for &(t, p) in samples {
        if p == 0xFF {
            continue;
        }
        let label = if p == 0xFE {
            "after stop".to_string()
        } else if p == 0xFD {
            "flush".to_string()
        } else {
            names
                .get(p as usize)
                .cloned()
                .unwrap_or_else(|| format!("phase {p}"))
        };
        match out.last_mut() {
            Some(b) if b.label == label => b.x1 = t,
            _ => out.push(Band {
                x0: t,
                x1: t,
                label,
            }),
        }
    }
    out
}

fn series(
    v: Signal<Vec<Live>>,
    x: fn(&Live) -> f64,
    y: fn(&Live) -> f64,
) -> Signal<Vec<(f64, f64)>> {
    Signal::derive(move || v.with(|v| v.iter().map(|s| (x(s), y(s))).collect()))
}

#[component]
pub fn BrewPage() -> impl IntoView {
    let s = store();
    let tel = s.tel;
    let field = move |f: fn(&coffee_api::Telemetry) -> Option<f32>| {
        move || tel.with(|t| t.as_ref().and_then(f))
    };

    let temp = field(|t| t.water_c.or(t.temp_c));
    let pressure = field(|t| t.pressure_bar);
    let flow = field(|t| Some(t.flow_ml_s));
    let weight = field(|t| t.weight_g);
    let active_profile =
        move || tel.with(|t| t.as_ref().map(|t| t.profile.clone()).unwrap_or_default());
    let profile = Memo::new(move |_| {
        let name = active_profile();
        s.profiles
            .with(|ps| ps.iter().find(|p| p.name == name).cloned())
    });
    let target_weight = move || profile.with(|p| p.as_ref().and_then(|p| p.stop.weight_g));

    // The chart is always live: the last 60 s.
    let live: Signal<Vec<Live>> = s.live.into();
    let xs = Signal::derive(move || {
        let end = live.with(|v| v.last().map(|l| l.t).unwrap_or(0.0));
        (end - 60.0, end)
    });
    let pick = move |f: fn(&Live) -> f64| {
        Signal::derive(move || live.with(|v| v.iter().map(|l| (l.t, f(l))).collect()))
    };
    // Shade the profile phases of a shot as it scrolls through.
    let bands = Signal::derive(move || {
        let names: Vec<String> = profile.with(|p| {
            p.as_ref()
                .map(|p| p.phases.iter().map(|ph| ph.name.clone()).collect())
                .unwrap_or_default()
        });
        live.with(|v| {
            phase_bands(
                &v.iter()
                    .map(|l| {
                        (
                            l.t,
                            if l.brewing {
                                l.phase.unwrap_or(0xFF)
                            } else {
                                0xFF
                            },
                        )
                    })
                    .collect::<Vec<_>>(),
                &names,
            )
        })
    });

    let temp_series = series(live, |l| l.t, |l| l.temp);
    let sp_series = series(live, |l| l.t, |l| l.setpoint);
    let temp_x = Signal::derive(move || {
        let end = live.with(|v| v.last().map(|l| l.t).unwrap_or(0.0));
        (end - 180.0, end)
    });

    view! {
        {move || {
            let faults = tel.with(|t| t.as_ref().map(|t| t.faults.clone()).unwrap_or_default());
            (!faults.is_empty()).then(|| view! {
                <div class="banner">
                    <b>"Fault"</b>
                    <div class="row">{faults.into_iter().map(|f| view! { <span class="chip bad">{f}</span> }).collect_view()}</div>
                    <span style="flex:1"></span>
                    <button class="btn small" on:click=move |_| api::command(Command::ClearFaults, "Faults cleared")>"Clear"</button>
                </div>
            })
        }}

        <div class="readouts">
            <div class="readout" style="--c: var(--temp)">
                <div class="label">"Boiler water (est.)"</div>
                <div class="value">{move || fmt1(temp())}<small>"°C"</small></div>
                <div class="sub">{move || tel.with(|t| t.as_ref().map(|t| {
                    let probe = t.temp_c.map(|p| format!("probe {p:.1}")).unwrap_or_else(|| "probe –".into());
                    if t.boost_c > 0.3 {
                        format!("target {:.1} · {probe} (+{:.0} boost)", t.setpoint_c, t.boost_c)
                    } else if t.heat_soak < 0.999 && t.state != "fault" {
                        format!("target {:.1} · group head {:.0}°C, still warming", t.setpoint_c, t.group_c.unwrap_or(0.0))
                    } else {
                        format!("target {:.1} · {probe}", t.setpoint_c)
                    }
                }).unwrap_or_default())}</div>
                <div class="bar"><i style:width=move || {
                    // Warm-up progress: water and group heat-soak together.
                    let (sp, soak) = tel.with(|t| t.as_ref().map(|t| (t.setpoint_c, t.heat_soak)).unwrap_or((93.0, 0.0)));
                    let water = ((temp().unwrap_or(22.0) - 22.0) / (sp - 22.0)).clamp(0.0, 1.0);
                    format!("{:.0}%", (water.min(1.0) * 0.5 + soak * 0.5) * 100.0)
                }></i></div>
            </div>
            <div class="readout" style="--c: var(--pressure)">
                <div class="label">"Pressure"</div>
                <div class="value">{move || fmt1(pressure())}<small>"bar"</small></div>
                <div class="sub">{move || tel.with(|t| t.as_ref().and_then(|t| t.target_pressure_bar).map(|p| format!("target {p:.1} bar")).unwrap_or_else(|| "–".into()))}</div>
                <div class="bar"><i style:width=move || format!("{:.0}%", (pressure().unwrap_or(0.0) / 12.0 * 100.0).clamp(0.0, 100.0))></i></div>
            </div>
            <div class="readout" style="--c: var(--flow)">
                <div class="label">"Flow"</div>
                <div class="value">{move || fmt1(flow())}<small>"ml/s"</small></div>
                <div class="sub">{move || tel.with(|t| t.as_ref().and_then(|t| t.target_flow_ml_s).map(|p| format!("target {p:.1} ml/s")).unwrap_or_else(|| "pump estimate".into()))}</div>
                <div class="bar"><i style:width=move || format!("{:.0}%", (flow().unwrap_or(0.0) / 8.0 * 100.0).clamp(0.0, 100.0))></i></div>
            </div>
            <div class="readout" style="--c: var(--weight)">
                <div class="label">"Cup"</div>
                <div class="value">{move || fmt1(weight())}<small>"g"</small></div>
                <div class="sub">{move || match target_weight() { Some(w) => format!("stop at {w:.0} g"), None => "no target".into() }}</div>
                <div class="bar"><i style:width=move || format!("{:.0}%", (weight().unwrap_or(0.0) / target_weight().unwrap_or(40.0) * 100.0).clamp(0.0, 100.0))></i></div>
            </div>
        </div>

        <div class="grid brew-layout">
            <div class="stack">
            <FrontPanel />
            <div class="card">
                <h2>"Machine"<span class="spacer"></span><span class="faint small">"live from telemetry"</span></h2>
                <Diagram data=from_telemetry(s) />
            </div>
            <div class="card">
                <h2>"Live"<span class="spacer"></span><span class="faint small">"last minute"</span></h2>
                <Chart
                    x=xs
                    y_left=(0.0, 12.0)
                    y_right=(0.0, 48.0)
                    height=320.0
                    bands=bands
                    lines=vec![
                        Line::new("var(--pressure)", Axis::Left, pick(|l| l.target_pressure)).dashed(),
                        Line::new("var(--flow)", Axis::Left, pick(|l| l.target_flow)).dashed(),
                        Line::new("var(--pressure)", Axis::Left, pick(|l| l.pressure)),
                        Line::new("var(--flow)", Axis::Left, pick(|l| l.flow)),
                        Line::new("var(--weight)", Axis::Right, pick(|l| l.weight)),
                    ]
                />
                <div class="legend">
                    <span><i style="--c: var(--pressure)"></i>"pressure (bar)"</span>
                    <span><i style="--c: var(--flow)"></i>"flow (ml/s)"</span>
                    <span><i style="--c: var(--weight)"></i>"weight (g, right)"</span>
                    <span><i class="dash" style="--c: var(--muted)"></i>"targets"</span>
                </div>
                <div style="height:12px"></div>
                <PhaseTrack />
            </div>
            <ShotList />
            </div>

            <div class="stack">
                <div class="card stack">
                    <div class="row spread">
                        <div>
                            <div class="muted small">"Shot time"</div>
                            <div class="num" style="font-size:44px;font-weight:600">
                                {move || tel.with(|t| t.as_ref().map(|t| format!("{:.1}", t.shot_time_s)).unwrap_or_else(|| "0.0".into()))}
                                <span class="muted" style="font-size:16px">" s"</span>
                            </div>
                        </div>
                        <div class="muted small" style="text-align:right">
                            {move || tel.with(|t| t.as_ref().and_then(|t| t.phase_name.clone())).unwrap_or_default()}
                        </div>
                    </div>
                    {move || {
                        let state = tel.with(|t| t.as_ref().map(|t| t.state.clone()).unwrap_or_default());
                        match state.as_str() {
                            "brewing" => view! { <button class="btn danger big-btn" on:click=move |_| api::command(Command::StopBrew, "")>"Stop"</button> }.into_any(),
                            "ready" => view! {
                                <div class="stack">
                                    <button class="btn primary big-btn" on:click=move |_| api::command(Command::StartBrew { force: false }, "")>"Brew"</button>
                                    <FlushPrompt />
                                </div>
                            }.into_any(),
                            "heating" | "boot" => view! {
                                <div class="stack">
                                    <button class="btn big-btn" disabled>{move || waiting_reason(tel.get().as_ref())}</button>
                                    <FlushPrompt />
                                    <button class="btn small ghost" title="Start a shot before the water is at temperature"
                                        on:click=move |_| api::command(Command::StartBrew { force: true }, "Brewing before ready")>"Brew anyway"</button>
                                </div>
                            }.into_any(),
                            "fault" => view! { <button class="btn big-btn" disabled>"Fault: see banner"</button> }.into_any(),
                            other => view! { <button class="btn big-btn" disabled>{other.replace('_', " ")}</button> }.into_any(),
                        }
                    }}
                    <label class="stack small">
                        <span class="muted">"Profile"</span>
                        <crate::picker::ProfilePicker
                            profiles=s.profiles
                            current=Signal::derive(active_profile)
                            on_pick=Callback::new(move |name: String| api::command(Command::SelectProfile { name }, "Profile selected"))
                        />
                    </label>
                    <div class="muted small">{move || profile.with(|p| p.as_ref().map(|p| p.description.clone())).unwrap_or_default()}</div>
                    <div class="grind-line">{move || profile.with(|p| p.as_ref().map(|p| grind_label(p.grind)))}</div>
                </div>

                <div class="card">
                    <h2>"Temperature · 3 min"</h2>
                    <Chart x=temp_x y_left=(20.0, 150.0) height=200.0 width=400.0 lines=vec![
                        Line::new("var(--muted)", Axis::Left, sp_series).dashed(),
                        Line::new("var(--temp)", Axis::Left, temp_series),
                    ] />
                    <div class="row small muted">
                        "heater "
                        <span class="num">{move || tel.with(|t| t.as_ref().map(|t| format!("{:.0}%", t.heater_duty * 100.0)).unwrap_or_default())}</span>
                        " · pump "
                        <span class="num">{move || tel.with(|t| t.as_ref().map(|t| format!("{:.0}%", t.pump_duty * 100.0)).unwrap_or_default())}</span>
                    </div>
                </div>

                <LastShot />
            </div>
        </div>
    }
}

/// Previous shots; clicking one opens it in History.
/// The machine's front panel as it looks on the Gaggia: three latching rockers (power, coffee,
/// steam), each with its lamp (now an LED the controller drives). The lamps show exactly what the
/// board drives on its LED pins (telemetry `lamps`, sent on every edge). On the emulator the rockers
/// are clickable: a click flips the emulated switch, which reaches the engine as the same debounced
/// input the real rocker gives (coffee on = start a shot, off = stop it). On real hardware they only
/// show the rockers' positions.
#[component]
fn FrontPanel() -> impl IntoView {
    let s = store();
    let panel = move || {
        let t = s.tel.get();
        let (sw, lamps) = match t.as_ref() {
            Some(t) => ([t.power_switch, t.brew_switch, t.steam_switch], t.lamps),
            None => ([false; 3], [false; 3]),
        };
        let lab = s.lab_connected.get();
        [("Power", "switch.power"), ("Coffee", "switch.brew"), ("Steam", "switch.steam")].into_iter().enumerate().map(|(i, (name, path))| {
            let on = sw[i];
            let lit = lamps[i];
            let class = match (on, lab) {
                (true, true) => "fp-rocker on click",
                (true, false) => "fp-rocker on",
                (false, true) => "fp-rocker click",
                (false, false) => "fp-rocker",
            };
            let title = match (on, lab) {
                (_, false) => if on { "switch on (the rocker on the machine)" } else { "switch off (the rocker on the machine)" },
                (true, true) => "on: click to switch off",
                (false, true) => "off: click to switch on",
            };
            view! {
                <div class="fp-unit">
                    <div class=if lit { "fp-led on" } else { "fp-led" } title=if lit { "lamp lit (LED pin high)" } else { "lamp off (LED pin low)" }></div>
                    <button class=class title=title disabled=!lab
                        on:click=move |_| api::lab_command(coffee_api::lab::LabCommand::Set { path: path.into(), value: if on { "off".into() } else { "on".into() } })>
                        <span class="fp-mark">{if on { "I" } else { "O" }}</span>
                    </button>
                    <div class="fp-label">{name}</div>
                </div>
            }
        }).collect_view()
    };
    view! {
        <div class="card">
            <h2>"Front panel"<span class="spacer"></span>
                <span class="faint small">{move || if s.lab_connected.get() { "emulator: click a switch to flip it" } else { "switches and lamps on the machine" }}</span>
            </h2>
            <div class="fp">{panel}</div>
        </div>
    }
}

#[component]
fn ShotList() -> impl IntoView {
    let s = store();
    let page = expect_context::<RwSignal<crate::Page>>();
    view! {
        <div class="card">
            <h2>"Previous shots"<span class="spacer"></span>
                <button class="btn small ghost" on:click=move |_| page.set(crate::Page::History)>"All in History →"</button>
            </h2>
            {move || {
                let shots = s.shots.get();
                if shots.is_empty() {
                    return view! { <div class="muted small">"No shots yet."</div> }.into_any();
                }
                view! {
                    <table>
                        <thead><tr><th>"#"</th><th>"Profile"</th><th>"Time"</th><th>"Yield"</th><th>"Peak"</th><th>"Puck R"</th></tr></thead>
                        <tbody>
                            {shots.into_iter().take(12).map(|m| {
                                let id = m.id;
                                view! {
                                    <tr class="clickable" on:click=move |_| { s.selected_shot.set(Some(id)); page.set(crate::Page::History); }>
                                        <td class="num">{m.id}</td>
                                        <td>{m.profile.clone()}</td>
                                        <td class="num">{format!("{:.1} s", m.duration_s)}</td>
                                        <td class="num">{m.weight_g.map(|w| format!("{w:.1} g")).unwrap_or_else(|| "–".into())}</td>
                                        <td class="num">{format!("{:.1} bar", m.peak_pressure_bar)}</td>
                                        <td class="num">{m.mean_resistance.map(|r| format!("{r:.2}")).unwrap_or_else(|| "–".into())}</td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn PhaseTrack() -> impl IntoView {
    let s = store();
    let phases = move || {
        let tel = s.tel.get();
        let name = tel.as_ref().map(|t| t.profile.clone()).unwrap_or_default();
        let current = tel.as_ref().and_then(|t| t.phase);
        let brewing = tel
            .as_ref()
            .is_some_and(|t| t.state == "brewing" || t.state == "brew_done");
        let names: Vec<String> = s.profiles.with(|ps| {
            ps.iter()
                .find(|p| p.name == name)
                .map(|p| p.phases.iter().map(|ph| ph.name.clone()).collect())
                .unwrap_or_default()
        });
        names
            .into_iter()
            .enumerate()
            .map(|(i, n)| {
                let cls = match current {
                    Some(c) if brewing && (c as usize) == i => "phase active",
                    Some(c) if brewing && (c as usize) > i => "phase done",
                    _ => "phase",
                };
                view! { <div class=cls><b>{n}</b>{format!("phase {}", i + 1)}</div> }
            })
            .collect_view()
    };
    view! { <div class="phases">{phases}</div> }
}

#[component]
fn LastShot() -> impl IntoView {
    let s = store();
    view! {
        {move || s.shots.with(|v| v.first().cloned()).map(|m| view! {
            <div class="card stack">
                <h2>{format!("Last shot · #{}", m.id)}</h2>
                <dl class="kv">
                    <dt>"Time"</dt><dd>{format!("{:.1} s", m.duration_s)}</dd>
                    <dt>"Yield"</dt><dd>{m.weight_g.map(|w| format!("{w:.1} g")).unwrap_or_else(|| "–".into())}</dd>
                    <dt>"Peak"</dt><dd>{format!("{:.1} bar", m.peak_pressure_bar)}</dd>
                    <dt>"Puck R"</dt><dd>{m.mean_resistance.map(|r| format!("{r:.2} bar·s/ml")).unwrap_or_else(|| "–".into())}</dd>
                    <dt>"Ended"</dt><dd>{m.end.replace('_', " ")}</dd>
                </dl>
                {match m.advice {
                    Some(a) => view! { <div class="advice">{a.text}</div> }.into_any(),
                    None => view! { <div class="muted small">"Liked this shot? Mark it as the grind reference in History to get dial-in advice."</div> }.into_any(),
                }}
            </div>
        })}
    }
}
