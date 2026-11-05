//! Networking: Wi-Fi station, the HTTP server, the WebSocket telemetry stream,
//! OTA, and the `coffee.local` mDNS responder. All of it runs in the thread
//! executor, below the control loop.
//!
//! HTTP (port 80, `HTTP_WORKERS` connections at a time):
//! - `GET /api/ws`: WebSocket; the first message is the current telemetry, then
//!   every node event (`ServerMsg` JSON), as the PC engine does.
//! - `GET /api/device`: firmware status (version, network, heap...), as `status`
//!   on the USB console.
//! - `POST /api/ota`: body = an app image (`espflash save-image`); written to the
//!   other app slot, activated, then the board reboots into it.
//! - `/api/*`: queued for the control task, which owns the node (`link.rs`): it
//!   runs `Node::prepare_http`, the same API the PC engine uses, and this task
//!   encodes the reply. 503 if the queue is full, 504 if no reply in 1.5 s.
//! - anything else: the embedded UI (gzip), with `index.html` for unknown paths.

use crate::assets::ASSETS;
use crate::link::{self, LINK, TELEMETRY};
use crate::store::{self, WIFI_CHANGED};
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use coffee_api::ServerMsg;
use coffee_board::{http, mdns, ws};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_net::tcp::TcpSocket;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{IpAddress, IpEndpoint, Ipv4Address, Runner, Stack, StackResources};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{PubSubChannel, WaitResult};
use embassy_time::{Duration, Timer, with_timeout};
use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::{AuthenticationMethodConfig, Config, ControllerConfig, Interface, WifiController};
use static_cell::StaticCell;

const HOSTNAME: &str = "coffee";
const MDNS_NAME: &str = "coffee.local";
pub const HTTP_WORKERS: usize = 4;
/// Largest non-OTA request body (profiles and config are a few KB).
const MAX_BODY: usize = 32 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Node events for WebSocket clients. Each client subscribes; a slow client
/// lags (skips messages) instead of blocking anyone.
static EVENTS: PubSubChannel<CriticalSectionRawMutex, ServerMsg, 16, HTTP_WORKERS, 1> = PubSubChannel::new();

/// Called by the control loop for every node event. Never blocks.
pub fn publish(e: ServerMsg) {
    EVENTS.immediate_publisher().publish_immediate(e);
}

pub async fn start(spawner: Spawner, wifi: esp_hal::peripherals::WIFI<'static>) {
    let controller = match WifiController::new(wifi, ControllerConfig::default()) {
        Ok(c) => c,
        Err(e) => {
            store::update_net(|n| n.last_error = Some(alloc::format!("wifi init: {e:?}")));
            return; // no networking; the machine runs without it
        }
    };
    let iface = Interface::station();

    let mut dhcp = embassy_net::DhcpConfig::default();
    dhcp.hostname = heapless::String::try_from(HOSTNAME).ok();
    let cfg = embassy_net::Config::dhcpv4(dhcp);
    let rng = esp_hal::rng::Rng::new();
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;
    static RES: StaticCell<StackResources<{ HTTP_WORKERS + 3 }>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(iface, cfg, RES.init(StackResources::new()), seed);

    spawner.spawn(net_task(runner).unwrap());
    spawner.spawn(wifi_task(controller).unwrap());
    spawner.spawn(ip_watch(stack).unwrap());
    spawner.spawn(mdns_task(stack).unwrap());
    for id in 0..HTTP_WORKERS {
        spawner.spawn(http_worker(stack, id).unwrap());
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await
}

fn station_config(c: &store::WifiCreds) -> Option<Config> {
    let auth = if c.password.is_empty() {
        AuthenticationMethodConfig::Open
    } else {
        AuthenticationMethodConfig::WpaWpa2Personal(c.password.as_str().try_into().ok()?)
    };
    Some(Config::Station(
        StationConfig::default().with_ssid(c.ssid.as_str().try_into().ok()?).with_authentication(auth),
    ))
}

/// Keep the station connected to whatever credentials are stored; start over
/// whenever they change.
#[embassy_executor::task]
async fn wifi_task(mut wifi: WifiController<'static>) {
    loop {
        WIFI_CHANGED.reset();
        let Some(creds) = store::wifi_creds() else {
            store::update_net(|n| *n = store::NetState::default());
            let _ = wifi.disconnect_async().await;
            WIFI_CHANGED.wait().await;
            continue;
        };
        store::update_net(|n| {
            n.configured = true;
            n.ssid = Some(creds.ssid.clone());
            n.connected = false;
            n.ip = None;
        });
        let Some(cfg) = station_config(&creds) else {
            store::update_net(|n| n.last_error = Some("invalid credentials".into()));
            WIFI_CHANGED.wait().await;
            continue;
        };
        if let Err(e) = wifi.set_config(&cfg) {
            store::update_net(|n| n.last_error = Some(alloc::format!("config: {e:?}")));
            WIFI_CHANGED.wait().await;
            continue;
        }
        // Connect / stay connected until the credentials change.
        'session: loop {
            match select(wifi.connect_async(), WIFI_CHANGED.wait()).await {
                Either::Second(()) => break 'session,
                Either::First(Err(e)) => {
                    store::update_net(|n| {
                        n.connected = false;
                        n.last_error = Some(alloc::format!("connect: {e:?}"));
                    });
                    if let Either::Second(()) = select(Timer::after_secs(5), WIFI_CHANGED.wait()).await {
                        break 'session;
                    }
                }
                Either::First(Ok(_)) => {
                    store::update_net(|n| {
                        n.connected = true;
                        n.last_error = None;
                    });
                    crate::usb::event(&alloc::format!(r#"{{"event":"wifi","connected":true,"ssid":{:?}}}"#, creds.ssid));
                    loop {
                        match select3(wifi.wait_for_disconnect_async(), WIFI_CHANGED.wait(), Timer::after_secs(10)).await {
                            Either3::First(_) => {
                                store::update_net(|n| {
                                    n.connected = false;
                                    n.rssi = None;
                                });
                                crate::usb::event(r#"{"event":"wifi","connected":false}"#);
                                Timer::after_secs(2).await;
                                continue 'session;
                            }
                            Either3::Second(()) => break 'session,
                            Either3::Third(()) => {
                                let r = wifi.rssi().ok();
                                store::update_net(|n| n.rssi = r);
                            }
                        }
                    }
                }
            }
        }
        let _ = wifi.disconnect_async().await;
    }
}

/// Publish the DHCP address (status, USB event) as it comes and goes.
#[embassy_executor::task]
async fn ip_watch(stack: Stack<'static>) {
    let mut last: Option<Ipv4Address> = None;
    loop {
        let now = stack.config_v4().map(|c| c.address.address());
        if now != last {
            last = now;
            let ip = now.map(|a| a.to_string());
            store::update_net(|n| n.ip = ip.clone());
            crate::usb::event(&alloc::format!(r#"{{"event":"ip","ip":{}}}"#, ip.map(|i| alloc::format!("{i:?}")).unwrap_or("null".into())));
        }
        Timer::after_millis(500).await;
    }
}

/// Answer `coffee.local` A queries.
#[embassy_executor::task]
async fn mdns_task(stack: Stack<'static>) {
    let mut rx_meta = [PacketMetadata::EMPTY; 4];
    let mut tx_meta = [PacketMetadata::EMPTY; 4];
    let mut rx_buf = vec![0u8; 1024];
    let mut tx_buf = vec![0u8; 512];
    let group = Ipv4Address::new(mdns::GROUP[0], mdns::GROUP[1], mdns::GROUP[2], mdns::GROUP[3]);
    loop {
        stack.wait_config_up().await;
        let Some(ip) = stack.config_v4().map(|c| c.address.address()) else { continue };
        let _ = stack.join_multicast_group(group);
        let mut sock = UdpSocket::new(stack, &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf);
        if sock.bind(mdns::PORT).is_err() {
            Timer::after_secs(5).await;
            continue;
        }
        let answer = mdns::response(MDNS_NAME, ip.octets());
        let to_group = IpEndpoint::new(IpAddress::Ipv4(group), mdns::PORT);
        let _ = sock.send_to(&answer, to_group).await; // announce
        let mut pkt = [0u8; 512];
        loop {
            match with_timeout(Duration::from_secs(30), sock.recv_from(&mut pkt)).await {
                Ok(Ok((n, meta))) => {
                    if let Some(unicast) = mdns::wants(&pkt[..n], MDNS_NAME) {
                        let dest = if unicast { meta.endpoint } else { to_group };
                        let _ = sock.send_to(&answer, dest).await;
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => {
                    // address changed? start over with the new one
                    if stack.config_v4().map(|c| c.address.address()) != Some(ip) {
                        break;
                    }
                }
            }
        }
    }
}

async fn write_all(sock: &mut TcpSocket<'_>, mut data: &[u8]) -> Result<(), ()> {
    while !data.is_empty() {
        let n = with_timeout(IO_TIMEOUT, sock.write(data)).await.map_err(|_| ())?.map_err(|_| ())?;
        if n == 0 {
            return Err(());
        }
        data = &data[n..];
    }
    Ok(())
}

async fn respond(sock: &mut TcpSocket<'_>, status: u16, ctype: &str, extra: &str, body: &[u8]) {
    let _ = write_all(sock, &http::response_head(status, ctype, body.len(), extra)).await;
    let _ = write_all(sock, body).await;
}

fn json_err(msg: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"ok": false, "error": msg})).unwrap_or_default()
}

#[embassy_executor::task(pool_size = HTTP_WORKERS)]
async fn http_worker(stack: Stack<'static>, id: usize) {
    let mut client = LINK.client(id);
    let mut rx = vec![0u8; 2048];
    let mut tx = vec![0u8; 2048];
    loop {
        let mut sock = TcpSocket::new(stack, &mut rx, &mut tx);
        sock.set_timeout(Some(Duration::from_secs(30)));
        if sock.accept(80).await.is_err() {
            continue;
        }
        serve(&mut sock, &mut client).await;
        sock.close();
        let _ = with_timeout(Duration::from_secs(1), sock.flush()).await;
        sock.abort();
    }
}

async fn serve(sock: &mut TcpSocket<'_>, client: &mut link::NodeClient) {
    // ---- request head ----
    let mut buf = vec![0u8; http::MAX_HEAD];
    let mut have = 0;
    let req = loop {
        match http::parse_head(&buf[..have]) {
            Ok(r) => break r,
            Err(http::HeadError::Incomplete) => {}
            Err(_) => return respond(sock, 400, "application/json", "", &json_err("bad request")).await,
        }
        if have == buf.len() {
            return respond(sock, 413, "application/json", "", &json_err("head too large")).await;
        }
        match with_timeout(IO_TIMEOUT, sock.read(&mut buf[have..])).await {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return,
            Ok(Ok(n)) => have += n,
        }
    };
    let leftover = buf[req.head_len..have].to_vec();
    let path_only = req.path.split('?').next().unwrap_or("/");

    if path_only == "/api/ws" {
        return match &req.ws_key {
            Some(k) => websocket(sock, k).await,
            None => respond(sock, 400, "application/json", "", &json_err("websocket upgrade required")).await,
        };
    }
    if req.method == "POST" && path_only == "/api/ota" {
        return ota(sock, req.content_length, leftover).await;
    }
    if path_only == "/api/device" {
        let body = crate::usb::status_json().to_string();
        return respond(sock, 200, "application/json", "Cache-Control: no-store\r\n", body.as_bytes()).await;
    }
    if path_only.starts_with("/api/") {
        if req.content_length > MAX_BODY {
            return respond(sock, 413, "application/json", "", &json_err("body too large")).await;
        }
        let mut body = leftover;
        while body.len() < req.content_length {
            let mut chunk = [0u8; 1024];
            match with_timeout(IO_TIMEOUT, sock.read(&mut chunk)).await {
                Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return,
                Ok(Ok(n)) => body.extend_from_slice(&chunk[..n]),
            }
        }
        body.truncate(req.content_length);
        // The control task routes it and copies out what the reply needs; the JSON
        // encoding happens here, in this worker.
        let r = link::http(client, &req.method, &req.path, body).await.unwrap_or_else(link::error_response);
        return respond(sock, r.status, "application/json", "Cache-Control: no-store\r\n", &r.body).await;
    }

    // ---- static UI ----
    if req.method != "GET" && req.method != "HEAD" {
        return respond(sock, 405, "application/json", "", &json_err("method not allowed")).await;
    }
    let want = if path_only == "/" { "/index.html" } else { path_only };
    let (name, gz) = ASSETS
        .iter()
        .find(|(n, _)| *n == want)
        .or_else(|| ASSETS.iter().find(|(n, _)| *n == "/index.html"))
        .copied()
        .unwrap_or(("/index.html", &[]));
    let (ctype, cache) = http::static_meta(name);
    let extra = alloc::format!("Content-Encoding: gzip\r\nCache-Control: {cache}\r\nVary: Accept-Encoding\r\n");
    let body: &[u8] = if req.method == "HEAD" { &[] } else { gz };
    let _ = write_all(sock, &http::response_head(200, ctype, gz.len(), &extra)).await;
    let _ = write_all(sock, body).await;
}

async fn websocket(sock: &mut TcpSocket<'_>, key: &str) {
    let Ok(mut sub) = EVENTS.subscriber() else {
        return respond(sock, 503, "application/json", "", &json_err("too many websocket clients")).await;
    };
    if write_all(sock, &ws::handshake_response(key)).await.is_err() {
        return;
    }
    if let Some(t) = TELEMETRY.get()
        && let Ok(s) = serde_json::to_vec(&ServerMsg::Telemetry(t))
        && write_all(sock, &ws::encode(ws::OP_TEXT, &s)).await.is_err()
    {
        return;
    }
    let mut inbuf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        match select(sub.next_message(), sock.read(&mut chunk)).await {
            Either::First(WaitResult::Lagged(_)) => continue,
            Either::First(WaitResult::Message(m)) => {
                let Ok(s) = serde_json::to_vec(&m) else { continue };
                if write_all(sock, &ws::encode(ws::OP_TEXT, &s)).await.is_err() {
                    return;
                }
            }
            Either::Second(Ok(0)) | Either::Second(Err(_)) => return,
            Either::Second(Ok(n)) => {
                inbuf.extend_from_slice(&chunk[..n]);
                loop {
                    match ws::decode(&inbuf, 4096) {
                        Err(_) => return,
                        Ok(None) => break,
                        Ok(Some((f, used))) => {
                            inbuf.drain(..used);
                            match f.opcode {
                                ws::OP_CLOSE => {
                                    let _ = write_all(sock, &ws::encode(ws::OP_CLOSE, &[])).await;
                                    return;
                                }
                                ws::OP_PING
                                    if write_all(sock, &ws::encode(ws::OP_PONG, &f.payload)).await.is_err() =>
                                {
                                    return;
                                }
                                _ => {} // the UI doesn't send anything else
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Stream an app image into the other OTA slot, activate it, reboot.
async fn ota(sock: &mut TcpSocket<'_>, len: usize, first: Vec<u8>) {
    let (base, size) = match store::ota_target() {
        Ok(t) => t,
        Err(e) => return respond(sock, 500, "application/json", "", &json_err(&e)).await,
    };
    if len == 0 || len > size as usize {
        let m = alloc::format!("image is {len} bytes; the slot holds {size}");
        return respond(sock, 413, "application/json", "", &json_err(&m)).await;
    }
    if store::BUSY.load(core::sync::atomic::Ordering::Relaxed) {
        return respond(sock, 409, "application/json", "", &json_err("a shot is running")).await;
    }
    if !store::OTA_ARMED.load(core::sync::atomic::Ordering::Relaxed) {
        let m = "OTA needs physical presence: power rocker off and steam rocker on, then upload";
        return respond(sock, 403, "application/json", "", &json_err(m)).await;
    }
    const SECTOR: usize = 4096;
    let mut pending = first;
    let mut written = 0usize;
    let mut got = pending.len();
    let mut chunk = vec![0u8; 1460];
    loop {
        while pending.len() >= SECTOR || (got >= len && !pending.is_empty()) {
            let take = pending.len().min(SECTOR);
            if written == 0 && pending[0] != 0xE9 {
                return respond(sock, 400, "application/json", "", &json_err("not an ESP app image (use espflash save-image)")).await;
            }
            if let Err(e) = store::ota_write(base + written as u32, &pending[..take]) {
                return respond(sock, 500, "application/json", "", &json_err(&e)).await;
            }
            written += take;
            pending.drain(..take);
        }
        if got >= len {
            break;
        }
        match with_timeout(IO_TIMEOUT, sock.read(&mut chunk)).await {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => return, // incomplete upload: old slot stays active
            Ok(Ok(n)) => {
                let n = n.min(len - got);
                pending.extend_from_slice(&chunk[..n]);
                got += n;
            }
        }
    }
    if let Err(e) = store::ota_activate() {
        return respond(sock, 500, "application/json", "", &json_err(&e)).await;
    }
    let body = alloc::format!(r#"{{"ok":true,"written":{written},"rebooting":true}}"#);
    respond(sock, 200, "application/json", "", body.as_bytes()).await;
    let _ = with_timeout(Duration::from_secs(1), sock.flush()).await;
    Timer::after_millis(300).await;
    store::reboot();
}

