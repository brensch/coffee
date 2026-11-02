//! HTTP plumbing: `/api/ws` streams node events; every other `/api/*` request is
//! passed verbatim to `Node::handle_http` (the same function the firmware uses);
//! anything else is the static UI.

use crate::App;
use anyhow::Result;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::extract::ws::{Message as WsMsg, WebSocket, WebSocketUpgrade};
use axum::http::{Method, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use coffee_api::ServerMsg;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

pub async fn serve(app: App) -> Result<()> {
    let ui = app.args.ui.clone();
    let mut router = Router::new()
        .route("/api/ws", get(ws))
        .route("/api/{*rest}", any(api))
        .with_state(app.clone());
    if ui.join("index.html").exists() {
        router = router
            .fallback_service(ServeDir::new(&ui).fallback(ServeFile::new(ui.join("index.html"))));
    } else {
        router = router.fallback(get(no_ui));
    }
    // index.html must never be served stale (it names the hashed wasm/js bundles,
    // which are immutable and can be cached forever).
    let router = router
        .layer(axum::middleware::map_response(cache_headers))
        .layer(CorsLayer::permissive());
    let listener = tokio::net::TcpListener::bind(&app.args.http).await?;
    eprintln!("[engine] UI + API on http://{}", app.args.http);
    axum::serve(listener, router).await?;
    Ok(())
}

async fn cache_headers(uri: Uri, mut resp: Response) -> Response {
    let p = uri.path();
    let v = if p.starts_with("/api/") {
        "no-store"
    } else if p.ends_with(".wasm") || p.ends_with(".js") || p.ends_with(".css") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, header::HeaderValue::from_static(v));
    resp
}

async fn no_ui() -> Html<&'static str> {
    Html(
        "<!doctype html><title>coffee</title><body style='font-family:system-ui;padding:2rem'>\
         <h1>coffee-engine is running</h1><p>The UI isn't built yet. Run <code>trunk build --release</code> \
         in <code>ui/</code>, or use the API at <a href='/api/status'>/api/status</a>.</p>",
    )
}

async fn api(State(app): State<App>, method: Method, uri: Uri, body: Bytes) -> Response {
    let r = app
        .node
        .lock()
        .unwrap()
        .handle_http(method.as_str(), uri.path(), &body);
    Response::builder()
        .status(StatusCode::from_u16(r.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r.body))
        .unwrap()
        .into_response()
}

async fn ws(State(app): State<App>, up: WebSocketUpgrade) -> Response {
    up.on_upgrade(move |sock| ws_session(app, sock))
}

async fn ws_session(app: App, mut sock: WebSocket) {
    let mut rx = app.tx.subscribe();
    let first = serde_json::to_string(&ServerMsg::Telemetry(
        app.node.lock().unwrap().telemetry().clone(),
    ))
    .unwrap();
    if sock.send(WsMsg::Text(first.into())).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            m = rx.recv() => match m {
                Ok(s) => if sock.send(WsMsg::Text(s.into())).await.is_err() { return },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return,
            },
            incoming = sock.recv() => match incoming {
                Some(Ok(WsMsg::Close(_))) | None | Some(Err(_)) => return,
                _ => {}
            }
        }
    }
}
