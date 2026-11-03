//! The firmware's instance of `coffee_board::link`: how the network side reaches the
//! node, which the control task owns outright.
//!
//! - `/api/*` requests and the USB console's `config get`/`config set` are queued
//!   as [`HttpReq`]s. Once per mains cycle, after outputs are applied, the control
//!   task handles up to [`MAX_PER_CYCLE`] of them within [`BUDGET_US`], calling
//!   `Node::prepare_http`. That only routes the request and copies out what the
//!   reply needs. The caller turns the `PreparedHttp` into JSON (`finish`) in its
//!   own task.
//! - Telemetry for `/api/device`, the USB `status` command and the first WebSocket
//!   message comes from [`TELEMETRY`], replaced by the control task on every
//!   telemetry event.
//! - Node events go to WebSocket clients through `net::publish` (a pub/sub channel
//!   that never blocks the publisher).

use alloc::string::String;
use alloc::vec::Vec;
use coffee_api::Telemetry;
use coffee_board::link::{Client, Latest, Link, LinkError};
use coffee_node::{HttpResponse, PreparedHttp};
use embassy_time::{Duration, Timer};

/// One request slot per HTTP worker, plus the USB console.
pub const CLIENTS: usize = crate::net::HTTP_WORKERS + 1;
pub const USB_CLIENT: usize = crate::net::HTTP_WORKERS;
/// Requests that may wait for the control task; more get 503.
const DEPTH: usize = 8;
/// Per-cycle limits for the control task (a cycle is 16.7 ms at 60 Hz).
pub const MAX_PER_CYCLE: usize = 2;
pub const BUDGET_US: u64 = 1_000;
/// How long a caller waits for its reply.
const TIMEOUT: Duration = Duration::from_millis(1500);

pub struct HttpReq {
    pub method: String,
    pub path: String,
    pub body: Vec<u8>,
}

pub type NodeLink = Link<HttpReq, PreparedHttp, DEPTH, CLIENTS>;
pub type NodeClient = Client<'static, HttpReq, PreparedHttp, DEPTH, CLIENTS>;

pub static LINK: NodeLink = Link::new();
pub static TELEMETRY: Latest<Telemetry> = Latest::new();

/// Run one API request through the control task. `Err` maps to 503 (queue full) or
/// 504 (no reply in time).
pub async fn http(client: &mut NodeClient, method: &str, path: &str, body: Vec<u8>) -> Result<HttpResponse, LinkError> {
    let req = HttpReq { method: method.into(), path: path.into(), body };
    client.call(req, Timer::after(TIMEOUT)).await.map(PreparedHttp::finish)
}

pub fn error_response(e: LinkError) -> HttpResponse {
    let (status, msg) = match e {
        LinkError::Busy => (503, "controller busy, try again"),
        LinkError::Timeout => (504, "controller did not answer in time"),
    };
    HttpResponse {
        status,
        body: serde_json::to_vec(&serde_json::json!({"ok": false, "error": msg})).unwrap_or_default(),
    }
}
