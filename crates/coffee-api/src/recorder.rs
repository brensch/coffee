//! Turns the per-tick engine output into shot records. Shared by the host engine
//! and the firmware so both produce identical history.

use crate::{Shot, ShotMeta, ShotSample, grind_advice};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use coffee_core::{BrewEnd, Profile, ShotSummary};
use coffee_io::{ActuatorFrame, EngineState};

pub struct Recorder {
    next_id: u32,
    current: Option<Shot>,
    last_sample_t: f64,
    pub max_samples: usize,
    /// Keep recording this long after the stop (drips, settled yield).
    pub settle_s: f64,
    settle_until: Option<f64>,
}

pub enum RecorderEvent {
    Started { id: u32 },
    Ended(Shot),
}

fn end_name(e: BrewEnd) -> &'static str {
    match e {
        BrewEnd::ProfileComplete => "profile_complete",
        BrewEnd::TargetWeight => "target_weight",
        BrewEnd::MaxTime => "max_time",
        BrewEnd::UserStop => "user_stop",
        BrewEnd::Fault => "fault",
    }
}

fn opt(x: f32) -> Option<f32> {
    x.is_finite().then_some(x)
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            next_id: 1,
            current: None,
            last_sample_t: f64::NEG_INFINITY,
            max_samples: 1800,
            settle_s: 4.0,
            settle_until: None,
        }
    }
}

impl Recorder {
    /// The shot being recorded right now, if any.
    pub fn current(&self) -> Option<&Shot> {
        self.current.as_ref()
    }

    pub fn with_next_id(next_id: u32) -> Self {
        Self {
            next_id,
            ..Default::default()
        }
    }

    /// Feed every tick. `last_shot` is `Engine::last_shot()`.
    pub fn observe(
        &mut self,
        t: f64,
        a: &ActuatorFrame,
        profile: &Profile,
        last_shot: Option<(u32, ShotSummary)>,
    ) -> Option<RecorderEvent> {
        let s = &a.status;
        let brewing = s.state == EngineState::Brewing;
        // A new shot while the previous one is still settling: file the old one now.
        let restart = brewing && self.settle_until.is_some();
        let tail = !brewing && self.current.is_some() && {
            let until = *self.settle_until.get_or_insert(t + self.settle_s);
            t < until
        };
        match (&mut self.current, (brewing && !restart) || tail) {
            (None, true) => {
                let id = self.next_id;
                self.next_id += 1;
                self.current = Some(Shot {
                    meta: ShotMeta {
                        id,
                        started_t: t,
                        profile: profile.name.clone(),
                        duration_s: 0.0,
                        weight_g: None,
                        pumped_ml: 0.0,
                        end: String::new(),
                        mean_resistance: None,
                        peak_pressure_bar: 0.0,
                        advice: None,
                    },
                    phases: profile.phases.iter().map(|p| p.name.clone()).collect(),
                    samples: Vec::new(),
                });
                self.last_sample_t = f64::NEG_INFINITY;
                Some(RecorderEvent::Started { id })
            }
            (Some(shot), true) => {
                if t - self.last_sample_t >= 0.1 && shot.samples.len() < self.max_samples {
                    self.last_sample_t = t;
                    shot.samples.push(ShotSample {
                        // Engine time since start, so the post-stop tail keeps advancing.
                        t: (t - shot.meta.started_t) as f32,
                        pressure: if s.pressure_bar.is_finite() {
                            s.pressure_bar
                        } else {
                            0.0
                        },
                        target_pressure: opt(s.target_pressure_bar),
                        flow: s.flow_ml_s,
                        target_flow: opt(s.target_flow_ml_s),
                        weight: opt(s.weight_g),
                        temp: opt(s.temp_c),
                        phase: s.phase,
                    });
                }
                None
            }
            (Some(_), false) => {
                self.settle_until = None;
                let mut shot = self.current.take().unwrap();
                if let Some((_, sum)) = last_shot {
                    shot.meta.duration_s = sum.duration_s;
                    shot.meta.weight_g = sum.weight_g;
                    shot.meta.pumped_ml = sum.pumped_ml;
                    shot.meta.end = end_name(sum.end).to_string();
                    shot.meta.mean_resistance = sum.mean_resistance;
                    shot.meta.peak_pressure_bar = sum.peak_pressure_bar;
                    shot.meta.advice = match (sum.mean_resistance, profile.target_resistance) {
                        // No reference yet: the user marks a shot they liked as the reference.
                        (Some(m), Some(tgt)) if sum.duration_s > 10.0 => Some(grind_advice(m, tgt)),
                        _ => None,
                    };
                }
                Some(RecorderEvent::Ended(shot))
            }
            (None, false) => None,
        }
    }
}
