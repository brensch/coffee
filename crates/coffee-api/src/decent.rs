//! Import Decent Espresso profiles (the `.tcl` files in decentespresso/de1app,
//! `de1plus/profiles/`, GPL-3.0). Converts only the files the user supplies.
//!
//! Decent profiles come in three flavours:
//! * `settings_2a`: simple *pressure* profile (preinfusion, hold, decline)
//! * `settings_2b`: simple *flow* profile (preinfusion, hold, decline)
//! * `settings_2c`: advanced, an explicit list of steps (`advanced_shot`)
//!
//! Advanced step mapping:
//!
//! | Decent step field                        | coffee                                   |
//! |------------------------------------------|------------------------------------------|
//! | `pump flow` / `pump pressure` + value    | `Target::Flow` / `Target::Pressure`      |
//! | `transition smooth` (+ `seconds`)        | `Curve::Linear` over the step            |
//! | `max_flow_or_pressure` (> 0) + range     | `limit` = value + range / 2 (soft band)  |
//! | `seconds`                                | `Exit::TimeS`                            |
//! | `exit_if 1` + `exit_type` + value        | `Exit::{Pressure,Flow}{Above,Below}`     |
//! | `volume` (< 500)                         | `Exit::PumpedAbove`                      |
//! | `weight` (> 0)                           | `Exit::WeightAbove`                      |
//!
//! Not representable (reported as warnings): per-step temperatures (the first
//! step's temperature is used for the whole shot) and Decent's volume-based
//! shot stop.

use crate::{Curve, Exit, Profile, ProfilePhase, Stop, Target, Transition};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub struct Imported {
    pub profile: Profile,
    pub warnings: Vec<String>,
}

/// Split a Tcl list into its elements (braces group, quotes group).
pub fn tcl_list(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        match b[i] {
            b'{' => {
                let mut depth = 0usize;
                let start = i + 1;
                while i < b.len() {
                    match b[i] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        b'\\' => i += 1,
                        _ => {}
                    }
                    i += 1;
                }
                out.push(s[start..i.min(b.len())].to_string());
                i += 1;
            }
            b'"' => {
                let start = i + 1;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                out.push(s[start..i.min(b.len())].to_string());
                i += 1;
            }
            _ => {
                let start = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                out.push(s[start..i].to_string());
            }
        }
    }
    out
}

fn dict(s: &str) -> BTreeMap<String, String> {
    let v = tcl_list(s);
    v.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect()
}

fn num(d: &BTreeMap<String, String>, k: &str) -> f32 {
    d.get(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0)
}

fn phase(name: &str, target: Target, over_s: f32, smooth: bool, limit: f32, exits: Vec<Exit>) -> ProfilePhase {
    ProfilePhase {
        name: name.to_string(),
        target,
        transition: if smooth && over_s > 0.0 {
            Transition { curve: Curve::Linear, over_s, adaptive: false }
        } else {
            Transition::default()
        },
        limit: (limit > 0.0).then_some(limit),
        exit: exits,
    }
}

/// The beverage type, so callers can skip tea/cleaning/pour-over profiles.
pub fn beverage_type(file: &str) -> String {
    dict(file).get("beverage_type").cloned().unwrap_or_default()
}

pub fn convert(file: &str) -> Result<Imported, String> {
    let d = dict(file);
    let mut warnings = Vec::new();
    let title = d.get("profile_title").cloned().filter(|t| !t.is_empty()).ok_or("no profile_title")?;
    let kind = d.get("settings_profile_type").map(String::as_str).unwrap_or("settings_2c");
    let mut phases = Vec::new();
    let mut temp = num(&d, "espresso_temperature");
    let stop_weight;

    match kind {
        "settings_2a" | "settings_2b" => {
            let flow_profile = kind == "settings_2b";
            let pre_t = num(&d, "preinfusion_time");
            if pre_t > 0.0 {
                phases.push(phase(
                    "preinfusion",
                    Target::Flow(num(&d, "preinfusion_flow_rate")),
                    0.0,
                    false,
                    0.0,
                    alloc::vec![Exit::TimeS(pre_t), Exit::PressureAbove(num(&d, "preinfusion_stop_pressure"))],
                ));
            }
            let (hold_t, decline_t) = (num(&d, "espresso_hold_time"), num(&d, "espresso_decline_time"));
            let (hold, end, limit) = if flow_profile {
                (Target::Flow(num(&d, "flow_profile_hold")), Target::Flow(num(&d, "flow_profile_decline")), num(&d, "maximum_pressure"))
            } else {
                (Target::Pressure(num(&d, "espresso_pressure")), Target::Pressure(num(&d, "pressure_end")), num(&d, "maximum_flow"))
            };
            if hold_t > 0.0 {
                phases.push(phase(if flow_profile { "hold flow" } else { "rise and hold" }, hold, 0.0, false, limit, alloc::vec![Exit::TimeS(hold_t)]));
            }
            if decline_t > 0.0 {
                phases.push(phase("decline", end, decline_t, true, limit, alloc::vec![Exit::TimeS(decline_t)]));
            }
            stop_weight = num(&d, "final_desired_shot_weight");
        }
        _ => {
            let steps = tcl_list(d.get("advanced_shot").ok_or("advanced profile without steps")?);
            let mut temps = Vec::new();
            for (i, raw) in steps.iter().enumerate() {
                let s = dict(raw);
                let name = s.get("name").cloned().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("step {}", i + 1));
                let secs = num(&s, "seconds");
                let pressure_pump = s.get("pump").map(String::as_str) == Some("pressure");
                let target = if pressure_pump { Target::Pressure(num(&s, "pressure")) } else { Target::Flow(num(&s, "flow")) };
                let smooth = s.get("transition").map(String::as_str) == Some("smooth");
                let mut exits = alloc::vec![Exit::TimeS(secs.max(0.5))];
                if num(&s, "exit_if") > 0.5 {
                    match s.get("exit_type").map(String::as_str) {
                        Some("pressure_over") => exits.push(Exit::PressureAbove(num(&s, "exit_pressure_over"))),
                        Some("pressure_under") => exits.push(Exit::PressureBelow(num(&s, "exit_pressure_under"))),
                        Some("flow_over") => exits.push(Exit::FlowAbove(num(&s, "exit_flow_over"))),
                        Some("flow_under") => exits.push(Exit::FlowBelow(num(&s, "exit_flow_under"))),
                        other => warnings.push(format!("step `{name}`: exit type {other:?} ignored")),
                    }
                }
                let vol = num(&s, "volume");
                if vol > 0.0 && vol < 500.0 {
                    exits.push(Exit::PumpedAbove(vol));
                }
                let w = num(&s, "weight");
                if w > 0.0 {
                    exits.push(Exit::WeightAbove(w));
                }
                temps.push(num(&s, "temperature"));
                // Decent's limiter is soft: it acts across `max_flow_or_pressure_range`
                // above the limit (Extractamundo's 1.0 ml/s limit with a 3.0 range is
                // documented to end at 2.5-4 ml/s). Our limit is a hard cap, so use the
                // middle of that band.
                let lim = num(&s, "max_flow_or_pressure");
                let lim = if lim > 0.0 { ((lim + num(&s, "max_flow_or_pressure_range") / 2.0) * 10.0 + 0.5) as i32 as f32 / 10.0 } else { 0.0 };
                phases.push(phase(&name, target, secs, smooth, lim, exits));
            }
            if let Some(&t0) = temps.first() {
                if t0 > 0.0 {
                    temp = t0;
                }
                if temps.iter().any(|t| *t > 0.0 && (t - t0).abs() > 0.5) {
                    warnings.push(String::from("per-step temperatures differ; using the first step's for the whole shot"));
                }
            }
            stop_weight = num(&d, "final_desired_shot_weight_advanced");
        }
    }
    if phases.is_empty() {
        return Err(String::from("profile has no steps"));
    }
    // Volume stop: advanced profiles use *_advanced; simple ones only fall back
    // to volume when no weight is set.
    let volume = match kind {
        "settings_2a" | "settings_2b" if stop_weight <= 0.0 => num(&d, "final_desired_shot_volume"),
        "settings_2a" | "settings_2b" => 0.0,
        _ => num(&d, "final_desired_shot_volume_advanced"),
    };
    let count_from = num(&d, "final_desired_shot_volume_advanced_count_start") as usize;
    let n_phases = phases.len();
    let total: f32 = phases.iter().filter_map(|p| p.exit.iter().find_map(|e| if let Exit::TimeS(t) = e { Some(*t) } else { None })).sum();
    let notes = d.get("profile_notes").cloned().unwrap_or_default();
    let description = {
        let first = notes.split(". ").next().unwrap_or("").trim();
        if first.len() > 220 { format!("{}…", &first[..first.char_indices().nth(217).map(|(i, _)| i).unwrap_or(first.len())]) } else { first.to_string() }
    };
    let profile = Profile {
        name: title,
        description,
        temperature_c: (temp > 70.0 && temp < 105.0).then_some(temp),
        phases,
        stop: Stop {
            weight_g: (stop_weight > 0.0).then_some(stop_weight),
            max_time_s: Some((total + 10.0).min(120.0)),
            volume_ml: (volume > 0.0).then_some(volume),
            volume_from_phase: (volume > 0.0 && count_from > 0 && count_from < n_phases).then_some(count_from as u8),
        },
        target_resistance: None,
        grind: None,
        author: d.get("author").cloned().filter(|a| !a.is_empty()),
    };
    profile.validate().map_err(String::from)?;
    Ok(Imported { profile, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcl_lists() {
        assert_eq!(tcl_list("a {b c} \"d e\" {f {g h}}"), alloc::vec!["a", "b c", "d e", "f {g h}"]);
    }

    #[test]
    fn simple_pressure_profile() {
        let f = "settings_profile_type settings_2a\nprofile_title {Classic Italian espresso}\nauthor Decent\nbeverage_type espresso\n\
                 preinfusion_time 8\npreinfusion_flow_rate 8.0\npreinfusion_stop_pressure 4\nespresso_pressure 9.0\n\
                 espresso_hold_time 35\nespresso_decline_time 0\npressure_end 6.0\nmaximum_flow 4.5\nespresso_temperature 94\nfinal_desired_shot_weight 36\n";
        let p = convert(f).unwrap().profile;
        assert_eq!(p.phases.len(), 2);
        assert_eq!(p.phases[0].target, Target::Flow(8.0));
        assert!(p.phases[0].exit.contains(&Exit::PressureAbove(4.0)));
        assert_eq!(p.phases[1].target, Target::Pressure(9.0));
        assert_eq!(p.phases[1].limit, Some(4.5));
        assert_eq!(p.stop.weight_g, Some(36.0));
        assert_eq!(p.author.as_deref(), Some("Decent"));
    }

    #[test]
    fn advanced_profile() {
        let f = "settings_profile_type settings_2c\nprofile_title {Blooming Espresso}\nbeverage_type espresso\n\
                 advanced_shot {{exit_if 1 flow 4 volume 500 transition fast temperature 97.5 name preinfusion pressure 1 pump flow exit_type pressure_over exit_pressure_over 4.00 seconds 23} \
                 {exit_if 0 flow 2.2 volume 500 transition smooth temperature 92 name {ramp} pressure 4 pump flow seconds 5 max_flow_or_pressure 8.6}}\n\
                 final_desired_shot_weight_advanced 42\n";
        let imp = convert(f).unwrap();
        let p = imp.profile;
        assert_eq!(p.phases.len(), 2);
        assert!(p.phases[0].exit.contains(&Exit::PressureAbove(4.0)));
        assert_eq!(p.phases[1].transition.curve, Curve::Linear);
        assert_eq!(p.phases[1].limit, Some(8.6));
        assert_eq!(p.temperature_c, Some(97.5));
        assert_eq!(p.stop.weight_g, Some(42.0));
        assert_eq!(imp.warnings.len(), 1); // differing temperatures
    }

    #[test]
    fn limiter_range_is_a_soft_band() {
        // Extractamundo Dos!: 6 bar with a 1.0 ml/s limiter over a 3.0 range, documented
        // to end at 2.5-4 ml/s. A hard 1.0 ml/s cap would make that impossible.
        let f = "settings_profile_type settings_2c\nprofile_title {X}\nbeverage_type espresso\n\
                 advanced_shot {{exit_if 0 flow 2.2 volume 100 max_flow_or_pressure_range 3.0 transition fast temperature 90 name {6 bar} pressure 6.0 pump pressure seconds 60 max_flow_or_pressure 1.0}}\n";
        let p = convert(f).unwrap().profile;
        assert_eq!(p.phases[0].limit, Some(2.5));
    }
}
