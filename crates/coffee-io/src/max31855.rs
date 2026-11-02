//! MAX31855 cold-junction-compensated K-type thermocouple converter.
//!
//! 32-bit read-out (datasheet table 2):
//! ```text
//! 31..18  thermocouple temperature, 14-bit two's complement, 0.25 C/LSB
//! 17      reserved (0)
//! 16      fault (any of SCV/SCG/OC)
//! 15..4   internal (cold junction) temperature, 12-bit two's complement, 0.0625 C/LSB
//! 3       reserved (0)
//! 2       SCV: thermocouple shorted to VCC
//! 1       SCG: thermocouple shorted to GND
//! 0       OC:  thermocouple open (no connection)
//! ```

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TcFault {
    Open,
    ShortGnd,
    ShortVcc,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub thermocouple_c: f32,
    pub internal_c: f32,
}

fn round(x: f32) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        (x - 0.5) as i32
    }
}

/// Build the 32-bit word the chip would return. Values are quantised and clamped
/// to the chip's representable range (-270..1800 C thermocouple).
pub fn encode(thermocouple_c: f32, internal_c: f32, fault: Option<TcFault>) -> u32 {
    let int_raw = round(internal_c / 0.0625).clamp(-2048, 2047) as u32 & 0xFFF;
    let mut word = int_raw << 4;
    match fault {
        Some(f) => {
            word |= 1 << 16;
            word |= match f {
                TcFault::Open => 1,
                TcFault::ShortGnd => 2,
                TcFault::ShortVcc => 4,
            };
            // The chip leaves the TC field at full-scale-ish garbage on fault; we
            // report 0x1FFF like most units do on open circuit.
            word |= 0x1FFF << 18;
        }
        None => {
            let tc_raw = round(thermocouple_c / 0.25).clamp(-8192, 8191) as u32 & 0x3FFF;
            word |= tc_raw << 18;
        }
    }
    word
}

fn sign_extend(v: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((v << shift) as i32) >> shift
}

/// Decode a 32-bit read-out. `0x0000_0000` and `0xFFFF_FFFF` (bus stuck / chip
/// absent) are reported as [`TcFault::Open`].
pub fn decode(word: u32) -> Result<Reading, TcFault> {
    if word == 0 || word == 0xFFFF_FFFF {
        return Err(TcFault::Open);
    }
    if word & (1 << 16) != 0 {
        return Err(if word & 1 != 0 {
            TcFault::Open
        } else if word & 2 != 0 {
            TcFault::ShortGnd
        } else {
            TcFault::ShortVcc
        });
    }
    let tc = sign_extend(word >> 18, 14) as f32 * 0.25;
    let internal = sign_extend((word >> 4) & 0xFFF, 12) as f32 * 0.0625;
    Ok(Reading {
        thermocouple_c: tc,
        internal_c: internal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datasheet_examples() {
        // Datasheet table 4: +100.75 C => 0b00 0001 1001 0011 (0x0193) in bits 31..18.
        let w = encode(100.75, 25.0, None);
        assert_eq!(w >> 18, 0x0193);
        let r = decode(w).unwrap();
        assert_eq!(r.thermocouple_c, 100.75);
        assert_eq!(r.internal_c, 25.0);
        // -0.25 C => 0x3FFF
        assert_eq!(encode(-0.25, 25.0, None) >> 18, 0x3FFF);
        assert_eq!(decode(encode(-0.25, -1.0, None)).unwrap().internal_c, -1.0);
    }

    #[test]
    fn faults() {
        assert_eq!(
            decode(encode(0.0, 25.0, Some(TcFault::Open))),
            Err(TcFault::Open)
        );
        assert_eq!(
            decode(encode(0.0, 25.0, Some(TcFault::ShortGnd))),
            Err(TcFault::ShortGnd)
        );
        assert_eq!(
            decode(encode(0.0, 25.0, Some(TcFault::ShortVcc))),
            Err(TcFault::ShortVcc)
        );
        assert_eq!(decode(0), Err(TcFault::Open));
    }
}
