//! Import GaggiMate profiles (schema: github.com/jniebuhr/gaggimate `schema/profile.json`).
//!
//! GaggiMate profiles are CC BY-NC-SA 4.0 (as is that project); this module only
//! converts files the user supplies. Mapping:
//!
//! | GaggiMate                         | coffee                                     |
//! |-----------------------------------|--------------------------------------------|
//! | `pump.target = pressure`          | `Target::Pressure`, `flow` (>0) -> `limit` |
//! | `pump.target = flow`              | `Target::Flow`, `pressure` (>0) -> `limit` |
//! | `pump` = integer percent          | flow target = % of the pump's 0-bar flow   |
//! | `duration`                        | `Exit::TimeS`                              |
//! | `transition.type/duration`        | `Transition { curve, over_s }`             |
//! | `targets[]` volumetric/ratio      | `Exit::WeightAbove` (ratio x dose)         |
//! | `targets[]` pressure/flow gte/lte | `Exit::{Pressure,Flow}{Above,Below}`       |
//! | `targets[]` pumped                | `Exit::PumpedAbove` (per phase in both)    |
//!
//! Not representable (reported as warnings): `valve: 0` (closed-valve phases,
//! e.g. backflush), per-phase temperatures that differ from the profile's, and
//! `-1` "maintain current" sentinels (approximated by the previous setpoint).

use crate::{Curve, Exit, Profile, ProfilePhase, Stop, Target, Transition};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use serde::Deserialize;

/// Flow of an Ulka E5 at 0 bar (ml/s), used to translate "pump percent".
const PUMP_MAX_FLOW_ML_S: f32 = 650.0 / 60.0;

#[derive(Deserialize)]
pub struct GmProfile {
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub dose: Option<f32>,
    #[serde(default)]
    pub utility: bool,
    pub phases: Vec<GmPhase>,
}

#[derive(Deserialize)]
pub struct GmPhase {
    pub name: String,
    #[serde(default)]
    pub valve: Option<i32>,
    pub duration: f32,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub transition: Option<GmTransition>,
    pub pump: GmPump,
    #[serde(default)]
    pub targets: Vec<GmTarget>,
}

#[derive(Deserialize)]
pub struct GmTransition {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub adaptive: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum GmPump {
    Simple(f32),
    Advanced {
        target: String,
        #[serde(default)]
        pressure: f32,
        #[serde(default)]
        flow: f32,
    },
}

#[derive(Deserialize)]
pub struct GmTarget {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub operator: Option<String>,
    pub value: f32,
}

pub struct Imported {
    pub profile: Profile,
    pub warnings: Vec<String>,
}

pub fn convert(gm: &GmProfile) -> Result<Imported, String> {
    let mut warnings = Vec::new();
    if gm.utility {
        warnings.push(String::from(
            "utility profile (flush/backflush): imported for reference only",
        ));
    }
    let dose = gm.dose.unwrap_or(18.0);
    let mut phases = Vec::new();
    let (mut last_p, mut last_f) = (0.0f32, 0.0f32);
    let mut total = 0.0f32;
    for ph in &gm.phases {
        if ph.valve == Some(0) {
            warnings.push(format!(
                "phase `{}`: closed-valve phases aren't supported; valve stays open",
                ph.name
            ));
        }
        if ph
            .temperature
            .is_some_and(|t| t > 0.0 && Some(t) != gm.temperature)
        {
            warnings.push(format!(
                "phase `{}`: per-phase temperature ignored",
                ph.name
            ));
        }
        let (target, limit) = match &ph.pump {
            GmPump::Simple(pct) => {
                warnings.push(format!(
                    "phase `{}`: pump {pct}% mapped to a flow target",
                    ph.name
                ));
                (
                    Target::Flow(pct.clamp(0.0, 100.0) / 100.0 * PUMP_MAX_FLOW_ML_S),
                    None,
                )
            }
            GmPump::Advanced {
                target,
                pressure,
                flow,
            } => {
                let keep = |v: f32, last: f32, what: &str, w: &mut Vec<String>| {
                    if v < 0.0 {
                        w.push(format!("phase `{}`: \"maintain current {what}\" approximated by the previous setpoint", ph.name));
                        last
                    } else {
                        v
                    }
                };
                if target == "flow" {
                    let f = keep(*flow, last_f, "flow", &mut warnings);
                    (Target::Flow(f), (*pressure > 0.0).then_some(*pressure))
                } else {
                    let p = keep(*pressure, last_p, "pressure", &mut warnings);
                    (Target::Pressure(p), (*flow > 0.0).then_some(*flow))
                }
            }
        };
        match target {
            Target::Pressure(v) => last_p = v,
            Target::Flow(v) => last_f = v,
        }
        let transition = match &ph.transition {
            None => Transition::default(),
            Some(t) => {
                let curve = match t.kind.as_str() {
                    "linear" => Curve::Linear,
                    "ease-in" => Curve::EaseIn,
                    "ease-out" => Curve::EaseOut,
                    "ease-in-out" => Curve::EaseInOut,
                    _ => Curve::Instant,
                };
                Transition {
                    curve,
                    over_s: t.duration.min(ph.duration),
                    adaptive: t.adaptive,
                }
            }
        };
        let mut exit = alloc::vec![Exit::TimeS(ph.duration)];
        for t in &ph.targets {
            let lte = t.operator.as_deref() == Some("lte");
            let e = match (t.kind.as_str(), lte) {
                ("volumetric", false) => Some(Exit::WeightAbove(t.value)),
                ("ratio", false) => Some(Exit::WeightAbove(t.value * dose)),
                ("pressure", false) => Some(Exit::PressureAbove(t.value)),
                ("pressure", true) => Some(Exit::PressureBelow(t.value)),
                ("flow", false) => Some(Exit::FlowAbove(t.value)),
                ("flow", true) => Some(Exit::FlowBelow(t.value)),
                ("pumped", false) => Some(Exit::PumpedAbove(t.value)),
                (k, _) => {
                    warnings.push(format!("phase `{}`: target `{k}` ignored", ph.name));
                    None
                }
            };
            exit.extend(e);
        }
        total += ph.duration;
        phases.push(ProfilePhase {
            name: ph.name.clone(),
            target,
            transition,
            limit,
            exit,
        });
    }
    let profile = Profile {
        name: gm.label.clone(),
        description: if gm.description.is_empty() {
            String::from("Imported from GaggiMate")
        } else {
            gm.description.clone()
        },
        temperature_c: gm.temperature.filter(|t| *t > 0.0),
        phases,
        stop: Stop {
            weight_g: None,
            max_time_s: Some(total + 5.0),
            volume_ml: None,
            volume_from_phase: None,
        },
        target_resistance: None,
        grind: None,
        author: Some(String::from("GaggiMate")),
    };
    profile.validate().map_err(String::from)?;
    Ok(Imported { profile, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;

    #[test]
    fn converts_a_pro_profile() {
        let json = r#"{
          "label": "Test lever", "type": "pro", "temperature": 92, "dose": 18,
          "phases": [
            { "name": "Fill", "phase": "preinfusion", "valve": 1, "duration": 10,
              "pump": { "target": "flow", "pressure": 3, "flow": 6 },
              "targets": [ { "type": "pressure", "operator": "gte", "value": 2.5 } ] },
            { "name": "Peak", "phase": "brew", "valve": 1, "duration": 30,
              "transition": { "type": "ease-in-out", "duration": 4, "adaptive": true },
              "pump": { "target": "pressure", "pressure": 9, "flow": 0 },
              "targets": [ { "type": "ratio", "value": 2 } ] }
          ]
        }"#;
        let gm: GmProfile = serde_json_core_free(json);
        let imp = convert(&gm).unwrap();
        let p = &imp.profile;
        assert_eq!(p.phases[0].target, Target::Flow(6.0));
        assert_eq!(p.phases[0].limit, Some(3.0));
        assert!(p.phases[0].exit.contains(&Exit::PressureAbove(2.5)));
        assert_eq!(p.phases[1].transition.curve, Curve::EaseInOut);
        assert!(p.phases[1].exit.contains(&Exit::WeightAbove(36.0)));
        assert!(imp.warnings.is_empty(), "{:?}", imp.warnings);
        assert!(p.phases[1].transition.adaptive);
    }

    fn serde_json_core_free(s: &str) -> GmProfile {
        serde_json::from_str(s).unwrap()
    }
}
