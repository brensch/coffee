//! The thermocouple front end's two I2C chips, as pure byte-level functions so they are tested on
//! the host and the firmware only moves bytes.
//!
//! - MCP3421A0 (Microchip DS22003E) at 0x68: 18-bit delta-sigma ADC across the probe (PGA 8).
//! - TMP117 (TI SNOSD82D) at 0x48: the cold junction, beside the probe's socket.

/// MCP3421 I2C address (A0 part).
pub const MCP3421_ADDR: u8 = 0x68;
/// Configuration byte (DS22003E §5.2): RDY 1 (no effect in continuous mode), O/C 1 continuous,
/// S1S0 11 = 18 bit at 3.75 SPS, G1G0 11 = PGA x8 (+-256 mV, 15.625 uV / 8 per LSB).
pub const MCP3421_CONFIG: u8 = 0b1001_1111;
/// Bits of the returned config byte that must match what was written (everything but RDY).
const MCP3421_CONFIG_MASK: u8 = 0b0111_1111;

/// What one 4-byte read of the MCP3421 (18-bit mode: three data bytes then the config byte) says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mcp3421 {
    /// A conversion not read before (RDY = 0): the EMF in microvolts.
    Fresh(i32),
    /// The same conversion as last time (RDY = 1): nothing new.
    Stale,
    /// The config byte isn't ours (the chip reset, or garbage on the bus): write the config again.
    Misconfigured,
}

pub fn mcp3421_decode(b: [u8; 4]) -> Mcp3421 {
    if b[3] & MCP3421_CONFIG_MASK != MCP3421_CONFIG & MCP3421_CONFIG_MASK {
        return Mcp3421::Misconfigured;
    }
    if b[3] & 0x80 != 0 {
        return Mcp3421::Stale;
    }
    // 18-bit two's complement, the top byte carrying the sign repeated (MMMMMM D17 D16): casting
    // it through i8 sign-extends it.
    let code = ((b[0] as i8 as i32) << 16) | ((b[1] as i32) << 8) | b[2] as i32;
    // 15.625 uV / 8 = 125/64 uV per LSB, exact in integers (|code| < 2^17, no overflow).
    Mcp3421::Fresh(code * 125 / 64)
}

/// TMP117 I2C address (ADD0 to GND).
pub const TMP117_ADDR: u8 = 0x48;
pub const TMP117_REG_TEMP: u8 = 0x00;
pub const TMP117_REG_ID: u8 = 0x0F;

/// Is this the TMP117's device-ID register (bits 11:0 = 0x117; 15:12 the revision)?
pub fn tmp117_id_ok(b: [u8; 2]) -> bool {
    u16::from_be_bytes(b) & 0x0FFF == 0x0117
}

/// Temperature register (big-endian, 7.8125 m°C per LSB) -> centi-degrees C, rounded.
/// `None` for 0x8000, the value the TMP117 reports before its first conversion after reset.
pub fn tmp117_centi(b: [u8; 2]) -> Option<i16> {
    let raw = i16::from_be_bytes(b);
    if raw == i16::MIN {
        return None;
    }
    // 0.78125 centi-C per LSB = 25/32, rounded half away from zero; |raw| * 25 / 32 < 25600.
    let x = raw as i32 * 25;
    Some(((x + if x >= 0 { 16 } else { -16 }) / 32) as i16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp3421_codes() {
        let cfg_new = MCP3421_CONFIG & 0x7F; // RDY clear: fresh
        // +1 LSB, -1 LSB, full scale both ways (DS22003E Table 5-3, 18 bit).
        assert_eq!(mcp3421_decode([0x00, 0x00, 0x01, cfg_new]), Mcp3421::Fresh(1));   // 1.95 uV -> 1
        assert_eq!(mcp3421_decode([0x00, 0x00, 0x40, cfg_new]), Mcp3421::Fresh(125));
        assert_eq!(mcp3421_decode([0xFF, 0xFF, 0xC0, cfg_new]), Mcp3421::Fresh(-125));
        assert_eq!(mcp3421_decode([0x01, 0xFF, 0xFF, cfg_new]), Mcp3421::Fresh(131_071 * 125 / 64));
        assert_eq!(mcp3421_decode([0xFE, 0x00, 0x00, cfg_new]), Mcp3421::Fresh(-131_072 * 125 / 64));
        // brew temperature against a 40 C board: ~2.16 mV = code ~1106
        assert_eq!(mcp3421_decode([0x00, 0x04, 0x52, cfg_new]), Mcp3421::Fresh(1106 * 125 / 64));
        assert_eq!(mcp3421_decode([0x00, 0x04, 0x52, MCP3421_CONFIG]), Mcp3421::Stale);
        // power-on default config (one-shot 12 bit x1) or an all-ones bus
        assert_eq!(mcp3421_decode([0, 0, 0, 0x90]), Mcp3421::Misconfigured);
        assert_eq!(mcp3421_decode([0xFF; 4]), Mcp3421::Misconfigured);
    }

    #[test]
    fn tmp117_registers() {
        assert!(tmp117_id_ok([0x01, 0x17]));
        assert!(tmp117_id_ok([0x21, 0x17]));
        assert!(!tmp117_id_ok([0xFF, 0xFF]));
        // datasheet examples (Table 8-1): 0x0C80 = 25 C, 0xFF80 = -1 C, 0x0001 = 7.8125 mC
        assert_eq!(tmp117_centi([0x0C, 0x80]), Some(2500));
        assert_eq!(tmp117_centi([0xFF, 0x80]), Some(-100));
        assert_eq!(tmp117_centi([0x00, 0x01]), Some(1));
        assert_eq!(tmp117_centi([0x7F, 0xFF]), Some(25_599)); // 255.99 C
        assert_eq!(tmp117_centi([0x80, 0x00]), None);
    }
}
