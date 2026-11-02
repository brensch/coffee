//! `coffee-engine`: the PC shell around [`coffee_node::Node`].
//!
//! All behaviour lives in `coffee-node` and is identical to the ESP32 firmware.
//! This binary only provides plumbing: the lockstep link to the emulator (stdio
//! or TCP), file storage, an axum HTTP server that passes API requests straight
//! to `Node::handle_http`, and static hosting for the UI.
//!
//! ```text
//! coffee-engine --emu tcp://127.0.0.1:7878 --data data --http 127.0.0.1:8080
//! coffee-engine --emu stdio --http off          # spawned by `coffee-emu run`
//! ```

mod http;
mod link;
mod storage;

use anyhow::{Context, Result};
use clap::Parser;
use coffee_api::Info;
use coffee_core::{EngineConfig, Profile};
use coffee_node::{MemStorage, Node, NodeOptions, Storage};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

#[derive(Parser, Debug, Clone)]
#[command(version, about = "Espresso engine (PC shell)")]
pub struct Args {
    /// Link to the machine: `stdio` (spawned by the emulator) or `tcp://host:port`.
    #[arg(long, default_value = "tcp://127.0.0.1:7878")]
    pub emu: String,
    /// Data directory for user profiles, config, active profile and shots.
    /// Omit to keep everything in memory (what scenario runs use).
    #[arg(long)]
    pub data: Option<PathBuf>,
    /// Engine config TOML applied at startup (not persisted).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Active profile at startup: a library name or a path to a JSON file.
    #[arg(long)]
    pub profile: Option<String>,
    /// HTTP listen address for UI + API, or `off`.
    #[arg(long, default_value = "127.0.0.1:8080")]
    pub http: String,
    /// Built UI to serve (from `trunk build` in ui/).
    #[arg(long, default_value = "ui/dist")]
    pub ui: PathBuf,
    /// Write the per-tick binary log (`.clog`, what `coffee-emu fit` consumes).
    #[arg(long)]
    pub log: Option<PathBuf>,
    /// Advertise the emulator lab API to the UI.
    #[arg(long)]
    pub lab_url: Option<String>,
}

pub type SharedNode = Node<Box<dyn Storage + Send>>;

pub struct AppState {
    pub node: Mutex<SharedNode>,
    pub tx: broadcast::Sender<String>,
    pub args: Args,
}

pub type App = Arc<AppState>;

fn main() -> Result<()> {
    let args = Args::parse();
    let storage: Box<dyn Storage + Send> = match &args.data {
        Some(dir) => Box::new(storage::FsStorage::new(dir.clone())?),
        None => Box::new(MemStorage::default()),
    };
    let mut node = Node::new(
        storage,
        NodeOptions {
            info: Info {
                name: "coffee".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                platform: "host".into(),
                emulated: true,
                lab_url: args.lab_url.clone(),
            },
            max_shots: 200,
            persist_shots: args.data.is_some(),
            history_capacity: 3000, // 10 min at 5 Hz
            history_hz: 5.0,
        },
    );
    if let Some(p) = &args.config {
        let cfg: EngineConfig = toml::from_str(
            &std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?,
        )
        .with_context(|| format!("parsing {}", p.display()))?;
        node.set_config(cfg).map_err(anyhow::Error::msg)?;
    }
    if let Some(sel) = &args.profile {
        let prof = match node.library().iter().find(|p| &p.name == sel) {
            Some(p) => p.clone(),
            None => {
                let txt = std::fs::read_to_string(sel).with_context(|| {
                    format!("profile `{sel}` is not in the library and not a file")
                })?;
                serde_json::from_str::<Profile>(&txt).with_context(|| format!("parsing {sel}"))?
            }
        };
        node.set_profile(prof).map_err(anyhow::Error::msg)?;
    }

    let (tx, _) = broadcast::channel(1024);
    let app: App = Arc::new(AppState {
        node: Mutex::new(node),
        tx,
        args: args.clone(),
    });

    let link_app = app.clone();
    let link = std::thread::Builder::new()
        .name("link".into())
        .spawn(move || link::run(link_app))?;

    if args.http != "off" {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        rt.block_on(async move {
            tokio::select! {
                r = http::serve(app.clone()) => r,
                _ = tokio::signal::ctrl_c() => Ok(()),
                _ = async {
                    while !link.is_finished() { tokio::time::sleep(std::time::Duration::from_millis(100)).await; }
                } => Ok(()),
            }
        })?;
    } else {
        link.join()
            .map_err(|_| anyhow::anyhow!("link thread panicked"))??;
    }
    Ok(())
}
