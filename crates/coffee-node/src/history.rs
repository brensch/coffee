//! Rolling telemetry history: a fixed-size ring of compact samples so a UI that
//! (re)connects can draw the last few minutes immediately.
//!
//! 24 bytes per sample. At 5 Hz: 10 min = 72 KB (PC), 4 min = 29 KB (ESP32).

use alloc::vec::Vec;
use coffee_io::{ActuatorFrame, EngineState};
use serde::Serialize;

const NONE: i16 = i16::MIN;

#[derive(Clone, Copy, Default)]
struct Sample {
    t_ms: u32,
    temp: i16,     // 0.1 C (probe)
    water: i16,    // 0.1 C (estimate)
    setpoint: i16, // 0.1 C
    pressure: i16, // 0.01 bar
    tgt_p: i16,    // 0.01 bar
    flow: i16,     // 0.01 ml/s
    tgt_f: i16,    // 0.01 ml/s
    weight: i16,   // 0.1 g
    heater: u8,    // %
    pump: u8,      // %
    state: u8,
    phase: u8,
}

fn q(x: f32, s: f32) -> i16 {
    let v = x * s;
    if v.is_finite() {
        (if v >= 0.0 { v + 0.5 } else { v - 0.5 }).clamp(-32767.0, 32767.0) as i16
    } else {
        NONE
    }
}

fn dq(v: i16, s: f32) -> Option<f32> {
    (v != NONE).then(|| v as f32 / s)
}

/// Phase marker recorded while a flush runs.
pub const PHASE_FLUSH: u8 = 0xFD;

pub struct History {
    buf: Vec<Sample>,
    cap: usize,
    head: usize,
    len: usize,
    period_s: f64,
    last_t: f64,
}

/// Column-oriented JSON for `/api/telemetry/history` (nulls where unknown).
#[derive(Serialize, Default)]
pub struct HistoryColumns {
    pub t: Vec<f64>,
    pub temp: Vec<Option<f32>>,
    pub water: Vec<Option<f32>>,
    pub setpoint: Vec<Option<f32>>,
    pub pressure: Vec<Option<f32>>,
    pub target_pressure: Vec<Option<f32>>,
    pub flow: Vec<Option<f32>>,
    pub target_flow: Vec<Option<f32>>,
    pub weight: Vec<Option<f32>>,
    pub heater: Vec<u8>,
    pub pump: Vec<u8>,
    pub state: Vec<&'static str>,
    pub phase: Vec<Option<u8>>,
}

impl History {
    pub fn new(capacity: usize, hz: f32) -> Self {
        Self {
            buf: alloc::vec![Sample::default(); capacity.max(1)],
            cap: capacity.max(1),
            head: 0,
            len: 0,
            period_s: 1.0 / hz.max(0.1) as f64,
            last_t: f64::NEG_INFINITY,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.head = 0;
        self.last_t = f64::NEG_INFINITY;
    }

    pub fn observe(&mut self, t: f64, a: &ActuatorFrame, flushing: bool) {
        if t < self.last_t {
            self.clear(); // clock went backwards: new session
        }
        if t - self.last_t < self.period_s {
            return;
        }
        self.last_t = t;
        let s = &a.status;
        self.buf[self.head] = Sample {
            t_ms: (t * 1000.0) as u32,
            temp: q(s.temp_c, 10.0),
            water: q(s.water_c, 10.0),
            setpoint: q(s.setpoint_c, 10.0),
            pressure: q(s.pressure_bar, 100.0),
            tgt_p: q(s.target_pressure_bar, 100.0),
            flow: q(s.flow_ml_s, 100.0),
            tgt_f: q(s.target_flow_ml_s, 100.0),
            weight: q(s.weight_g, 10.0),
            heater: (s.heater_duty.clamp(0.0, 1.0) * 100.0) as u8,
            pump: (s.pump_duty.clamp(0.0, 1.0) * 100.0) as u8,
            state: s.state as u8,
            // Flushes reuse phase 0 of a one-phase profile: mark them so charts can say "flush".
            phase: if flushing && s.phase != 0xFF { PHASE_FLUSH } else { s.phase },
        };
        self.head = (self.head + 1) % self.cap;
        self.len = (self.len + 1).min(self.cap);
    }

    /// The last `seconds` of history, oldest first.
    pub fn columns(&self, seconds: f64) -> HistoryColumns {
        let mut c = HistoryColumns::default();
        let start = (self.head + self.cap - self.len) % self.cap;
        let cutoff_ms = ((self.last_t - seconds).max(0.0) * 1000.0) as u32;
        for i in 0..self.len {
            let s = self.buf[(start + i) % self.cap];
            if s.t_ms < cutoff_ms {
                continue;
            }
            c.t.push(s.t_ms as f64 / 1000.0);
            c.temp.push(dq(s.temp, 10.0));
            c.water.push(dq(s.water, 10.0));
            c.setpoint.push(dq(s.setpoint, 10.0));
            c.pressure.push(dq(s.pressure, 100.0));
            c.target_pressure.push(dq(s.tgt_p, 100.0));
            c.flow.push(dq(s.flow, 100.0));
            c.target_flow.push(dq(s.tgt_f, 100.0));
            c.weight.push(dq(s.weight, 10.0));
            c.heater.push(s.heater);
            c.pump.push(s.pump);
            c.state.push(EngineState::from_u8(s.state).name());
            c.phase.push((s.phase != 0xFF).then_some(s.phase));
        }
        c
    }
}
