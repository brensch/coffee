//! Raw peripheral data -> engineering units.

use crate::config::{PressureSensorConfig, ThermocoupleConfig};
use coffee_io::{ads1115, max31855};

pub fn thermocouple_c(cfg: &ThermocoupleConfig, word: u32) -> Result<f32, max31855::TcFault> {
    max31855::decode(word).map(|r| r.thermocouple_c + cfg.offset_c)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PressureReading {
    Bar(f32),
    /// ADC voltage below the plausible minimum: wire off / sensor dead.
    Disconnected,
}

pub fn pressure(cfg: &PressureSensorConfig, counts: i16) -> PressureReading {
    let v_adc = ads1115::counts_to_volts(counts);
    if v_adc < cfg.v_fault_low {
        return PressureReading::Disconnected;
    }
    let v = v_adc / cfg.divider;
    let bar = (v - cfg.v_zero) / (cfg.v_full - cfg.v_zero) * cfg.range_bar;
    PressureReading::Bar(bar)
}

/// Inverse of [`pressure`], used by tests and by the emulator's sensor model.
pub fn pressure_to_counts(cfg: &PressureSensorConfig, bar: f32) -> i16 {
    let v = cfg.v_zero + bar / cfg.range_bar * (cfg.v_full - cfg.v_zero);
    ads1115::volts_to_counts(v * cfg.divider)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pressure_roundtrip() {
        let c = PressureSensorConfig::default();
        for bar in [0.0f32, 1.0, 9.0, 15.9] {
            match pressure(&c, pressure_to_counts(&c, bar)) {
                PressureReading::Bar(b) => assert!((b - bar).abs() < 0.01),
                _ => panic!(),
            }
        }
        assert_eq!(pressure(&c, 0), PressureReading::Disconnected);
    }
}
