//! USB console: a line-based command protocol on the USB Serial/JTAG port.
//!
//! The same port streams the binary `.clog` log (device → host). Commands go the
//! other way as text lines, and every reply or event the device sends is a
//! framed JSON object:
//!
//! ```text
//! request  := line "\n"                 e.g.  wifi set "My Net" "pa ss"
//! reply    := STX "COFFEE " json ETX "\n"
//! ```
//!
//! STX/ETX (0x02/0x03) can't appear inside the JSON (serde escapes control
//! characters), and the `.clog` reader resynchronises on its own magic, so
//! frames and log chunks interleave freely. `Deframer` pulls frames out of the
//! mixed byte stream; the `coffee` CLI uses it on the host.
//!
//! Arguments are whitespace-separated; double quotes group, and `\"` / `\\`
//! escape inside quotes (SSIDs and passwords may contain spaces).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const STX: u8 = 0x02;
pub const ETX: u8 = 0x03;
pub const TAG: &[u8] = b"COFFEE ";
/// Longest accepted command line (a JSON config fits).
pub const MAX_LINE: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cmd {
    /// `wifi set <ssid> [password]`: store credentials and (re)connect. No password = open network.
    WifiSet { ssid: String, password: String },
    /// `wifi clear`: forget credentials and disconnect.
    WifiClear,
    /// `status`: JSON status (version, network, temperatures, faults).
    Status,
    /// `reboot`.
    Reboot,
    /// `config get`: the engine config JSON.
    ConfigGet,
    /// `config set <json>`: replace the engine config.
    ConfigSet(String),
    /// `help`.
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    Unknown(String),
    Usage(&'static str),
    UnterminatedQuote,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty command"),
            ParseError::Unknown(c) => write!(f, "unknown command '{c}' (try help)"),
            ParseError::Usage(u) => write!(f, "usage: {u}"),
            ParseError::UnterminatedQuote => write!(f, "unterminated quote"),
        }
    }
}

pub const HELP: &str = "commands: wifi set <ssid> [password] | wifi clear | status | config get | config set <json> | reboot | help";

/// Split a line into arguments (quotes group, backslash escapes inside quotes).
pub fn tokenize(line: &str) -> Result<Vec<String>, ParseError> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_tok = false;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_tok = true;
                loop {
                    match chars.next() {
                        None => return Err(ParseError::UnterminatedQuote),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e) => cur.push(e),
                            None => return Err(ParseError::UnterminatedQuote),
                        },
                        Some(o) => cur.push(o),
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_tok {
                    out.push(core::mem::take(&mut cur));
                    in_tok = false;
                }
            }
            c => {
                in_tok = true;
                cur.push(c);
            }
        }
    }
    if in_tok {
        out.push(cur);
    }
    Ok(out)
}

/// Quote an argument so `tokenize` gives it back unchanged.
pub fn quote(arg: &str) -> String {
    let mut s = String::with_capacity(arg.len() + 2);
    s.push('"');
    for c in arg.chars() {
        if c == '"' || c == '\\' {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
    s
}

pub fn parse(line: &str) -> Result<Cmd, ParseError> {
    let line = line.trim();
    // `config set` takes the rest of the line verbatim (JSON has its own quotes).
    if let Some(rest) = line.strip_prefix("config set") {
        let json = rest.trim();
        if json.is_empty() {
            return Err(ParseError::Usage("config set <json>"));
        }
        return Ok(Cmd::ConfigSet(json.to_string()));
    }
    let t = tokenize(line)?;
    let words: Vec<&str> = t.iter().map(String::as_str).collect();
    match words.as_slice() {
        [] => Err(ParseError::Empty),
        ["wifi", "set", ssid] => Ok(Cmd::WifiSet {
            ssid: ssid.to_string(),
            password: String::new(),
        }),
        ["wifi", "set", ssid, pw] => Ok(Cmd::WifiSet {
            ssid: ssid.to_string(),
            password: pw.to_string(),
        }),
        ["wifi", "set", ..] => Err(ParseError::Usage("wifi set <ssid> [password]")),
        ["wifi", "clear"] => Ok(Cmd::WifiClear),
        ["wifi", ..] => Err(ParseError::Usage("wifi set <ssid> [password] | wifi clear")),
        ["status"] => Ok(Cmd::Status),
        ["reboot"] => Ok(Cmd::Reboot),
        ["config", "get"] => Ok(Cmd::ConfigGet),
        ["config", ..] => Err(ParseError::Usage("config get | config set <json>")),
        ["help"] => Ok(Cmd::Help),
        [c, ..] => Err(ParseError::Unknown(c.to_string())),
    }
}

/// Validate Wi-Fi credentials against 802.11 limits (the driver would reject them later, less clearly).
pub fn check_wifi(ssid: &str, password: &str) -> Result<(), &'static str> {
    if ssid.is_empty() || ssid.len() > 32 {
        return Err("SSID must be 1-32 bytes");
    }
    if !password.is_empty() && !(8..=64).contains(&password.len()) {
        return Err("password must be empty (open network) or 8-64 characters");
    }
    Ok(())
}

/// Wrap a JSON object as a console frame.
pub fn frame(json: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(json.len() + TAG.len() + 3);
    v.push(STX);
    v.extend_from_slice(TAG);
    v.extend_from_slice(json.as_bytes());
    v.push(ETX);
    v.push(b'\n');
    v
}

/// Assemble text lines from incoming bytes (host → device).
#[derive(Default)]
pub struct LineBuf {
    buf: Vec<u8>,
    overflow: bool,
}

impl LineBuf {
    /// Feed one byte; returns a complete line (without the newline) when one ends.
    /// An over-long line is dropped whole and reported as `Some(Err)`, so the
    /// caller can reply instead of acting on a truncated command.
    pub fn push(&mut self, b: u8) -> Option<Result<String, ParseError>> {
        match b {
            b'\n' | b'\r' => {
                if self.overflow {
                    self.overflow = false;
                    self.buf.clear();
                    return Some(Err(ParseError::Usage("line too long")));
                }
                if self.buf.is_empty() {
                    return None;
                }
                let line = String::from_utf8_lossy(&self.buf).into_owned();
                self.buf.clear();
                Some(Ok(line))
            }
            _ if self.overflow => None,
            _ => {
                if self.buf.len() >= MAX_LINE {
                    self.overflow = true;
                    self.buf.clear();
                } else {
                    self.buf.push(b);
                }
                None
            }
        }
    }
}

/// Pull console frames out of a mixed stream (frames + binary log + text).
#[derive(Default)]
pub struct Deframer {
    state: DeState,
    buf: Vec<u8>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum DeState {
    #[default]
    Seek,
    /// Matched STX and this many tag bytes.
    Tag(usize),
    Body,
}

impl Deframer {
    /// Feed bytes; returns the JSON payloads of the frames completed by them.
    pub fn push(&mut self, data: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for &b in data {
            match self.state {
                DeState::Seek => {
                    if b == STX {
                        self.state = DeState::Tag(0);
                    }
                }
                DeState::Tag(n) => {
                    if b == TAG[n] {
                        self.state = if n + 1 == TAG.len() {
                            self.buf.clear();
                            DeState::Body
                        } else {
                            DeState::Tag(n + 1)
                        };
                    } else {
                        self.state = if b == STX {
                            DeState::Tag(0)
                        } else {
                            DeState::Seek
                        };
                    }
                }
                DeState::Body => match b {
                    ETX => {
                        out.push(String::from_utf8_lossy(&self.buf).into_owned());
                        self.buf.clear();
                        self.state = DeState::Seek;
                    }
                    STX => self.state = DeState::Tag(0), // broken frame: restart
                    _ if self.buf.len() >= 64 * 1024 => self.state = DeState::Seek,
                    _ => self.buf.push(b),
                },
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn parses_commands() {
        assert_eq!(parse("status"), Ok(Cmd::Status));
        assert_eq!(parse("  reboot  "), Ok(Cmd::Reboot));
        assert_eq!(parse("wifi clear"), Ok(Cmd::WifiClear));
        assert_eq!(
            parse(r#"wifi set "My Net" "p a\"ss\\""#),
            Ok(Cmd::WifiSet {
                ssid: "My Net".into(),
                password: r#"p a"ss\"#.into()
            })
        );
        assert_eq!(
            parse("wifi set Open"),
            Ok(Cmd::WifiSet {
                ssid: "Open".into(),
                password: "".into()
            })
        );
        assert_eq!(
            parse(r#"config set {"mains_hz": 60}"#),
            Ok(Cmd::ConfigSet(r#"{"mains_hz": 60}"#.into()))
        );
        assert!(matches!(parse("wifi set"), Err(ParseError::Usage(_))));
        assert!(matches!(parse("frobnicate"), Err(ParseError::Unknown(_))));
        assert_eq!(parse(r#"wifi set "x"#), Err(ParseError::UnterminatedQuote));
        assert_eq!(parse(""), Err(ParseError::Empty));
    }

    #[test]
    fn quote_round_trips() {
        for s in ["plain", "with space", r#"q"uo\te"#, "", "ünï cødé"] {
            assert_eq!(tokenize(&quote(s)).unwrap(), vec![s.to_string()]);
        }
    }

    #[test]
    fn wifi_limits() {
        assert!(check_wifi("net", "").is_ok());
        assert!(check_wifi("net", "12345678").is_ok());
        assert!(check_wifi("net", "short").is_err());
        assert!(check_wifi("", "12345678").is_err());
        assert!(check_wifi(&"x".repeat(33), "").is_err());
    }

    #[test]
    fn line_buffer() {
        let mut lb = LineBuf::default();
        let mut got = Vec::new();
        for &b in b"status\r\n\nwifi clear\n" {
            if let Some(l) = lb.push(b) {
                got.push(l.unwrap());
            }
        }
        assert_eq!(got, vec!["status", "wifi clear"]);
        // over-long line: dropped with an error, the next line still works
        let mut lb = LineBuf::default();
        for _ in 0..MAX_LINE + 10 {
            assert!(lb.push(b'a').is_none());
        }
        assert!(matches!(lb.push(b'\n'), Some(Err(_))));
        for &b in b"status" {
            lb.push(b);
        }
        assert_eq!(lb.push(b'\n'), Some(Ok("status".into())));
    }

    #[test]
    fn deframes_mixed_stream() {
        let mut stream = vec![0xC0, 0xF7, 2, 0x02, 0x10, 0xFF]; // log bytes, including a stray STX
        stream.extend(frame(r#"{"ok":true}"#));
        stream.extend([0x02, b'C', b'O', 0x99]); // false start
        stream.extend(b"# text line\n");
        let f2 = frame(r#"{"event":"wifi","ip":"10.0.0.2"}"#);
        let mut d = Deframer::default();
        let mut got = d.push(&stream);
        // second frame split across two reads
        got.extend(d.push(&f2[..5]));
        got.extend(d.push(&f2[5..]));
        assert_eq!(
            got,
            vec![r#"{"ok":true}"#, r#"{"event":"wifi","ip":"10.0.0.2"}"#]
        );
    }
}
