//! Fuzzy, alphabetical profile picker.

use coffee_api::Profile;
use leptos::prelude::*;

/// Case-insensitive fuzzy match: every whitespace-separated term in `query`
/// must appear in `haystack` as a subsequence (so "blm esp" finds
/// "Blooming Espresso", "dec" finds anything by Decent).
pub fn fuzzy(query: &str, haystack: &str) -> bool {
    let hay: Vec<char> = haystack.to_lowercase().chars().collect();
    query.to_lowercase().split_whitespace().all(|term| {
        let mut it = hay.iter();
        term.chars().all(|c| it.any(|h| *h == c))
    })
}

/// Sort key: alphabetical, ignoring case and leading punctuation.
pub fn alpha_key(name: &str) -> String {
    name.trim_start_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

fn searchable(p: &Profile) -> String {
    format!("{} {}", p.name, p.author.as_deref().unwrap_or(""))
}

/// Profiles matching `query`, alphabetical.
pub fn filtered(profiles: &[Profile], query: &str) -> Vec<Profile> {
    let mut v: Vec<Profile> = profiles.iter().filter(|p| query.trim().is_empty() || fuzzy(query, &searchable(p))).cloned().collect();
    v.sort_by_key(|p| alpha_key(&p.name));
    v
}

#[component]
pub fn ProfilePicker(
    #[prop(into)] profiles: Signal<Vec<Profile>>,
    #[prop(into)] current: Signal<String>,
    on_pick: Callback<String>,
) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let open = RwSignal::new(false);
    let hi = RwSignal::new(0usize);
    let matches = Memo::new(move |_| profiles.with(|ps| filtered(ps, &query.get())));

    let pick = move |name: String| {
        on_pick.run(name);
        query.set(String::new());
        open.set(false);
    };

    view! {
        <div class="picker">
            <input
                class="picker-input"
                type="search"
                placeholder=move || if current.get().is_empty() { "Search profiles…".to_string() } else { current.get() }
                prop:value=move || query.get()
                on:focus=move |_| { open.set(true); hi.set(0); }
                on:blur=move |_| open.set(false)
                on:input=move |ev| { query.set(event_target_value(&ev)); open.set(true); hi.set(0); }
                on:keydown=move |ev| {
                    let n = matches.with(|m| m.len());
                    match ev.key().as_str() {
                        "ArrowDown" => { ev.prevent_default(); open.set(true); if n > 0 { hi.update(|h| *h = (*h + 1).min(n - 1)); } }
                        "ArrowUp" => { ev.prevent_default(); hi.update(|h| *h = h.saturating_sub(1)); }
                        "Enter" => {
                            ev.prevent_default();
                            if let Some(p) = matches.with(|m| m.get(hi.get_untracked()).cloned()) { pick(p.name); }
                        }
                        "Escape" => { query.set(String::new()); open.set(false); }
                        _ => {}
                    }
                }
            />
            <Show when=move || open.get()>
                <div class="picker-list">
                    {move || {
                        let m = matches.get();
                        if m.is_empty() {
                            return view! { <div class="picker-empty">"No profiles match"</div> }.into_any();
                        }
                        m.into_iter().enumerate().map(|(i, p)| {
                            let name = p.name.clone();
                            let is_current = current.with(|c| *c == p.name);
                            view! {
                                // mousedown (not click) so it fires before the input's blur closes the list
                                <div class="picker-item" class:hi=move || hi.get() == i class:current=is_current
                                    on:mousedown=move |ev| { ev.prevent_default(); pick(name.clone()); }
                                    on:mouseenter=move |_| hi.set(i)>
                                    <span class="picker-name">{p.name.clone()}</span>
                                    <span class="picker-meta">{p.author.clone().unwrap_or_default()}</span>
                                </div>
                            }
                        }).collect_view().into_any()
                    }}
                </div>
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuzzy_terms() {
        assert!(fuzzy("blm esp", "Blooming Espresso Decent"));
        assert!(fuzzy("DEC", "Londonium Decent"));
        assert!(!fuzzy("turbo xyz", "TurboBloom Decent"));
        assert_eq!(alpha_key("80's Espresso"), "80's espresso");
        assert_eq!(alpha_key("\"Quoted\""), "quoted\"");
    }
}
