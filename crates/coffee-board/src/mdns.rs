//! A minimal mDNS responder: answers `A` queries for one name (`coffee.local`).
//!
//! Browsers and the `coffee` CLI resolve the board as `http://coffee.local/`.
//! Only what that needs is implemented: parse the questions of a query packet,
//! and build a response with one `A` record (cache-flush, TTL 120 s).

use alloc::vec::Vec;

pub const PORT: u16 = 5353;
pub const GROUP: [u8; 4] = [224, 0, 0, 251];
const TYPE_A: u16 = 1;
const TYPE_ANY: u16 = 255;
const CLASS_IN: u16 = 1;

/// Read a (possibly compressed) DNS name at `off`; returns lower-cased labels joined by '.'.
fn read_name(p: &[u8], mut off: usize) -> Option<(alloc::string::String, usize)> {
    let mut name = alloc::string::String::new();
    let mut end = None;
    for _ in 0..32 {
        let len = *p.get(off)? as usize;
        if len == 0 {
            return Some((name, end.unwrap_or(off + 1)));
        }
        if len & 0xC0 == 0xC0 {
            let ptr = ((len & 0x3F) << 8) | *p.get(off + 1)? as usize;
            end.get_or_insert(off + 2);
            off = ptr;
            continue;
        }
        let label = p.get(off + 1..off + 1 + len)?;
        if !name.is_empty() {
            name.push('.');
        }
        for &b in label {
            name.push(b.to_ascii_lowercase() as char);
        }
        off += 1 + len;
    }
    None
}

/// Does this packet ask for `host` (e.g. "coffee.local") as A/ANY?
/// Returns whether a unicast reply was requested (QU bit).
pub fn wants(packet: &[u8], host: &str) -> Option<bool> {
    if packet.len() < 12 || packet[2] & 0x80 != 0 {
        return None; // too short, or a response
    }
    let qd = u16::from_be_bytes([packet[4], packet[5]]);
    let mut off = 12;
    for _ in 0..qd {
        let (name, next) = read_name(packet, off)?;
        let t = u16::from_be_bytes([*packet.get(next)?, *packet.get(next + 1)?]);
        let c = u16::from_be_bytes([*packet.get(next + 2)?, *packet.get(next + 3)?]);
        off = next + 4;
        if name.eq_ignore_ascii_case(host) && (t == TYPE_A || t == TYPE_ANY) && c & 0x7FFF == CLASS_IN {
            return Some(c & 0x8000 != 0);
        }
    }
    None
}

/// An mDNS response announcing `host` at `ip`.
pub fn response(host: &str, ip: [u8; 4]) -> Vec<u8> {
    let mut p = Vec::with_capacity(64);
    p.extend_from_slice(&[0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0]); // id 0, QR|AA, 1 answer
    for label in host.split('.') {
        p.push(label.len() as u8);
        p.extend_from_slice(label.as_bytes());
    }
    p.push(0);
    p.extend_from_slice(&TYPE_A.to_be_bytes());
    p.extend_from_slice(&(CLASS_IN | 0x8000).to_be_bytes()); // cache-flush
    p.extend_from_slice(&120u32.to_be_bytes());
    p.extend_from_slice(&4u16.to_be_bytes());
    p.extend_from_slice(&ip);
    p
}

/// A one-question query for `host` (A, unicast-response bit set: the CLI listens on an ephemeral port).
pub fn query(host: &str) -> Vec<u8> {
    let mut p = Vec::with_capacity(32);
    p.extend_from_slice(&[0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in host.split('.') {
        p.push(label.len() as u8);
        p.extend_from_slice(label.as_bytes());
    }
    p.push(0);
    p.extend_from_slice(&TYPE_A.to_be_bytes());
    p.extend_from_slice(&(CLASS_IN | 0x8000).to_be_bytes());
    p
}

/// The IPv4 address for `host` in a response packet, if it has one.
pub fn parse_a(packet: &[u8], host: &str) -> Option<[u8; 4]> {
    if packet.len() < 12 || packet[2] & 0x80 == 0 {
        return None;
    }
    let qd = u16::from_be_bytes([packet[4], packet[5]]);
    let an = u16::from_be_bytes([packet[6], packet[7]]) as usize
        + u16::from_be_bytes([packet[8], packet[9]]) as usize
        + u16::from_be_bytes([packet[10], packet[11]]) as usize;
    let mut off = 12;
    for _ in 0..qd {
        off = read_name(packet, off)?.1 + 4;
    }
    for _ in 0..an {
        let (name, next) = read_name(packet, off)?;
        let t = u16::from_be_bytes([*packet.get(next)?, *packet.get(next + 1)?]);
        let len = u16::from_be_bytes([*packet.get(next + 8)?, *packet.get(next + 9)?]) as usize;
        let data = packet.get(next + 10..next + 10 + len)?;
        if t == TYPE_A && len == 4 && name.eq_ignore_ascii_case(host) {
            return Some([data[0], data[1], data[2], data[3]]);
        }
        off = next + 10 + len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(name: &str, qtype: u16, qu: bool) -> Vec<u8> {
        let mut p = alloc::vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        for l in name.split('.') {
            p.push(l.len() as u8);
            p.extend_from_slice(l.as_bytes());
        }
        p.push(0);
        p.extend_from_slice(&qtype.to_be_bytes());
        p.extend_from_slice(&(if qu { 0x8001u16 } else { 1 }).to_be_bytes());
        p
    }

    #[test]
    fn answers_only_our_name() {
        assert_eq!(wants(&query("coffee.local", TYPE_A, false), "coffee.local"), Some(false));
        assert_eq!(wants(&query("COFFEE.local", TYPE_ANY, true), "coffee.local"), Some(true));
        assert_eq!(wants(&query("other.local", TYPE_A, false), "coffee.local"), None);
        assert_eq!(wants(&query("coffee.local", 28, false), "coffee.local"), None); // AAAA
        assert_eq!(wants(&response("coffee.local", [1, 2, 3, 4]), "coffee.local"), None);
        assert_eq!(wants(&[0, 1, 2], "coffee.local"), None);
    }

    #[test]
    fn response_is_well_formed() {
        let r = response("coffee.local", [192, 168, 1, 50]);
        assert_eq!(&r[2..4], &[0x84, 0]);
        let (name, next) = read_name(&r, 12).unwrap();
        assert_eq!(name, "coffee.local");
        assert_eq!(&r[next..next + 2], &[0, 1]);
        assert_eq!(&r[r.len() - 4..], &[192, 168, 1, 50]);
    }

    #[test]
    fn query_and_parse_round_trip() {
        let q = super::query("coffee.local");
        assert_eq!(wants(&q, "coffee.local"), Some(true));
        let r = response("coffee.local", [10, 0, 0, 7]);
        assert_eq!(parse_a(&r, "coffee.local"), Some([10, 0, 0, 7]));
        assert_eq!(parse_a(&r, "other.local"), None);
        assert_eq!(parse_a(&q, "coffee.local"), None); // a query, not a response
    }

    #[test]
    fn follows_compression_pointers() {
        // two questions; the second points back at "local" in the first
        let mut p = query("x.local", TYPE_A, false);
        p[5] = 2;
        p.extend_from_slice(&[6, b'c', b'o', b'f', b'f', b'e', b'e', 0xC0, 14, 0, 1, 0, 1]);
        assert_eq!(wants(&p, "coffee.local"), Some(false));
    }
}
