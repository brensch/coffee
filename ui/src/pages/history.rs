use crate::api::{self, store};
use crate::chart::{Axis, Chart, Line};
use crate::pages::brew::phase_bands;
use coffee_api::Shot;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn HistoryPage() -> impl IntoView {
    let s = store();
    api::refresh_shots();
    let open = RwSignal::new(None::<Shot>);

    // Only the most recently requested shot may land (avoids a slower request
    // for a different shot overwriting the one you clicked).
    let wanted = RwSignal::new(None::<u32>);
    let load = move |id: u32| {
        wanted.set(Some(id));
        spawn_local(async move {
            match api::get::<Shot>(&format!("/api/shots/{id}")).await {
                Ok(shot) if wanted.get_untracked() == Some(id) => open.set(Some(shot)),
                Ok(_) => {}
                Err(e) => s.notify(e, false),
            }
        });
    };
    Effect::new(move |_| {
        // A shot picked on the Machine page wins; otherwise open the latest.
        if let Some(id) = s.selected_shot.get() {
            s.selected_shot.set(None);
            load(id);
        } else if open.with_untracked(|o| o.is_none())
            && wanted.get_untracked().is_none()
            && let Some(id) = s.shots.with(|v| v.first().map(|m| m.id))
        {
            load(id);
        }
    });

    let pts = move |f: fn(&coffee_api::ShotSample) -> Option<f32>| {
        Signal::derive(move || {
            open.with(|o| {
                o.as_ref()
                    .map(|s| {
                        s.samples
                            .iter()
                            .map(|x| (x.t as f64, f(x).map(|v| v as f64).unwrap_or(f64::NAN)))
                            .collect()
                    })
                    .unwrap_or_default()
            })
        })
    };
    let xr = Signal::derive(move || {
        (
            0.0,
            open.with(|o| {
                o.as_ref()
                    .and_then(|s| s.samples.last())
                    .map(|l| l.t as f64 + 2.0)
                    .unwrap_or(30.0)
            }),
        )
    });
    let bands = Signal::derive(move || {
        open.with(|o| {
            o.as_ref()
                .map(|s| {
                    let dur = s.meta.duration_s;
                    let pts: Vec<(f64, u8)> = s
                        .samples
                        .iter()
                        .map(|x| (x.t as f64, if x.t > dur + 0.05 { 0xFE } else { x.phase }))
                        .collect();
                    phase_bands(&pts, &s.phases)
                })
                .unwrap_or_default()
        })
    });

    view! {
        <div class="grid cols-2">
            <div class="card">
                <h2>"Shots"<span class="spacer"></span><button class="btn small ghost" on:click=move |_| api::refresh_shots()>"Refresh"</button></h2>
                {move || {
                    let shots = s.shots.get();
                    if shots.is_empty() {
                        return view! { <div class="empty">"No shots yet. Pull one!"</div> }.into_any();
                    }
                    view! {
                        <table>
                            <thead><tr><th>"#"</th><th>"Profile"</th><th>"Time"</th><th>"Yield"</th><th>"Peak"</th><th>"Advice"</th></tr></thead>
                            <tbody>
                                {shots.into_iter().map(|m| {
                                    let id = m.id;
                                    view! {
                                        <tr class="clickable" class:selected=move || open.with(|o| o.as_ref().is_some_and(|s| s.meta.id == id)) on:click=move |_| load(id)>
                                            <td class="num">{m.id}</td>
                                            <td>{m.profile.clone()}</td>
                                            <td class="num">{format!("{:.1}s", m.duration_s)}</td>
                                            <td class="num">{m.weight_g.map(|w| format!("{w:.1}g")).unwrap_or_else(|| "–".into())}</td>
                                            <td class="num">{format!("{:.1}", m.peak_pressure_bar)}</td>
                                            <td class="small muted">{m.advice.as_ref().map(|a| a.text.clone()).unwrap_or_default()}</td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    }.into_any()
                }}
            </div>
            <div class="card">
                {move || open.get().map(|shot| {
                    let m = shot.meta.clone();
                    view! {
                        <h2>{format!("Shot #{} · {}", m.id, m.profile)}</h2>
                        <dl class="kv">
                            <dt>"Duration"</dt><dd>{format!("{:.1} s", m.duration_s)}</dd>
                            <dt>"Yield"</dt><dd>{m.weight_g.map(|w| format!("{w:.1} g")).unwrap_or_else(|| "–".into())}</dd>
                            <dt>"Water pumped"</dt><dd>{format!("{:.0} ml", m.pumped_ml)}</dd>
                            <dt>"Mean puck resistance"</dt><dd>{m.mean_resistance.map(|r| format!("{r:.2} bar·s/ml")).unwrap_or_else(|| "–".into())}</dd>
                            <dt>"Ended by"</dt><dd>{m.end.replace('_', " ")}</dd>
                        </dl>
                        {m.advice.clone().map(|a| view! { <div class="advice" style="margin-top:10px">{a.text}</div> })}
                        <div class="row" style="margin-top:10px">
                            <button class="btn small" disabled=m.mean_resistance.is_none() on:click=move |_| {
                                api::command(coffee_api::Command::SetReferenceShot { id: m.id }, "Reference set: later shots on this profile get grind advice");
                                api::refresh_profiles();
                            }>"Use as grind reference"</button>
                        </div>
                    }
                })}
                <div style="height:10px"></div>
                <Chart x=xr y_left=(0.0, 12.0) y_right=(0.0, 48.0) height=280.0 bands=bands lines=vec![
                    Line::new("var(--pressure)", Axis::Left, pts(|x| x.target_pressure)).dashed(),
                    Line::new("var(--flow)", Axis::Left, pts(|x| x.target_flow)).dashed(),
                    Line::new("var(--pressure)", Axis::Left, pts(|x| Some(x.pressure))),
                    Line::new("var(--flow)", Axis::Left, pts(|x| Some(x.flow))),
                    Line::new("var(--weight)", Axis::Right, pts(|x| x.weight)),
                ] />
                <div class="legend">
                    <span><i style="--c: var(--pressure)"></i>"pressure"</span>
                    <span><i style="--c: var(--flow)"></i>"flow"</span>
                    <span><i style="--c: var(--weight)"></i>"weight (right)"</span>
                </div>
            </div>
        </div>
    }
}
