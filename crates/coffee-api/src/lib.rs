//! The one API every host speaks.
//!
//! Engine API (served by `coffee-engine` on a PC and by the ESP32 firmware):
//!
//! | Method | Path               | Body / Response                              |
//! |--------|--------------------|----------------------------------------------|
//! | GET    | `/api/info`        | [`Info`]                                     |
//! | GET    | `/api/status`      | [`Telemetry`]                                |
//! | WS     | `/api/ws`          | stream of [`ServerMsg`]                      |
//! | POST   | `/api/command`     | [`Command`] -> [`Ack`]                       |
//! | GET    | `/api/profile`     | active [`Profile`]                           |
//! | PUT    | `/api/profile`     | [`Profile`] -> [`Ack`]                       |
//! | GET    | `/api/profiles`    | `Vec<Profile>` (library)                     |
//! | GET    | `/api/config`      | [`EngineConfig`]                             |
//! | PUT    | `/api/config`      | [`EngineConfig`] -> [`Ack`]                  |
//! | GET    | `/api/shots`       | `Vec<ShotMeta>`                              |
//! | GET    | `/api/shots/{id}`  | [`Shot`]                                     |
//! | GET    | `/api/shot/current`| [`Shot`] in progress (404 when idle)         |
//! | GET    | `/api/telemetry/history?seconds=N` | rolling 5 Hz history (columns) |
//!
//! Lab API (served by `coffee-emu serve` only):
//!
//! | Method | Path            | Body / Response                  |
//! |--------|-----------------|----------------------------------|
//! | GET    | `/lab/info`     | [`lab::LabInfo`]                 |
//! | WS     | `/lab/ws`       | stream of [`lab::LabFrame`]      |
//! | POST   | `/lab/command`  | [`lab::LabCommand`] -> [`Ack`]   |

#![no_std]
extern crate alloc;

pub use coffee_io::panel_leds;
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub mod decent;
pub mod gaggimate;
pub mod recorder;

pub use coffee_core::config::EngineConfig;
pub use coffee_core::profile::{
    Curve, Exit, Phase as ProfilePhase, Profile, Stop, Target, Transition,
};
use coffee_io::{ActuatorFrame, EngineState, fault};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Info {
    pub name: String,
    pub version: String,
    /// "esp32c6", "host", ...
    pub platform: String,
    /// True when the engine's sensors come from an emulator.
    pub emulated: bool,
    /// Where the lab API lives, if known (UI may also probe).
    pub lab_url: Option<String>,
}

/// Telemetry snapshot, streamed at ~25 Hz.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Telemetry {
    /// Engine time, s.
    pub t: f64,
    pub state: String,
    pub faults: Vec<String>,
    pub phase: Option<u8>,
    pub phase_name: Option<String>,
    pub temp_c: Option<f32>,
    pub setpoint_c: f32,
    pub pressure_bar: Option<f32>,
    pub target_pressure_bar: Option<f32>,
    pub flow_ml_s: f32,
    pub target_flow_ml_s: Option<f32>,
    pub weight_g: Option<f32>,
    pub shot_time_s: f32,
    pub heater_duty: f32,
    pub pump_duty: f32,
    pub boost_c: f32,
    pub heat_soak: f32,
    pub water_c: Option<f32>,
    pub group_c: Option<f32>,
    pub heater: bool,
    pub pump: bool,
    pub valve: bool,
    pub brew_switch: bool,
    pub steam_switch: bool,
    /// Front-panel lamps (power, coffee, steam) exactly as the board drives its LED pins.
    #[serde(default)]
    pub lamps: [bool; 3],
    pub profile: String,
}

fn opt(x: f32) -> Option<f32> {
    if x.is_finite() { Some(x) } else { None }
}

impl Telemetry {
    pub fn from_frame(t: f64, a: &ActuatorFrame, inputs: u8, profile: &Profile) -> Self {
        let s = &a.status;
        let phase = (s.phase != 0xFF).then_some(s.phase);
        Telemetry {
            t,
            state: s.state.name().into(),
            faults: fault::NAMES
                .iter()
                .filter(|(b, _)| s.faults & b != 0)
                .map(|(_, n)| String::from(*n))
                .collect(),
            phase,
            phase_name: phase
                .and_then(|p| profile.phases.get(p as usize))
                .map(|p| p.name.clone()),
            temp_c: opt(s.temp_c),
            setpoint_c: s.setpoint_c,
            pressure_bar: opt(s.pressure_bar),
            target_pressure_bar: opt(s.target_pressure_bar),
            flow_ml_s: s.flow_ml_s,
            target_flow_ml_s: opt(s.target_flow_ml_s),
            weight_g: opt(s.weight_g),
            shot_time_s: s.shot_time_s,
            heater_duty: s.heater_duty,
            pump_duty: s.pump_duty,
            boost_c: s.boost_c,
            heat_soak: s.heat_soak,
            water_c: opt(s.water_c),
            group_c: opt(s.group_c),
            heater: a.heater,
            pump: a.pump,
            valve: a.valve,
            brew_switch: inputs & coffee_io::input::BREW != 0,
            steam_switch: inputs & coffee_io::input::STEAM != 0,
            lamps: a.lamps,
            profile: profile.name.clone(),
        }
    }

    pub fn engine_state(&self) -> EngineState {
        EngineState::from_name(&self.state).unwrap_or(EngineState::Fault)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Refused unless the machine is Ready, or `force` is set (flushing,
    /// backflushing, descaling...). The physical brew switch always works.
    StartBrew {
        #[serde(default)]
        force: bool,
    },
    StopBrew,
    ClearFaults,
    SetBrewTemp {
        c: f32,
    },
    SetSteamTemp {
        c: f32,
    },
    SelectProfile {
        name: String,
    },
    /// Warm-up / cleaning flush through the empty portafilter (not recorded).
    Flush {
        seconds: f32,
    },
    /// Use this shot's puck resistance as the grind reference for its profile.
    SetReferenceShot {
        id: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Ack {
    pub ok: bool,
    pub error: Option<String>,
}

impl Ack {
    pub fn ok() -> Self {
        Self {
            ok: true,
            error: None,
        }
    }
    pub fn err(e: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(e.into()),
        }
    }
}

/// One recorded sample of a shot (10 Hz).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ShotSample {
    pub t: f32,
    pub pressure: f32,
    pub target_pressure: Option<f32>,
    pub flow: f32,
    pub target_flow: Option<f32>,
    pub weight: Option<f32>,
    pub temp: Option<f32>,
    pub phase: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ShotMeta {
    pub id: u32,
    /// Engine time at start, s.
    pub started_t: f64,
    pub profile: String,
    pub duration_s: f32,
    pub weight_g: Option<f32>,
    pub pumped_ml: f32,
    pub end: String,
    pub mean_resistance: Option<f32>,
    pub peak_pressure_bar: f32,
    pub advice: Option<GrindAdvice>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Shot {
    pub meta: ShotMeta,
    pub phases: Vec<String>,
    pub samples: Vec<ShotSample>,
}

/// Dial-in suggestion from measured puck resistance.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GrindAdvice {
    /// Negative = go finer, positive = go coarser. Units: "relative grind", where
    /// 0.1 ~ 10% change in puck resistance.
    pub direction: f32,
    pub text: String,
}

/// Resistance is roughly proportional to grind fineness for a given dose, so
/// the log-ratio to the target tells us how far and which way to move.
pub fn grind_advice(measured: f32, target: f32) -> GrindAdvice {
    let ratio = measured / target;
    let r = libm_ln(ratio);
    let text = if r.abs() < 0.08 {
        String::from("Grind is on target.")
    } else if r < 0.0 {
        alloc::format!("Puck ran {:.0}% fast. Grind finer.", (1.0 - ratio) * 100.0)
    } else {
        alloc::format!(
            "Puck ran {:.0}% slow. Grind coarser.",
            (ratio - 1.0) * 100.0
        )
    };
    GrindAdvice {
        direction: -r,
        text,
    }
}

fn libm_ln(x: f32) -> f32 {
    // Small no_std natural log (good to ~1e-4 for 0.05..20), avoids a libm dep.
    if x <= 0.0 {
        return f32::NEG_INFINITY;
    }
    let mut m = x;
    let mut k = 0i32;
    while m > 1.5 {
        m /= core::f32::consts::E;
        k += 1;
    }
    while m < 0.5 {
        m *= core::f32::consts::E;
        k -= 1;
    }
    let y = (m - 1.0) / (m + 1.0);
    let y2 = y * y;
    k as f32 + 2.0 * y * (1.0 + y2 / 3.0 + y2 * y2 / 5.0 + y2 * y2 * y2 / 7.0)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Telemetry(Telemetry),
    ShotStarted { id: u32, profile: String },
    ShotEnded(ShotMeta),
    Log { text: String },
}

pub mod lab {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct SignalInfo {
        pub name: String,
        pub unit: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct KnobInfo {
        pub path: String,
        pub hint: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct LabInfo {
        pub model_name: String,
        pub signals: Vec<SignalInfo>,
        pub knobs: Vec<KnobInfo>,
        /// Every numeric model parameter path.
        pub params: Vec<String>,
        pub scenarios: Vec<String>,
    }

    /// Ground-truth snapshot. `values[i]` matches `LabInfo::signals[i]`.
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum LabFrame {
        /// `None` where a signal is undefined right now (JSON has no NaN).
        Truth {
            t: f64,
            speed: f32,
            paused: bool,
            engine_connected: bool,
            values: Vec<Option<f64>>,
        },
        Event {
            t: f64,
            text: String,
        },
        ScenarioDone {
            name: String,
            passed: bool,
            lines: Vec<String>,
        },
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    #[serde(tag = "cmd", rename_all = "snake_case")]
    pub enum LabCommand {
        /// Set a knob, fault or model parameter. `value` is a number or a word.
        Set {
            path: String,
            value: String,
        },
        Pause,
        Resume,
        Speed {
            x: f32,
        },
        /// Run a scenario (DSL text) against the live machine.
        RunScenario {
            text: String,
        },
        RunNamedScenario {
            name: String,
        },
        Reset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ln_and_advice() {
        assert!((libm_ln(1.0)).abs() < 1e-4);
        assert!((libm_ln(2.0) - core::f32::consts::LN_2).abs() < 1e-3);
        assert!((libm_ln(0.3) + 1.203_97).abs() < 1e-3);
        assert!(grind_advice(3.0, 4.5).text.contains("finer"));
        assert!(grind_advice(6.0, 4.5).text.contains("coarser"));
    }
}

