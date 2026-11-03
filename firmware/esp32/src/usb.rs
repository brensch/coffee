//! USB Serial/JTAG: the `.clog` log stream out, the provisioning console in.
//!
//! Everything the device sends goes through one non-blocking queue (`push`):
//! log chunks from the control loop, and console replies and events as framed
//! JSON (`coffee_board::console::frame`). When nobody reads the port, whole
//! chunks are dropped, so the control loop never blocks on USB.
//!
//! Commands arrive as text lines (see `coffee_board::console`):
//! `wifi set <ssid> [password]`, `wifi clear`, `status`, `config get`,
//! `config set <json>`, `reboot`, `help`. Every command gets exactly one reply
//! frame: `{"ok":true,...}` or `{"ok":false,"error":"..."}`.

use crate::link::{self, LINK, TELEMETRY, USB_CLIENT};
use crate::store::{self, WifiCreds};
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use coffee_board::console::{self, Cmd, LineBuf};
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;
use serde_json::json;

const TX_QUEUE_BYTES: usize = 8 * 1024;
static TX: Mutex<RefCell<VecDeque<u8>>> = Mutex::new(RefCell::new(VecDeque::new()));

/// Queue a whole chunk for the host, or drop it if it doesn't fit.
pub fn push(chunk: &[u8]) {
    critical_section::with(|cs| {
        let mut q = TX.borrow_ref_mut(cs);
        if q.len() + chunk.len() <= TX_QUEUE_BYTES {
            q.extend(chunk);
        }
    })
}

/// Send a JSON object as a console frame (events and replies).
pub fn event(json: &str) {
    push(&console::frame(json));
}

fn reply(v: serde_json::Value) {
    event(&v.to_string());
}

fn err(msg: impl core::fmt::Display) {
    reply(json!({"ok": false, "error": alloc::format!("{msg}")}));
}

/// Firmware status: version, uptime, network, live telemetry (from the control
/// task's snapshot; this never waits on the control task).
pub fn status_json() -> serde_json::Value {
    let t = TELEMETRY.get().unwrap_or_default();
    json!({
        "ok": true,
        "version": crate::VERSION,
        "platform": "esp32c6",
        "uptime_s": Instant::now().as_secs(),
        "heap_free": esp_alloc::HEAP.free(),
        "storage_pending": store::with_kv(|k| k.pending()),
        "wifi": store::net_state(),
        "state": t.state,
        "temp_c": t.temp_c,
        "pressure_bar": t.pressure_bar,
        "faults": t.faults,
        "api_queue": LINK.queued(),
    })
}

async fn handle(line: &str, client: &mut link::NodeClient) {
    let cmd = match console::parse(line) {
        Ok(c) => c,
        Err(e) => return err(e),
    };
    match cmd {
        Cmd::Help => reply(json!({"ok": true, "help": console::HELP})),
        Cmd::Status => reply(status_json()),
        Cmd::WifiSet { ssid, password } => {
            if let Err(e) = console::check_wifi(&ssid, &password) {
                return err(e);
            }
            match store::set_wifi(Some(&WifiCreds { ssid: ssid.clone(), password })) {
                Ok(()) => reply(json!({"ok": true, "ssid": ssid, "note": "connecting; poll status for wifi.ip"})),
                Err(e) => err(e),
            }
        }
        Cmd::WifiClear => match store::set_wifi(None) {
            Ok(()) => reply(json!({"ok": true})),
            Err(e) => err(e),
        },
        Cmd::ConfigGet => {
            let r = match link::http(client, "GET", "/api/config", alloc::vec::Vec::new()).await {
                Ok(r) => r,
                Err(e) => return err(alloc::format!("{e:?}")),
            };
            let cfg: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            reply(json!({"ok": true, "config": cfg}))
        }
        Cmd::ConfigSet(j) => {
            let r = match link::http(client, "PUT", "/api/config", j.into_bytes()).await {
                Ok(r) => r,
                Err(e) => return err(alloc::format!("{e:?}")),
            };
            let body: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            if r.status == 200 {
                reply(json!({"ok": true}))
            } else {
                err(body.get("error").and_then(|e| e.as_str()).map(String::from).unwrap_or_else(|| alloc::format!("HTTP {}", r.status)))
            }
        }
        Cmd::Reboot => {
            reply(json!({"ok": true}));
            Timer::after(Duration::from_millis(100)).await; // let the reply out
            store::reboot()
        }
    }
}

/// Poll the port every 2 ms: drain received bytes into lines, send queued bytes.
#[embassy_executor::task]
pub async fn run(dev: esp_hal::peripherals::USB_DEVICE<'static>) {
    let mut usb = UsbSerialJtag::new(dev);
    let mut client = LINK.client(USB_CLIENT);
    let mut lines = LineBuf::default();
    loop {
        while let Ok(b) = usb.read_byte() {
            if let Some(l) = lines.push(b) {
                match l {
                    Ok(line) => handle(&line, &mut client).await,
                    Err(e) => err(e),
                }
            }
        }
        let sent = critical_section::with(|cs| {
            let mut q = TX.borrow_ref_mut(cs);
            let mut sent = false;
            while let Some(&b) = q.front() {
                if usb.write_byte_nb(b).is_err() {
                    break;
                }
                q.pop_front();
                sent = true;
            }
            sent
        });
        if sent {
            let _ = usb.flush_tx_nb();
        }
        Timer::after(Duration::from_millis(2)).await;
    }
}
