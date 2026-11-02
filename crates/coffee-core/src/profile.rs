//! Shot profiles: a list of phases, each with a pressure or flow target, a
//! transition, an optional limit and exit conditions.
//!
//! ```json
//! {
//!   "name": "Classic 9 bar",
//!   "temperature_c": 93,
//!   "phases": [
//!     { "name": "fill", "target": { "flow": 6.0 }, "limit": 2.0,
//!       "exit": [ { "pressure_above": 1.5 }, { "time_s": 10 } ] },
//!     { "name": "ramp", "target": { "pressure": 9.0 },
//!       "transition": { "curve": "ease_in_out", "over_s": 4 }, "exit": [ { "time_s": 4 } ] },
//!     { "name": "extract", "target": { "pressure": 9.0 }, "limit": 3.5 }
//!   ],
//!   "stop": { "weight_g": 36, "max_time_s": 60 }
//! }
//! ```
//!
//! `limit` means *max flow (ml/s)* on a pressure phase and *max pressure (bar)* on
//! a flow phase.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Brew temperature; falls back to the engine config when absent.
    #[serde(default)]
    pub temperature_c: Option<f32>,
    pub phases: Vec<Phase>,
    #[serde(default)]
    pub stop: Stop,
    /// Expected mean puck resistance (bar per ml/s) for a well-dialled shot on
    /// this profile. Used for grind advice.
    #[serde(default)]
    pub target_resistance: Option<f32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Pressure(f32),
    Flow(f32),
}

impl Target {
    pub fn value(self) -> f32 {
        match self {
            Target::Pressure(v) | Target::Flow(v) => v,
        }
    }
    pub fn same_kind(self, other: Target) -> bool {
        matches!(
            (self, other),
            (Target::Pressure(_), Target::Pressure(_)) | (Target::Flow(_), Target::Flow(_))
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    #[default]
    Instant,
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Curve {
    /// Map progress 0..1 to eased progress 0..1.
    pub fn apply(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        match self {
            Curve::Instant => 1.0,
            Curve::Linear => x,
            Curve::EaseIn => x * x,
            Curve::EaseOut => 1.0 - (1.0 - x) * (1.0 - x),
            Curve::EaseInOut => x * x * (3.0 - 2.0 * x),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Transition {
    pub curve: Curve,
    pub over_s: f32,
    /// Start the ramp from the *measured* value at phase entry instead of the
    /// previous phase's setpoint.
    #[serde(default, skip_serializing_if = "core::ops::Not::not")]
    pub adaptive: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Exit {
    TimeS(f32),
    PressureAbove(f32),
    PressureBelow(f32),
    FlowAbove(f32),
    FlowBelow(f32),
    WeightAbove(f32),
    PumpedAbove(f32),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Phase {
    pub name: String,
    pub target: Target,
    #[serde(default)]
    pub transition: Transition,
    #[serde(default)]
    pub limit: Option<f32>,
    /// Any one of these ends the phase. Empty = runs until the shot stops.
    #[serde(default)]
    pub exit: Vec<Exit>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Stop {
    /// Stop when the (latency-compensated) cup weight reaches this.
    pub weight_g: Option<f32>,
    pub max_time_s: Option<f32>,
}

/// Inputs used to evaluate exit conditions.
#[derive(Clone, Copy, Debug, Default)]
pub struct Observed {
    pub phase_time_s: f32,
    pub pressure_bar: f32,
    pub flow_ml_s: f32,
    pub weight_g: Option<f32>,
    /// Water pumped since the phase started (ml).
    pub pumped_ml: f32,
}

impl Exit {
    pub fn met(self, o: &Observed) -> bool {
        match self {
            Exit::TimeS(t) => o.phase_time_s >= t,
            Exit::PressureAbove(p) => o.pressure_bar >= p,
            Exit::PressureBelow(p) => o.pressure_bar <= p,
            Exit::FlowAbove(f) => o.flow_ml_s >= f,
            Exit::FlowBelow(f) => o.flow_ml_s <= f,
            Exit::WeightAbove(w) => o.weight_g.is_some_and(|g| g >= w),
            Exit::PumpedAbove(v) => o.pumped_ml >= v,
        }
    }
}

impl Phase {
    /// Setpoint `t` seconds into the phase, starting from `start`.
    pub fn setpoint(&self, start: f32, t: f32) -> f32 {
        let end = self.target.value();
        let tr = self.transition;
        if tr.curve == Curve::Instant || tr.over_s <= 0.0 {
            return end;
        }
        start + (end - start) * tr.curve.apply(t / tr.over_s)
    }
}

impl Profile {
    pub fn classic_9bar() -> Self {
        serde_default_profile()
    }

    pub const FLUSH_NAME: &'static str = "Flush";

    /// Warm-up / cleaning flush: steady flow through an empty basket.
    pub fn flush(seconds: f32) -> Self {
        use alloc::string::ToString;
        Profile {
            name: Self::FLUSH_NAME.to_string(),
            description: "Water through the empty portafilter".to_string(),
            temperature_c: None,
            phases: alloc::vec![Phase {
                name: "flush".to_string(),
                target: Target::Flow(6.0),
                transition: Transition::default(),
                limit: Some(3.0),
                exit: alloc::vec![Exit::TimeS(seconds)],
            }],
            stop: Stop {
                weight_g: None,
                max_time_s: Some(seconds + 1.0),
            },
            target_resistance: None,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.phases.is_empty() {
            return Err("profile has no phases");
        }
        for p in &self.phases {
            let v = p.target.value();
            match p.target {
                Target::Pressure(_) if !(0.0..=12.0).contains(&v) => {
                    return Err("pressure target out of range 0..12 bar");
                }
                Target::Flow(_) if !(0.0..=12.0).contains(&v) => {
                    return Err("flow target out of range 0..12 ml/s");
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn serde_default_profile() -> Profile {
    use alloc::string::ToString;
    use alloc::vec;
    Profile {
        name: "Classic 9 bar".to_string(),
        description: "Gentle fill, ramp to 9 bar, hold. Stops at 36 g.".to_string(),
        temperature_c: Some(93.0),
        phases: vec![
            Phase {
                name: "fill".to_string(),
                target: Target::Flow(6.0),
                transition: Transition::default(),
                limit: Some(2.0),
                exit: vec![Exit::PressureAbove(1.8), Exit::TimeS(12.0)],
            },
            Phase {
                name: "soak".to_string(),
                target: Target::Pressure(2.0),
                transition: Transition::default(),
                limit: Some(1.0),
                exit: vec![Exit::TimeS(4.0), Exit::WeightAbove(1.0)],
            },
            Phase {
                name: "ramp".to_string(),
                target: Target::Pressure(9.0),
                transition: Transition {
                    curve: Curve::EaseInOut,
                    over_s: 4.0,
                    adaptive: false,
                },
                limit: None,
                exit: vec![Exit::TimeS(4.0)],
            },
            Phase {
                name: "extract".to_string(),
                target: Target::Pressure(9.0),
                transition: Transition::default(),
                limit: Some(4.0),
                exit: vec![],
            },
        ],
        stop: Stop {
            weight_g: Some(36.0),
            max_time_s: Some(60.0),
        },
        target_resistance: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_roundtrip_and_shape() {
        let p = Profile::classic_9bar();
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"flow\":6.0"));
        assert!(s.contains("\"pressure_above\":1.8"));
        let back: Profile = serde_json::from_str(&s).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn transition_curve() {
        let ph = Phase {
            name: "r".into(),
            target: Target::Pressure(9.0),
            transition: Transition {
                curve: Curve::Linear,
                over_s: 4.0,
                adaptive: false,
            },
            limit: None,
            exit: alloc::vec![],
        };
        assert_eq!(ph.setpoint(1.0, 0.0), 1.0);
        assert_eq!(ph.setpoint(1.0, 2.0), 5.0);
        assert_eq!(ph.setpoint(1.0, 10.0), 9.0);
    }
}
