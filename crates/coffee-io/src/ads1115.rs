//! ADS1115 16-bit ADC helpers, fixed at the +/-4.096 V full-scale range (PGA=001).
//!
//! The recommended pressure transducer is a 5 V ratiometric 0.5-4.5 V part. Its output
//! goes through a resistive divider (default 2/3, e.g. 10k over 20k) so the ADS1115,
//! powered from 3.3 V, never sees more than VDD.

pub const FULL_SCALE_V: f32 = 4.096;
pub const LSB_V: f32 = FULL_SCALE_V / 32768.0;

pub fn volts_to_counts(v: f32) -> i16 {
    let c = v / LSB_V;
    let c = if c >= 0.0 { c + 0.5 } else { c - 0.5 };
    c.clamp(-32768.0, 32767.0) as i16
}

pub fn counts_to_volts(c: i16) -> f32 {
    c as f32 * LSB_V
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        for v in [0.0f32, 0.333, 1.0, 3.0, 4.0] {
            assert!((counts_to_volts(volts_to_counts(v)) - v).abs() < LSB_V);
        }
        assert_eq!(volts_to_counts(5.0), 32767);
    }
}
