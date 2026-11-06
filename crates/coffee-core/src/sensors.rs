//! Sensor decoding: what the board measures -> engineering units.
//!
//! Temperature: the K-type thermocouple's EMF (MCP3421, microvolts) and the cold junction
//! (TMP117 at the probe's socket), converted with the NIST ITS-90 reference functions
//! ([`coffee_io::typek`]). Pressure: the transducer as calibrated ADC1 millivolts, oversampled
//! over one mains cycle. Both models also recognise wiring faults from impossible readings.

use crate::config::{PressureSensorConfig, ThermocoupleConfig};
use coffee_io::typek;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TcReading {
    Celsius(f32),
    /// EMF far above anything the boiler makes: probe or wire open.
    Open,
    /// EMF well negative: probe wires swapped.
    Reversed,
}

/// Calibrated hot-junction temperature for a measured EMF and cold-junction temperature
/// (no fault checks): undo the input network's gain, add back the cold junction's own EMF,
/// invert the type-K reference function, then apply the two-point calibration.
pub fn thermocouple_c(cfg: &ThermocoupleConfig, emf_uv: f32, cj_c: f32) -> f32 {
    let true_mv = emf_uv as f64 / 1000.0 / cfg.input_gain as f64;
    let t = typek::hot_junction_c(true_mv, cj_c as f64) as f32;
    cfg.cal_gain * t + cfg.cal_offset_c
}

/// Measured EMF and cold junction -> calibrated temperature, or the wiring fault it indicates.
pub fn thermocouple(cfg: &ThermocoupleConfig, emf_uv: i32, cj_c: f32) -> TcReading {
    let e = emf_uv as f32;
    if e >= cfg.open_uv {
        return TcReading::Open;
    }
    if e <= cfg.reversed_uv {
        return TcReading::Reversed;
    }
    TcReading::Celsius(thermocouple_c(cfg, e, cj_c))
}

/// Inverse of [`thermocouple_c`] (emulator, tests, tooling): the EMF the ADC measures for a
/// calibrated temperature `c` with the cold junction at `cj_c`.
pub fn thermocouple_uv_for_c(cfg: &ThermocoupleConfig, c: f32, cj_c: f32) -> f32 {
    let t = ((c - cfg.cal_offset_c) / cfg.cal_gain) as f64;
    ((typek::emf_mv(t) - typek::emf_mv(cj_c as f64)) * 1000.0 * cfg.input_gain as f64) as f32
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

    fn celsius(r: TcReading) -> f32 {
        match r {
            TcReading::Celsius(c) => c,
            other => panic!("expected a temperature, got {other:?}"),
        }
    }

    #[test]
    fn thermocouple_design_points() {
        let c = ThermocoupleConfig::default();
        // Brew temperature with the board at 40 C: ~2.15 mV across the probe, ~41 uV/C, so the
        // MCP3421's 2 uV LSB is ~0.05 C.
        let uv = thermocouple_uv_for_c(&c, 93.0, 40.0);
        assert!((uv - 2_160.0).abs() < 25.0, "{uv}");
        assert!((celsius(thermocouple(&c, uv.round() as i32, 40.0)) - 93.0).abs() < 0.06);
        // The hottest the boiler can get (thermal fuse, 184 C) against a cold board stays far
        // below the open threshold; the open probe's ~90 mV is far above it.
        assert!(thermocouple_uv_for_c(&c, 184.0, 0.0) < 0.6 * c.open_uv);
        assert!(90_000.0 > 3.0 * c.open_uv);
        // A cold boiler with a warm board (the case after a long steam session): EMF slightly
        // negative, never near the reversed threshold. A reversed probe at brew temperature is.
        assert!(thermocouple_uv_for_c(&c, 20.0, 45.0) > 0.75 * c.reversed_uv);
        assert!(-thermocouple_uv_for_c(&c, 93.0, 40.0) < c.reversed_uv);
    }

    #[test]
    fn thermocouple_round_trip_and_calibration() {
        let mut c = ThermocoupleConfig::default();
        for cj in [10.0f32, 25.0, 45.0, 70.0] {
            // boiler from 20 C under the board (cold boiler, warm board) up to steam
            for t in [10.0f32, 25.0, 60.0, 93.0, 120.0, 145.0, 170.0].into_iter().filter(|&t| t >= cj - 20.0) {
                let uv = thermocouple_uv_for_c(&c, t, cj);
                let back = celsius(thermocouple(&c, uv.round() as i32, cj));
                assert!((back - t).abs() < 0.08, "{t} @ cj {cj} -> {uv} -> {back}");
            }
        }
        c.cal_gain = 1.012;
        c.cal_offset_c = -1.4;
        for t in [20.0f32, 93.0, 140.0] {
            let back = celsius(thermocouple(&c, thermocouple_uv_for_c(&c, t, 35.0).round() as i32, 35.0));
            assert!((back - t).abs() < 0.06, "{t} -> {back}");
        }
    }

    #[test]
    fn thermocouple_faults() {
        let c = ThermocoupleConfig::default();
        assert_eq!(thermocouple(&c, 90_000, 30.0), TcReading::Open);
        assert_eq!(thermocouple(&c, 15_000, 30.0), TcReading::Open);
        assert_eq!(thermocouple(&c, -2_100, 30.0), TcReading::Reversed);
        // a shorted probe reads 0 uV: the board's own temperature, a valid-looking reading that
        // only the heater-ineffective check can catch
        assert!((celsius(thermocouple(&c, 0, 38.0)) - 38.0).abs() < 0.08);
    }

    #[test]
    fn pressure_decoding() {
        let c = PressureSensorConfig::default();
        // 0.5 V -> 0 bar, 4.5 V -> 12 bar (1.2 MPa), through the 100/147 divider.
        assert!((pressure_to_mv(&c, 0.0, 0.0) - 340.1).abs() < 0.1);
        assert!((pressure_to_mv(&c, 12.0, 0.0) - 3061.2).abs() < 0.1);
        let PressureReading::Bar(b) = pressure(&c, 3061, 0.0) else {
            panic!()
        };
        assert!((b - 12.0).abs() < 0.01);
        // 1 mV at the pin is ~4.4 mbar.
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
