//! Server-side WebSocket pieces (RFC 6455) for the telemetry stream at `/api/ws`.
//!
//! The device only sends text frames (unmasked, server → client) and only reads
//! client frames far enough to answer pings and notice a close.

use alloc::string::String;
use alloc::vec::Vec;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// `Sec-WebSocket-Accept` for a client's `Sec-WebSocket-Key`.
pub fn accept_key(client_key: &str) -> String {
    let mut h = sha1_smol::Sha1::new();
    h.update(client_key.trim().as_bytes());
    h.update(GUID.as_bytes());
    base64(&h.digest().bytes())
}

/// The 101 response that completes the handshake.
pub fn handshake_response(client_key: &str) -> Vec<u8> {
    let mut s = String::from(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ",
    );
    s.push_str(&accept_key(client_key));
    s.push_str("\r\n\r\n");
    s.into_bytes()
}

pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

pub const OP_TEXT: u8 = 0x1;
pub const OP_CLOSE: u8 = 0x8;
pub const OP_PING: u8 = 0x9;
pub const OP_PONG: u8 = 0xA;

/// An unmasked, final server frame.
pub fn encode(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(payload.len() + 10);
    f.push(0x80 | opcode);
    match payload.len() {
        n if n < 126 => f.push(n as u8),
        n if n <= 0xFFFF => {
            f.push(126);
            f.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            f.push(127);
            f.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    f.extend_from_slice(payload);
    f
}

/// A client frame larger than allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooBig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

/// Decode one (client, possibly masked) frame from the start of `buf`.
/// Returns the frame and the bytes it used, or `None` if more bytes are needed.
/// Payloads above `max` are an error.
pub fn decode(buf: &[u8], max: usize) -> Result<Option<(Frame, usize)>, TooBig> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let opcode = buf[0] & 0x0F;
    let masked = buf[1] & 0x80 != 0;
    let mut i = 2;
    let len = match buf[1] & 0x7F {
        126 => {
            if buf.len() < 4 {
                return Ok(None);
            }
            i = 4;
            u16::from_be_bytes([buf[2], buf[3]]) as usize
        }
        127 => {
            if buf.len() < 10 {
                return Ok(None);
            }
            i = 10;
            let mut b = [0u8; 8];
            b.copy_from_slice(&buf[2..10]);
            u64::from_be_bytes(b) as usize
        }
        n => n as usize,
    };
    if len > max {
        return Err(TooBig);
    }
    let mask = if masked {
        if buf.len() < i + 4 {
            return Ok(None);
        }
        let m = [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]];
        i += 4;
        Some(m)
    } else {
        None
    };
    if buf.len() < i + len {
        return Ok(None);
    }
    let mut payload = buf[i..i + len].to_vec();
    if let Some(m) = mask {
        for (k, b) in payload.iter_mut().enumerate() {
            *b ^= m[k % 4];
        }
    }
    Ok(Some((Frame { opcode, payload }, i + len)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_example_key() {
        // RFC 6455 section 1.3
        assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn base64_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn encode_lengths() {
        assert_eq!(encode(OP_TEXT, b"hi"), [0x81, 2, b'h', b'i']);
        let mid = encode(OP_TEXT, &[0u8; 300]);
        assert_eq!(&mid[..4], &[0x81, 126, 0x01, 0x2C]);
        let big = encode(OP_TEXT, &[0u8; 70000]);
        assert_eq!(big[1], 127);
        assert_eq!(big.len(), 70000 + 10);
    }

    #[test]
    fn decodes_masked_client_frame() {
        // RFC 6455 5.7: masked "Hello"
        let f = [0x81, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58];
        let (fr, used) = decode(&f, 1024).unwrap().unwrap();
        assert_eq!(fr.opcode, OP_TEXT);
        assert_eq!(fr.payload, b"Hello");
        assert_eq!(used, f.len());
        assert_eq!(decode(&f[..6], 1024), Ok(None));
        // server frames round-trip too
        let e = encode(OP_PING, b"x");
        assert_eq!(decode(&e, 16).unwrap().unwrap().0.payload, b"x");
        assert_eq!(decode(&encode(OP_TEXT, &[0; 200]), 100), Err(TooBig));
    }
}
