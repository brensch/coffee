mod api;
mod chart;
mod diagram;
mod pages;

use api::{Store, store};
use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Brew,
    Profiles,
    History,
    Lab,
    Settings,
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let s = Store::new();
    provide_context(s);
    api::start_engine_stream(s);

    let initial = web_sys::window()
        .and_then(|w| w.location().hash().ok())
        .map(|h| match h.as_str() {
            "#profiles" => Page::Profiles,
            "#history" => Page::History,
            "#lab" | "#experiments" => Page::Lab,
            "#settings" => Page::Settings,
            _ => Page::Brew,
        })
        .unwrap_or(Page::Brew);
    let page = RwSignal::new(initial);
    provide_context(page);
    Effect::new(move |_| {
        let h = match page.get() {
            Page::Brew => "#machine",
            Page::Profiles => "#profiles",
            Page::History => "#history",
            Page::Lab => "#experiments",
            Page::Settings => "#settings",
        };
        if let Some(w) = web_sys::window() {
            let _ = w.location().set_hash(h);
        }
    });

    view! {
        <TopBar page=page />
        <main>
            {move || match page.get() {
                Page::Brew => pages::brew::BrewPage().into_any(),
                Page::Profiles => pages::profiles::ProfilesPage().into_any(),
                Page::History => pages::history::HistoryPage().into_any(),
                Page::Lab => pages::lab::LabPage().into_any(),
                Page::Settings => pages::settings::SettingsPage().into_any(),
            }}
        </main>
        <Toast />
    }
}

#[component]
fn TopBar(page: RwSignal<Page>) -> impl IntoView {
    let s = store();
    let tab = move |p: Page, label: &'static str| {
        view! {
            <button class="tab" class:active=move || page.get() == p on:click=move |_| page.set(p)>
                {label}
                {move || (p == Page::Lab && s.lab_connected.get()).then(|| view! { <span class="badge">"LIVE"</span> })}
            </button>
        }
    };
    let state = move || {
        s.tel
            .with(|t| {
                t.as_ref().map(|t| {
                    if t.state == "brewing" && t.profile == "Flush" {
                        "flushing".to_string()
                    } else {
                        t.state.clone()
                    }
                })
            })
            .unwrap_or_else(|| "offline".into())
    };
    view! {
        <header class="topbar">
            <div class="brand"><span class="bean"></span>"coffee"</div>
            <nav class="tabs">
                {tab(Page::Brew, "Machine")}
                {tab(Page::Profiles, "Profiles")}
                {tab(Page::History, "History")}
                {tab(Page::Lab, "Experiments")}
                {tab(Page::Settings, "Settings")}
            </nav>
            <span class=move || format!("pill {}", state())>{move || state().replace('_', " ")}</span>
            <div class="conn">
                <span class="dot" class:on=move || s.connected.get() class:bad=move || !s.connected.get()></span>
                {move || {
                    let emu = s.info.with(|i| i.as_ref().is_some_and(|i| i.emulated));
                    match (s.connected.get(), emu) {
                        (false, _) => "disconnected",
                        (true, true) => "emulated",
                        (true, false) => "machine",
                    }
                }}
            </div>
        </header>
    }
}

#[component]
fn Toast() -> impl IntoView {
    let s = store();
    view! {
        {move || s.toast.get().map(|(text, ok)| {
            let style = format!(
                "position:fixed;bottom:20px;left:50%;transform:translateX(-50%);z-index:50;padding:10px 16px;border-radius:12px;\
                 background:var(--surface);border:1px solid {};box-shadow:var(--shadow);max-width:90vw",
                if ok { "var(--border)" } else { "var(--bad)" }
            );
            view! { <div style=style>{text}</div> }
        })}
    }
}
