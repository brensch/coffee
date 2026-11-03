//! The parts of the ESP32-C6 firmware that are pure logic, kept here so they are
//! unit-tested on the host. The firmware (`firmware/esp32`) is then only pin and
//! peripheral plumbing around these and `coffee_node::Node`.
//!
//! Timing model: there is no zero-cross detector. The firmware ticks on the
//! crystal once per nominal mains period ([`CycleClock`]); the MOC3063 opto-triacs
//! fire the triacs only at zero crossings, so whatever the window's phase against
//! the grid, a load asserted for one window conducts for one full mains cycle,
//! and the pump (diode in series) makes exactly one stroke. [`grid`] models what
//! happens when the grid is not exactly at nominal frequency.

#![no_std]

use coffee_io::EngineState;

/// Crystal-timed tick schedule. Tick `k` is at `t0 + round(k * period_ns / 1000)`
/// microseconds: no accumulated rounding error, and exactly the timestamps the
/// `.clog` format predicts (so steady logs cost nothing for timing).
#[derive(Clone, Copy, Debug)]
pub struct CycleClock {
    pub t0_us: u64,
    pub period_ns: u64,
}

impl CycleClock {
    pub fn new(t0_us: u64, mains_hz: f32) -> Self {
        let hz = if mains_hz.is_finite() {
            mains_hz.clamp(40.0, 70.0)
        } else {
            60.0
        };
        Self {
            t0_us,
            period_ns: (1e9 / hz as f64 + 0.5) as u64,
        }
    }

    /// Time of tick `k` (the boundary between cycle `k - 1` and cycle `k`).
    pub fn tick_us(&self, k: u32) -> u64 {
        self.t0_us + (k as u64 * self.period_ns + 500) / 1000
    }

    /// Time of ADC sample `i` of `n` in the cycle that starts at tick `k`. The
    /// samples are spread uniformly across the whole cycle, so their mean is the
    /// true cycle average of anything periodic with the mains (pump pressure
    /// ripple, mains pickup).
    pub fn sample_us(&self, k: u32, i: u32, n: u32) -> u64 {
        let base = k as u64 * self.period_ns * n as u64 + (2 * i as u64 + 1) * self.period_ns / 2;
        self.t0_us + (base / n as u64 + 500) / 1000
    }
}

/// Accumulates calibrated millivolt samples over one cycle.
#[derive(Clone, Copy, Debug, Default)]
pub struct Oversample {
    sum: u32,
    n: u32,
}

impl Oversample {
    pub fn push(&mut self, mv: u16) {
        self.sum += mv as u32;
        self.n += 1;
    }
    /// Rounded mean, clamped to the 3.3 V rail; resets the accumulator. With no
    /// samples (an overrun) returns `fallback`.
    pub fn take(&mut self, fallback: u16) -> u16 {
        let r = if self.n == 0 {
            fallback
        } else {
            ((self.sum + self.n / 2) / self.n).min(3300) as u16
        };
        *self = Self::default();
        r
    }
}

/// A switch input that only changes state once the raw level has held for
/// `need` consecutive cycles (3 cycles = 50 ms at 60 Hz: dry contacts bounce
/// for a few ms, and nobody can operate a lever faster than that).
#[derive(Clone, Copy, Debug)]
pub struct Debounce {
    state: bool,
    candidate: bool,
    count: u8,
    need: u8,
}

impl Debounce {
    pub const fn new(need: u8) -> Self {
        Self {
            state: false,
            candidate: false,
            count: 0,
            need,
        }
    }

    /// Feed this cycle's raw level (true = closed); returns the debounced state.
    pub fn update(&mut self, raw: bool) -> bool {
        if raw == self.state {
            self.count = 0;
        } else if raw == self.candidate && self.count > 0 {
            self.count += 1;
        } else {
            self.candidate = raw;
            self.count = 1;
        }
        if self.count >= self.need {
            self.state = raw;
            self.count = 0;
        }
        self.state
    }

    pub fn state(&self) -> bool {
        self.state
    }
}

/// Status LED: fast blink on any fault, slow heartbeat while warming up, solid
/// once the machine is usable (ready, brewing, steaming, ...).
pub fn status_led(state: EngineState, faults: u16, t_ms: u64) -> bool {
    match state {
        _ if faults != 0 => t_ms % 200 < 100,
        EngineState::Fault => t_ms % 200 < 100,
        EngineState::Boot | EngineState::Heating => t_ms % 1000 < 120,
        _ => true,
    }
}

/// What actually reaches the pump when the crystal and the grid disagree.
///
/// The opto-triac fires at a positive-going zero crossing only if its LED is lit
/// at that instant, and the pump's diode passes only that positive half wave, so
/// the strokes delivered while the pump output is asserted over `[a, b)` are the
/// positive-going crossings in `[a, b)`. Crossings sit on a lattice of spacing
/// `T_mains`; a window of the crystal's nominal period `T` holds exactly one of
/// them when `T == T_mains`, and otherwise one, except in the single window per
/// beat period `1 / |f - f_nominal|` whose edge a crossing slips across (0 or 2).
/// Times are integer picoseconds so the lattice arithmetic is exact.
pub mod grid {
    /// Positive-going crossings at `phase_ps + j * period_ps` lying in `[a, b)`.
    pub fn crossings_in(a_ps: i64, b_ps: i64, period_ps: i64, phase_ps: i64) -> i64 {
        let ceil_div = |x: i64| x.div_euclid(period_ps) + (x.rem_euclid(period_ps) != 0) as i64;
        ceil_div(b_ps - phase_ps) - ceil_div(a_ps - phase_ps)
    }

    /// Mains period in picoseconds for a frequency in millihertz.
    pub fn period_ps(mhz: i64) -> i64 {
        1_000_000_000_000_000 / mhz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_has_no_drift_and_spreads_samples() {
        let c = CycleClock::new(1_000, 60.0);
        assert_eq!(c.period_ns, 16_666_667);
        assert_eq!(c.tick_us(0), 1_000);
        assert_eq!(c.tick_us(3), 1_000 + 50_000);
        // No accumulated rounding: an hour of ticks is off nominal only by the
        // 1/3 ns the period was rounded to (72 us/h = 0.02 ppm, far below the
        // crystal's own +/-10 ppm).
        let k = 60 * 3600;
        assert!((c.tick_us(k) as i64 - (1_000 + 3_600_000_000)).abs() <= 73);
        // Samples are inside their cycle, evenly spaced, centred in their slots.
        let n = 32;
        let s: [u64; 32] = core::array::from_fn(|i| c.sample_us(10, i as u32, n));
        assert!(s[0] > c.tick_us(10) && s[31] < c.tick_us(11));
        for w in s.windows(2) {
            assert!((w[1] - w[0]).abs_diff(521) <= 1);
        }
        let mid = (s[0] + s[31]) / 2;
        assert!(mid.abs_diff((c.tick_us(10) + c.tick_us(11)) / 2) <= 1);
    }

    #[test]
    fn oversample_mean_and_clamp() {
        let mut o = Oversample::default();
        for mv in [1000u16, 1001, 1001, 1002] {
            o.push(mv);
        }
        assert_eq!(o.take(0), 1001);
        assert_eq!(o.take(77), 77);
        o.push(3350);
        assert_eq!(o.take(0), 3300);
    }

    #[test]
    fn debounce_needs_stable_cycles() {
        let mut d = Debounce::new(3);
        // Contact bounce: never three in a row.
        for raw in [true, false, true, true, false, true, false] {
            assert!(!d.update(raw));
        }
        assert!(!d.update(true));
        assert!(!d.update(true));
        assert!(d.update(true), "three stable cycles close it");
        // A one-cycle glitch while closed is ignored.
        assert!(d.update(false));
        assert!(d.update(true));
        assert!(d.update(false) && d.update(false));
        assert!(!d.update(false), "and three open it again");
    }

    #[test]
    fn led_patterns() {
        assert!(status_led(EngineState::Ready, 0, 12_345));
        let beats = (0..1000)
            .filter(|t| status_led(EngineState::Heating, 0, *t))
            .count();
        assert_eq!(beats, 120);
        let blink = (0..1000)
            .filter(|t| status_led(EngineState::Ready, 4, *t))
            .count();
        assert_eq!(blink, 500);
    }

    /// Commanded pump windows vs strokes delivered, for a pump pattern over
    /// `secs` of crystal time with the grid at `grid_mhz` and some phase.
    fn run(
        grid_mhz: i64,
        phase_ps: i64,
        secs: i64,
        pattern: impl Fn(i64) -> bool,
    ) -> (i64, i64, i64) {
        let t = grid::period_ps(60_000); // crystal window, nominal 60 Hz
        let tm = grid::period_ps(grid_mhz);
        let (mut commanded, mut delivered, mut odd) = (0, 0, 0);
        for k in 0..secs * 60 {
            if pattern(k) {
                let n = grid::crossings_in(k * t, (k + 1) * t, tm, phase_ps);
                commanded += 1;
                delivered += n;
                odd += (n != 1) as i64;
            }
        }
        (commanded, delivered, odd)
    }

    #[test]
    fn one_stroke_per_window_at_nominal_mains_any_phase() {
        let t = grid::period_ps(60_000);
        for phase in [0, 1, t / 7, t / 2, t - 1] {
            let (c, d, odd) = run(60_000, phase, 600, |k| k % 3 != 1);
            assert_eq!(c, d);
            assert_eq!(odd, 0);
        }
    }

    #[test]
    fn stroke_count_under_grid_drift() {
        // Grid at 59.95 and 60.05 Hz (well outside what a real grid does for
        // more than seconds), crystal at nominal. Ten minutes per case.
        let secs = 600;
        let t = grid::period_ps(60_000);
        for grid_mhz in [59_950, 60_050] {
            for phase in [0, t / 3, t - 7] {
                // Isolated strokes (duty 1/2, sigma-delta-like) and a mixed pattern.
                for pattern in [|k: i64| k % 2 == 0, |k: i64| k % 5 != 0] {
                    let (c, d, odd) = run(grid_mhz, phase, secs, pattern);
                    // Every window delivers one stroke except where a crossing
                    // slips across a window edge: at most one window per beat
                    // period (1 / 0.05 Hz = 20 s), i.e. <= 30 + 1 in 10 min.
                    let beats = secs * (grid_mhz - 60_000).abs() / 1000 + 1;
                    assert!(odd <= beats, "{grid_mhz} mHz: {odd} odd windows");
                    // So the count the engine assumes (one per window) is off
                    // by at most that many strokes: < 0.2 % of the volume.
                    assert!((d - c).abs() <= beats, "{c} commanded, {d} delivered");
                    assert!(((d - c).abs() as f64) < 0.002 * c as f64);
                }
                // Continuous running delivers exactly the grid's positive half
                // waves, as the pump would on its own switch.
                let (c, d, _) = run(grid_mhz, phase, secs, |_| true);
                let exact = grid::crossings_in(0, c * t, grid::period_ps(grid_mhz), phase);
                assert_eq!(d, exact);
            }
        }
    }
}
