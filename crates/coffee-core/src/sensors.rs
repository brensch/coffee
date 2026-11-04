//! Sensor decoding: ADC millivolts at the MCU pin -> engineering units.
//!
//! Both channels are read by the ESP32-C6's ADC1 (eFuse curve calibration,
//! oversampled over one mains cycle), so the inputs here are already calibrated
//! millivolts. The models below turn them into deg C and bar and recognise wiring
//! faults from out-of-range node voltages.

use crate::config::{NtcConfig, PressureSensorConfig};
use libm::{expf, logf};

const KELVIN: f32 = 273.15;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NtcReading {
    Celsius(f32),
    /// Node at the 3.3 V rail: thermistor or wire open.
    Open,
    /// Node at ground: thermistor or wire shorted.
    Short,
}

/// Thermistor resistance from the node voltage (NTC to GND, `r_ref` to 3.3 V).
pub fn ntc_ohms(cfg: &NtcConfig, mv: f32) -> f32 {
    let top = (cfg.v_ref_mv - mv).max(1e-3);
    cfg.r_ref_ohm * mv.max(0.0) / top
}

/// Model temperature (before calibration) for a thermistor resistance.
pub fn ntc_model_c(cfg: &NtcConfig, ohms: f32) -> f32 {
    let ln_r = logf(ohms.max(1e-3));
    let inv_t = match cfg.steinhart_hart {
        Some([a, b, c]) => a + b * ln_r + c * ln_r * ln_r * ln_r,
        None => 1.0 / (25.0 + KELVIN) + (ln_r - logf(cfg.r25_ohm)) / cfg.beta,
    };
    1.0 / inv_t - KELVIN
}

/// Node voltage -> calibrated temperature, or the wiring fault it indicates.
pub fn ntc(cfg: &NtcConfig, mv: u16) -> NtcReading {
    let mv = mv as f32;
    if mv >= cfg.open_mv {
        return NtcReading::Open;
    }
    if mv <= cfg.short_mv {
        return NtcReading::Short;
    }
    let t = cfg.cal_gain * ntc_model_c(cfg, ntc_ohms(cfg, mv)) + cfg.cal_offset_c;
    if t < cfg.min_c {
        return NtcReading::Open;   // colder than a boiler can be: an open probe read low
    }
    NtcReading::Celsius(t)
}

/// Inverse of [`ntc`] for a calibrated temperature (tests, tooling). Inverts the
/// calibration, then the model (closed form for beta, bisection for S-H).
pub fn ntc_mv_for_c(cfg: &NtcConfig, c: f32) -> f32 {
    let model_c = (c - cfg.cal_offset_c) / cfg.cal_gain;
    let ohms = match cfg.steinhart_hart {
        None => cfg.r25_ohm * expf(cfg.beta * (1.0 / (model_c + KELVIN) - 1.0 / (25.0 + KELVIN))),
        Some(_) => {
            // Model temperature falls monotonically with resistance.
            let (mut lo, mut hi) = (1.0f32, 1.0e8f32);
            for _ in 0..80 {
                let mid = libm::sqrtf(lo * hi);
                if ntc_model_c(cfg, mid) > model_c {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            libm::sqrtf(lo * hi)
        }
    };
    cfg.v_ref_mv * ohms / (cfg.r_ref_ohm + ohms)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PressureReading {
    Bar(f32),
    Disconnected,
}

/// Uncalibrated (datasheet transfer function) pressure for a pin voltage.
pub fn pressure_nominal_bar(cfg: &PressureSensorConfig, mv: f32) -> f32 {
    let v_sensor = mv / cfg.divider;
    (v_sensor - cfg.v_zero_mv) / (cfg.v_full_mv - cfg.v_zero_mv) * cfg.range_bar
}

/// Pin voltage -> pressure (gauge bar) with a given zero offset (nominal bar read
/// with the system open to air). The engine passes the install offset or the
/// accepted power-on self-zero.
pub fn pressure(cfg: &PressureSensorConfig, mv: u16, zero_offset_bar: f32) -> PressureReading {
    let mv = mv as f32;
    if mv < cfg.fault_low_mv {
        return PressureReading::Disconnected;
    }
    PressureReading::Bar(cfg.span_gain * (pressure_nominal_bar(cfg, mv) - zero_offset_bar))
}

/// Inverse of [`pressure`] (emulator / tests): pin millivolts for a gauge pressure.
pub fn pressure_to_mv(cfg: &PressureSensorConfig, bar: f32, zero_offset_bar: f32) -> f32 {
    let nominal = bar / cfg.span_gain + zero_offset_bar;
    (cfg.v_zero_mv + nominal / cfg.range_bar * (cfg.v_full_mv - cfg.v_zero_mv)) * cfg.divider
}

#[cfg(test)]
mod tests {
    use super::*;

    fn celsius(r: NtcReading) -> f32 {
        match r {
            NtcReading::Celsius(c) => c,
            other => panic!("expected a temperature, got {other:?}"),
        }
    }

    #[test]
    fn ntc_design_points() {
        let c = NtcConfig::default();
        // Molex 218537-3919 (50k) with the 4.7k reference, from Molex's R-T table:
        // 25 C 50.000k -> 3016 mV, 93 C ~3.9k -> 1496 mV, 140 C 1.029k -> 593 mV.
        assert!((ntc_mv_for_c(&c, 25.0) - 3016.0).abs() < 5.0);
        assert!((ntc_mv_for_c(&c, 93.0) - 1496.0).abs() < 8.0);
        assert!((ntc_mv_for_c(&c, 140.0) - 593.0).abs() < 5.0);
        // Steam (145 C) still well above the short threshold.
        assert!(ntc_mv_for_c(&c, 145.0) > 4.0 * c.short_mv);
        // A cold kitchen (10 C) does not look open...
        assert!(ntc_mv_for_c(&c, 10.0) < c.open_mv);
        // ...but an open probe does even if the ADC reads 40 mV low at the top of its range
        // (ESP32-C6 datasheet: +-40 mV at 12 dB attenuation).
        assert!(3300.0 - 40.0 >= c.open_mv + 50.0);
    }

    #[test]
    fn ntc_round_trip_beta_and_calibration() {
        let mut c = NtcConfig::default();
        for t in [10.0f32, 25.0, 60.0, 93.0, 120.0, 145.0, 160.0] {
            let mv = ntc_mv_for_c(&c, t);
            let back = celsius(ntc(&c, mv.round() as u16));
            // 1 mV of quantisation is < 0.1 C anywhere in the working range.
            assert!((back - t).abs() < 0.12, "{t} -> {mv} -> {back}");
        }
        c.cal_gain = 1.012;
        c.cal_offset_c = -1.4;
        for t in [20.0f32, 93.0, 140.0] {
            let back = celsius(ntc(&c, ntc_mv_for_c(&c, t).round() as u16));
            assert!((back - t).abs() < 0.12, "{t} -> {back}");
        }
    }

    #[test]
    fn ntc_steinhart_hart_matches_beta() {
        // S-H fitted to the beta curve at 25/90/150 C must agree in between.
        let beta = NtcConfig::default();
        let pts = [25.0f32, 90.0, 150.0].map(|t| {
            let r = ntc_ohms(&beta, ntc_mv_for_c(&beta, t));
            (logf(r) as f64, 1.0 / (t as f64 + KELVIN as f64))
        });
        // Solve the 3x3 system for A, B, C (Cramer's rule, f64).
        let m = pts.map(|(l, _)| [1.0, l, l * l * l]);
        let y = pts.map(|(_, y)| y);
        let det = |m: [[f64; 3]; 3]| {
            m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
        };
        let d = det(m);
        let coef: [f32; 3] = core::array::from_fn(|k| {
            let mut mk = m;
            for i in 0..3 {
                mk[i][k] = y[i];
            }
            (det(mk) / d) as f32
        });
        let sh = NtcConfig {
            steinhart_hart: Some(coef),
            ..NtcConfig::default()
        };
        for t in [40.0f32, 95.0, 130.0] {
            let mv = ntc_mv_for_c(&beta, t);
            let a = celsius(ntc(&beta, mv as u16));
            let b = celsius(ntc(&sh, mv as u16));
            assert!((a - b).abs() < 0.3, "{t}: beta {a} vs S-H {b}");
            assert!((ntc_mv_for_c(&sh, t) - mv).abs() < 10.0);
        }
    }

    #[test]
    fn ntc_faults() {
        let c = NtcConfig::default();
        assert_eq!(ntc(&c, 3300), NtcReading::Open);
        assert_eq!(ntc(&c, 3290), NtcReading::Open);
        assert_eq!(ntc(&c, 0), NtcReading::Short);
        assert_eq!(ntc(&c, 40), NtcReading::Short);
        assert!(matches!(ntc(&c, 1650), NtcReading::Celsius(_)));
    }

    #[test]
    fn pressure_decoding() {
        let c = PressureSensorConfig::default();
        // 0.5 V -> 0 bar, 4.5 V -> 17.24 bar (250 psi), through the 100/147 divider.
        assert!((pressure_to_mv(&c, 0.0, 0.0) - 340.1).abs() < 0.1);
        assert!((pressure_to_mv(&c, 17.24, 0.0) - 3061.2).abs() < 0.1);
        let PressureReading::Bar(b) = pressure(&c, 3061, 0.0) else {
            panic!()
        };
        assert!((b - 17.24).abs() < 0.01);
        // 1 mV at the pin is ~6.3 mbar.
        for bar in [0.0f32, 2.5, 9.0, 12.0] {
            let mv = pressure_to_mv(&c, bar, 0.0).round() as u16;
            let PressureReading::Bar(b) = pressure(&c, mv, 0.0) else {
                panic!()
            };
            assert!((b - bar).abs() < 0.01, "{bar} -> {b}");
        }
        // Calibration: offset and span both applied.
        let cal = PressureSensorConfig {
            span_gain: 1.05,
            ..c.clone()
        };
        let mv = pressure_to_mv(&cal, 9.0, 0.12).round() as u16;
        let PressureReading::Bar(b) = pressure(&cal, mv, 0.12) else {
            panic!()
        };
        assert!((b - 9.0).abs() < 0.015);
        // Unplugged: the divider pulls the pin to ground.
        assert_eq!(pressure(&c, 0, 0.0), PressureReading::Disconnected);
        assert_eq!(pressure(&c, 150, 0.0), PressureReading::Disconnected);
        assert!(matches!(pressure(&c, 320, 0.0), PressureReading::Bar(_)));
    }
}
