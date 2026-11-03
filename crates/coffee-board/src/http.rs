//! Minimal HTTP/1.1 request parsing and response heads for the firmware's server.
//!
//! The firmware serves three kinds of request: the embedded UI (static, gzip),
//! `/api/*` (passed to `Node::handle_http`) and `/api/ws` (WebSocket telemetry).
//! Requests are small, so the whole head is read into a buffer and parsed here;
//! bodies are read by `Content-Length` (no chunked uploads).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Largest request head accepted.
pub const MAX_HEAD: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Request {
    pub method: String,
    /// Path including any query string.
    pub path: String,
    pub content_length: usize,
    /// `Upgrade: websocket` with a key.
    pub ws_key: Option<String>,
    pub accepts_gzip: bool,
    /// Bytes of the head, including the blank line.
    pub head_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadError {
    /// Need more bytes.
    Incomplete,
    TooLarge,
    Malformed,
}

/// Parse a request head from the start of `buf`.
pub fn parse_head(buf: &[u8]) -> Result<Request, HeadError> {
    let end = match find(buf, b"\r\n\r\n") {
        Some(i) => i + 4,
        None if buf.len() >= MAX_HEAD => return Err(HeadError::TooLarge),
        None => return Err(HeadError::Incomplete),
    };
    let head = core::str::from_utf8(&buf[..end]).map_err(|_| HeadError::Malformed)?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().ok_or(HeadError::Malformed)?.split(' ');
    let method = first.next().filter(|m| !m.is_empty()).ok_or(HeadError::Malformed)?;
    let path = first.next().filter(|p| p.starts_with('/')).ok_or(HeadError::Malformed)?;
    if !first.next().is_some_and(|v| v.starts_with("HTTP/1.")) {
        return Err(HeadError::Malformed);
    }
    let mut r = Request {
        method: method.to_string(),
        path: path.to_string(),
        head_len: end,
        ..Default::default()
    };
    let mut upgrade = false;
    for l in lines {
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim();
        if k.eq_ignore_ascii_case("content-length") {
            r.content_length = v.parse().map_err(|_| HeadError::Malformed)?;
        } else if k.eq_ignore_ascii_case("upgrade") {
            upgrade = v.eq_ignore_ascii_case("websocket");
        } else if k.eq_ignore_ascii_case("sec-websocket-key") {
            r.ws_key = Some(v.to_string());
        } else if k.eq_ignore_ascii_case("accept-encoding") {
            r.accepts_gzip = v.split(',').any(|e| e.trim().starts_with("gzip"));
        }
    }
    if !upgrade {
        r.ws_key = None;
    }
    Ok(r)
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

pub fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        101 => "Switching Protocols",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "",
    }
}

/// A response head. `extra` lines must each end in `\r\n`.
pub fn response_head(status: u16, content_type: &str, len: usize, extra: &str) -> Vec<u8> {
    let mut s = String::new();
    use core::fmt::Write;
    let _ = write!(
        s,
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n{extra}\r\n",
        reason(status)
    );
    s.into_bytes()
}

/// Content type and cache policy for an embedded UI file (hashed bundles are immutable).
pub fn static_meta(path: &str) -> (&'static str, &'static str) {
    let ct = if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".wasm") {
        "application/wasm"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "application/octet-stream"
    };
    let cache = if path.ends_with(".html") {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    (ct, cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_get() {
        let r = parse_head(b"GET /api/status?x=1 HTTP/1.1\r\nHost: coffee.local\r\nAccept-Encoding: br, gzip\r\n\r\nrest").unwrap();
        assert_eq!(r.method, "GET");
        assert_eq!(r.path, "/api/status?x=1");
        assert!(r.accepts_gzip);
        assert_eq!(r.ws_key, None);
        assert_eq!(r.head_len, 79); // 30 + 20 + 27 + 2
    }

    #[test]
    fn parses_post_and_ws() {
        let r = parse_head(b"PUT /api/config HTTP/1.1\r\ncontent-length: 12\r\n\r\n").unwrap();
        assert_eq!(r.content_length, 12);
        let w = parse_head(b"GET /api/ws HTTP/1.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n").unwrap();
        assert_eq!(w.ws_key.as_deref(), Some("dGhlIHNhbXBsZSBub25jZQ=="));
        // a key without the upgrade header doesn't count
        let n = parse_head(b"GET /api/ws HTTP/1.1\r\nSec-WebSocket-Key: abc\r\n\r\n").unwrap();
        assert_eq!(n.ws_key, None);
    }

    #[test]
    fn rejects_bad_heads() {
        assert_eq!(parse_head(b"GET / HTTP/1.1\r\n"), Err(HeadError::Incomplete));
        assert_eq!(parse_head(b"GARBAGE\r\n\r\n"), Err(HeadError::Malformed));
        assert_eq!(parse_head(b"GET nope HTTP/1.1\r\n\r\n"), Err(HeadError::Malformed));
        assert_eq!(
            parse_head(b"POST /x HTTP/1.1\r\nContent-Length: lots\r\n\r\n"),
            Err(HeadError::Malformed)
        );
        assert_eq!(parse_head(&[b'a'; MAX_HEAD]), Err(HeadError::TooLarge));
    }

    #[test]
    fn response_heads() {
        let h = response_head(200, "application/json", 5, "Cache-Control: no-store\r\n");
        let s = core::str::from_utf8(&h).unwrap();
        assert!(s.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(s.contains("Content-Length: 5\r\n"));
        assert!(s.ends_with("Cache-Control: no-store\r\n\r\n"));
        assert_eq!(static_meta("/a.wasm"), ("application/wasm", "public, max-age=31536000, immutable"));
        assert_eq!(static_meta("/index.html").1, "no-cache");
    }
}
