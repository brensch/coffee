//! Talking to the board: the USB console (framed JSON replies in a stream that
//! also carries the binary log), plain HTTP to `/api/*`, and `.local` resolution.

use anyhow::{Context, Result, anyhow, bail};
use coffee_board::console::{Deframer, quote};
use coffee_board::mdns;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

/// Espressif's USB vendor id (the C6's built-in USB Serial/JTAG).
pub const ESPRESSIF_VID: u16 = 0x303a;

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub path: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub product: Option<String>,
    pub espressif: bool,
}

impl PortInfo {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "path": self.path,
            "vid": self.vid.map(|v| format!("{v:04x}")),
            "pid": self.pid.map(|v| format!("{v:04x}")),
            "product": self.product,
            "espressif": self.espressif,
        })
    }
}

/// Serial ports, Espressif devices first.
pub fn ports() -> Vec<PortInfo> {
    let mut out: Vec<PortInfo> = Vec::new();
    // Linux: sysfs gives USB ids without libudev.
    if let Ok(rd) = std::fs::read_dir("/sys/class/tty") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !(name.starts_with("ttyACM") || name.starts_with("ttyUSB")) {
                continue;
            }
            let dev = e.path().join("device");
            let usb = if name.starts_with("ttyACM") { dev.join("..") } else { dev.join("../..") };
            let rd_hex = |f: &str| {
                std::fs::read_to_string(usb.join(f)).ok().and_then(|s| u16::from_str_radix(s.trim(), 16).ok())
            };
            let vid = rd_hex("idVendor");
            out.push(PortInfo {
                path: format!("/dev/{name}"),
                vid,
                pid: rd_hex("idProduct"),
                product: std::fs::read_to_string(usb.join("product")).ok().map(|s| s.trim().to_string()),
                espressif: vid == Some(ESPRESSIF_VID),
            });
        }
    }
    // Elsewhere (macOS, Windows): the serialport crate's own enumeration.
    if out.is_empty()
        && let Ok(list) = serialport::available_ports()
    {
        for p in list {
            let (vid, pid, product) = match &p.port_type {
                serialport::SerialPortType::UsbPort(u) => (Some(u.vid), Some(u.pid), u.product.clone()),
                _ => (None, None, None),
            };
            out.push(PortInfo {
                path: p.port_name,
                vid,
                pid,
                product,
                espressif: vid == Some(ESPRESSIF_VID),
            });
        }
    }
    out.sort_by_key(|p| (!p.espressif, p.path.clone()));
    out
}

/// The port to use: the one given, else the only Espressif device.
pub fn pick_port(given: Option<&str>) -> Result<String> {
    if let Some(p) = given {
        return Ok(p.to_string());
    }
    let esp: Vec<PortInfo> = ports().into_iter().filter(|p| p.espressif).collect();
    match esp.as_slice() {
        [one] => Ok(one.path.clone()),
        [] => Err(anyhow!(NoDevice("no Espressif USB device found (plug the board in; on WSL attach it with usbipd)".into()))),
        many => Err(anyhow!(NoDevice(format!(
            "several Espressif devices: {}; pick one with --port",
            many.iter().map(|p| p.path.as_str()).collect::<Vec<_>>().join(", ")
        )))),
    }
}

/// A missing device or tool (exit code 3).
#[derive(Debug)]
pub struct NoDevice(pub String);
impl std::fmt::Display for NoDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for NoDevice {}

/// Arguments the board would reject (exit code 2, like a usage error).
#[derive(Debug)]
pub struct BadArgs(pub String);
impl std::fmt::Display for BadArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for BadArgs {}

/// Something didn't answer in time (exit code 4).
#[derive(Debug)]
pub struct TimedOut(pub String);
impl std::fmt::Display for TimedOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for TimedOut {}

pub fn open(path: &str) -> Result<Box<dyn serialport::SerialPort>> {
    serialport::new(path, 115_200)
        .timeout(Duration::from_millis(50))
        .open()
        .with_context(|| format!("opening {path}"))
}

/// Send one console command and wait for its reply frame (`{"ok":...}`).
/// Event frames and log bytes in between are skipped (events go to `on_event`).
/// The command is re-sent once if nothing answers within half the timeout:
/// opening the port can reset the chip, and it needs ~1 s to boot.
pub fn request<P: Read + Write + ?Sized>(
    port: &mut P,
    line: &str,
    timeout: Duration,
    on_event: &mut dyn FnMut(&Value),
) -> Result<Value> {
    let start = Instant::now();
    let mut sent_twice = false;
    port.write_all(format!("{line}\n").as_bytes())?;
    port.flush()?;
    let mut de = Deframer::default();
    let mut buf = [0u8; 4096];
    while start.elapsed() < timeout {
        if !sent_twice && start.elapsed() > timeout / 2 {
            port.write_all(format!("{line}\n").as_bytes())?;
            port.flush()?;
            sent_twice = true;
        }
        let n = match port.read(&mut buf) {
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut || e.kind() == std::io::ErrorKind::WouldBlock => 0,
            Err(e) => return Err(e.into()),
        };
        for f in de.push(&buf[..n]) {
            let Ok(v) = serde_json::from_str::<Value>(&f) else { continue };
            if v.get("ok").is_some() {
                return Ok(v);
            }
            on_event(&v);
        }
        if n == 0 {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Err(anyhow!(TimedOut(format!("no reply to '{}' within {:?}", line.split_whitespace().next().unwrap_or(""), timeout))))
}

/// `wifi set` as a console line (arguments quoted so spaces and quotes survive).
pub fn wifi_set_line(ssid: &str, password: &str) -> String {
    if password.is_empty() {
        format!("wifi set {}", quote(ssid))
    } else {
        format!("wifi set {} {}", quote(ssid), quote(password))
    }
}

/// Resolve `host` (IP, DNS name, or `name.local` via our own mDNS query when the
/// system resolver can't).
pub fn resolve(host: &str) -> Result<SocketAddr> {
    let (name, port) = match host.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h, p.parse().unwrap()),
        _ => (host, 80),
    };
    if let Ok(mut it) = (name, port).to_socket_addrs()
        && let Some(a) = it.find(|a| a.is_ipv4())
    {
        return Ok(a);
    }
    if name.ends_with(".local") {
        let ip = mdns_lookup(name, Duration::from_secs(3))?;
        return Ok(SocketAddr::from((ip, port)));
    }
    bail!(NoDevice(format!("can't resolve {name}")))
}

fn mdns_lookup(name: &str, timeout: Duration) -> Result<Ipv4Addr> {
    let sock = UdpSocket::bind(("0.0.0.0", 0))?;
    sock.set_read_timeout(Some(Duration::from_millis(300)))?;
    let q = mdns::query(name);
    let start = Instant::now();
    let mut buf = [0u8; 1500];
    while start.elapsed() < timeout {
        sock.send_to(&q, (Ipv4Addr::from(mdns::GROUP), mdns::PORT))?;
        let until = Instant::now() + Duration::from_millis(600);
        while Instant::now() < until {
            if let Ok((n, _)) = sock.recv_from(&mut buf)
                && let Some(ip) = mdns::parse_a(&buf[..n], name)
            {
                return Ok(Ipv4Addr::from(ip));
            }
        }
    }
    Err(anyhow!(NoDevice(format!("{name} didn't answer mDNS (is the board on Wi-Fi? try --host <ip> from `coffee status`)"))))
}

/// Minimal HTTP/1.1 request (no TLS, no chunked bodies: the board doesn't use them).
pub fn http(host: &str, method: &str, path: &str, body: &[u8], timeout: Duration) -> Result<(u16, Vec<u8>)> {
    let addr = resolve(host)?;
    let mut s = TcpStream::connect_timeout(&addr, timeout.min(Duration::from_secs(10)))
        .map_err(|e| anyhow!(NoDevice(format!("can't reach the board at {addr}: {e}"))))?;
    s.set_read_timeout(Some(timeout))?;
    s.set_write_timeout(Some(timeout))?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes())?;
    s.write_all(body)?;
    let mut resp = Vec::new();
    s.read_to_end(&mut resp).context("reading response")?;
    parse_response(&resp)
}

/// Split an HTTP response into (status, body).
pub fn parse_response(resp: &[u8]) -> Result<(u16, Vec<u8>)> {
    let end = resp.windows(4).position(|w| w == b"\r\n\r\n").context("truncated response")?;
    let head = std::str::from_utf8(&resp[..end]).context("bad response head")?;
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .context("bad status line")?;
    Ok((status, resp[end + 4..].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use coffee_board::console::{self, frame};
    use std::collections::VecDeque;

    /// A fake board: replies to each received line using `answer`, with log noise around it.
    struct Fake {
        rx: Vec<u8>,
        tx: VecDeque<u8>,
        answer: fn(&str) -> Option<String>,
        lines: Vec<String>,
    }
    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.tx.len()).min(7); // dribble it out in small reads
            for b in buf.iter_mut().take(n) {
                *b = self.tx.pop_front().unwrap();
            }
            Ok(n)
        }
    }
    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            for &b in buf {
                if b == b'\n' {
                    let line = String::from_utf8(std::mem::take(&mut self.rx)).unwrap();
                    self.lines.push(line.clone());
                    self.tx.extend([0xC0, 0xF7, 2, 9, 0, 1, 2, 3]); // a .clog chunk header
                    self.tx.extend(frame(r#"{"event":"ip","ip":"10.0.0.9"}"#));
                    if let Some(r) = (self.answer)(&line) {
                        self.tx.extend(frame(&r));
                    }
                } else {
                    self.rx.push(b);
                }
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn fake(answer: fn(&str) -> Option<String>) -> Fake {
        Fake { rx: vec![], tx: VecDeque::new(), answer, lines: vec![] }
    }

    #[test]
    fn request_skips_log_and_events() {
        let mut f = fake(|l| (l == "status").then(|| r#"{"ok":true,"version":"0.1.0"}"#.to_string()));
        let mut events = vec![];
        let v = request(&mut f, "status", Duration::from_secs(2), &mut |e| events.push(e.clone())).unwrap();
        assert_eq!(v["version"], "0.1.0");
        assert_eq!(events[0]["event"], "ip");
    }

    #[test]
    fn request_times_out_and_retries_once() {
        let mut f = fake(|_| None);
        let e = request(&mut f, "status", Duration::from_millis(300), &mut |_| {}).unwrap_err();
        assert!(e.downcast_ref::<TimedOut>().is_some());
        assert_eq!(f.lines, vec!["status", "status"]);
    }

    #[test]
    fn wifi_line_survives_the_device_parser() {
        let line = wifi_set_line(r#"Cafe "Net""#, "pa ss\\word");
        assert_eq!(
            console::parse(&line).unwrap(),
            console::Cmd::WifiSet { ssid: r#"Cafe "Net""#.into(), password: "pa ss\\word".into() }
        );
        assert_eq!(
            console::parse(&wifi_set_line("open", "")).unwrap(),
            console::Cmd::WifiSet { ssid: "open".into(), password: "".into() }
        );
    }

    #[test]
    fn parses_http_responses() {
        let (s, b) = parse_response(b"HTTP/1.1 404 Not Found\r\nContent-Length: 2\r\n\r\n{}").unwrap();
        assert_eq!((s, b.as_slice()), (404, &b"{}"[..]));
        assert!(parse_response(b"HTTP/1.1 200").is_err());
    }

    #[test]
    fn resolves_literal_addresses() {
        assert_eq!(resolve("127.0.0.1").unwrap(), "127.0.0.1:80".parse().unwrap());
        assert_eq!(resolve("127.0.0.1:8080").unwrap(), "127.0.0.1:8080".parse().unwrap());
    }
}
