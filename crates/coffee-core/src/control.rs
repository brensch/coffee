//! Small, allocation-free control building blocks.

use crate::config::PidGains;

/// First-order low-pass filter.
#[derive(Clone, Debug, Default)]
pub struct Lpf {
    pub value: Option<f32>,
}

impl Lpf {
    pub fn update(&mut self, x: f32, dt: f32, tau: f32) -> f32 {
        let v = match self.value {
            None => x,
            Some(prev) => {
                let a = if tau <= 0.0 { 1.0 } else { dt / (tau + dt) };
                prev + a * (x - prev)
            }
        };
        self.value = Some(v);
        v
    }
    pub fn get(&self) -> f32 {
        self.value.unwrap_or(f32::NAN)
    }
    pub fn reset(&mut self) {
        self.value = None;
    }
}

/// PID with derivative-on-measurement (no kick on setpoint changes), a filtered
/// derivative, feed-forward, and conditional-integration anti-windup.
#[derive(Clone, Debug, Default)]
pub struct Pid {
    pub integral: f32,
    prev_meas: Option<f32>,
    d_filt: Lpf,
    pub last_p: f32,
    pub last_i: f32,
    pub last_d: f32,
}

impl Pid {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Seed the integrator so the output starts at `out` (bumpless transfer).
    pub fn preload(&mut self, out: f32) {
        self.integral = out;
    }

    pub fn update(&mut self, g: &PidGains, setpoint: f32, meas: f32, ff: f32, dt: f32) -> f32 {
        let err = setpoint - meas;
        let raw_d = match self.prev_meas {
            Some(p) if dt > 0.0 => -(meas - p) / dt,
            _ => 0.0,
        };
        self.prev_meas = Some(meas);
        let d = self.d_filt.update(raw_d, dt, g.d_tau_s);
        let p = g.kp * err;
        let candidate_i = self.integral + g.ki * err * dt;
        let out_unclamped = ff + p + candidate_i + g.kd * d;
        // Only integrate if that doesn't push further into saturation.
        let saturating_high = out_unclamped > g.out_max && err > 0.0;
        let saturating_low = out_unclamped < g.out_min && err < 0.0;
        if !(saturating_high || saturating_low) && err.abs() < g.i_zone {
            let lim = g.i_max.min(g.out_max - g.out_min);
            self.integral = candidate_i.clamp(-lim, lim);
        }
        self.last_p = p;
        self.last_i = self.integral;
        self.last_d = g.kd * d;
        (ff + p + self.integral + g.kd * d).clamp(g.out_min, g.out_max)
    }
}

/// First-order sigma-delta modulator: turns a duty in 0..1 into a stream of
/// on/off decisions whose running average tracks the duty with error < 1 period.
#[derive(Clone, Debug, Default)]
pub struct SigmaDelta {
    acc: f32,
}

impl SigmaDelta {
    pub fn next(&mut self, duty: f32) -> bool {
        let duty = if duty.is_nan() {
            0.0
        } else {
            duty.clamp(0.0, 1.0)
        };
        self.acc += duty;
        if self.acc >= 1.0 {
            self.acc -= 1.0;
            true
        } else {
            false
        }
    }
    pub fn reset(&mut self) {
        self.acc = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigma_delta_tracks_duty() {
        let mut sd = SigmaDelta::default();
        let n = 1000;
        let on = (0..n).filter(|_| sd.next(0.37)).count();
        assert!((on as i32 - 370).abs() <= 1);
    }

    #[test]
    fn pid_does_not_wind_up() {
        let g = PidGains {
            kp: 1.0,
            ki: 1.0,
            kd: 0.0,
            d_tau_s: 0.0,
            out_min: 0.0,
            out_max: 1.0,
            ..Default::default()
        };
        let mut pid = Pid::default();
        for _ in 0..10_000 {
            pid.update(&g, 100.0, 0.0, 0.0, 0.01);
        }
        assert!(pid.integral <= 1.0);
        // Once above setpoint the output must drop to zero quickly.
        let out = (0..50)
            .map(|_| pid.update(&g, 0.0, 1.0, 0.0, 0.01))
            .last()
            .unwrap();
        assert!(out < 0.5);
    }
}
