use crate::api::{self, store};
use coffee_api::{Ack, Command, EngineConfig};
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn SettingsPage() -> impl IntoView {
    let s = store();
    let cfg_text = RwSignal::new(String::new());
    let brew = RwSignal::new(String::new());
    let steam = RwSignal::new(String::new());

    let load = move || {
        spawn_local(async move {
            match api::get::<EngineConfig>("/api/config").await {
                Ok(c) => {
                    brew.set(format!("{}", c.brew_temp_c));
                    steam.set(format!("{}", c.steam_temp_c));
                    cfg_text.set(serde_json::to_string_pretty(&c).unwrap_or_default());
                }
                Err(e) => s.notify(e, false),
            }
        });
    };
    load();

    let save_cfg = move |_| {
        let txt = cfg_text.get_untracked();
        match serde_json::from_str::<EngineConfig>(&txt) {
            Ok(c) => spawn_local(async move {
                match api::send::<_, Ack>("PUT", "/api/config", &c).await {
                    Ok(a) if a.ok => s.notify("Config applied", true),
                    Ok(a) => s.notify(a.error.unwrap_or_default(), false),
                    Err(e) => s.notify(e, false),
                }
            }),
            Err(e) => s.notify(format!("Invalid JSON: {e}"), false),
        }
    };

    view! {
        <div class="grid cols-2">
            <div class="card stack">
                <h2>"Temperatures"</h2>
                <div class="row">
                    <label class="stack small" style="flex:1"><span class="muted">"Brew °C (overrides profile)"</span>
                        <input type="number" step="0.5" prop:value=move || brew.get() on:input=move |ev| brew.set(event_target_value(&ev)) />
                    </label>
                    <button class="btn" on:click=move |_| match brew.get_untracked().parse() {
                        Ok(c) => api::command(Command::SetBrewTemp { c }, "Brew temperature set"),
                        Err(_) => s.notify("not a number", false),
                    }>"Set"</button>
                </div>
                <div class="row">
                    <label class="stack small" style="flex:1"><span class="muted">"Steam °C"</span>
                        <input type="number" step="1" prop:value=move || steam.get() on:input=move |ev| steam.set(event_target_value(&ev)) />
                    </label>
                    <button class="btn" on:click=move |_| match steam.get_untracked().parse() {
                        Ok(c) => api::command(Command::SetSteamTemp { c }, "Steam temperature set"),
                        Err(_) => s.notify("not a number", false),
                    }>"Set"</button>
                </div>
                <h2 style="margin-top:12px">"Machine"</h2>
                <div class="row">
                    <button class="btn" on:click=move |_| api::command(Command::ClearFaults, "Faults cleared")>"Clear latched faults"</button>
                </div>
                {move || s.info.get().map(|i| view! {
                    <dl class="kv small">
                        <dt>"Engine"</dt><dd>{format!("{} {}", i.name, i.version)}</dd>
                        <dt>"Platform"</dt><dd>{i.platform}</dd>
                        <dt>"Emulated"</dt><dd>{if i.emulated { "yes" } else { "no" }}</dd>
                        <dt>"Lab"</dt><dd>{move || s.lab_url.get().unwrap_or_else(|| "–".into())}</dd>
                    </dl>
                })}
            </div>
            <div class="card stack">
                <h2>"Engine config"<span class="spacer"></span>
                    <button class="btn small ghost" on:click=move |_| load()>"Reload"</button>
                    <button class="btn small primary" on:click=save_cfg>"Apply"</button>
                </h2>
                <p class="muted small" style="margin:0">"PID gains, sensor calibration, safety limits and pump curve. Applied immediately (not while brewing)."</p>
                <textarea class="code" prop:value=move || cfg_text.get() on:input=move |ev| cfg_text.set(event_target_value(&ev))></textarea>
            </div>
        </div>
    }
}
