//! `coffee`: flash, provision and inspect the controller board.
//!
//! Built to be driven by scripts and AI agents: nothing is interactive, every
//! wait has a timeout, `--json` prints exactly one JSON object on stdout, and
//! the exit code says what happened:
//!
//! | code | meaning                                             |
//! |------|-----------------------------------------------------|
//! | 0    | done                                                |
//! | 1    | failed (the board said no, a build/flash failed)    |
//! | 2    | bad arguments                                       |
//! | 3    | no board / port / tool found                        |
//! | 4    | timed out waiting for the board                     |
//!
//! See `firmware/esp32/README.md` for the blank-board-to-brewing sequence.

mod device;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use device::{BadArgs, NoDevice, TimedOut};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(name = "coffee", version, about = "Flash, provision and inspect the coffee controller (ESP32-C6)")]
struct Cli {
    /// Print one JSON object on stdout instead of text.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// List serial ports (Espressif boards first).
    Ports,
    /// Build the firmware and flash it over USB (needs `espflash`).
    Flash {
        #[arg(long)]
        port: Option<String>,
        /// Hardware-in-the-loop build (emulator drives the chip; no Wi-Fi).
        #[arg(long)]
        bridge: bool,
        /// Flash the existing build without rebuilding.
        #[arg(long)]
        no_build: bool,
        /// Seconds to wait for the board to report its boot after flashing (0 = don't wait).
        #[arg(long, default_value_t = 15)]
        wait: u64,
    },
    /// Wi-Fi credentials (stored on the board, survive reflashing the app).
    Wifi {
        #[command(subcommand)]
        cmd: WifiCmd,
    },
    /// Board status: version, Wi-Fi, temperature, pressure, faults.
    Status {
        #[arg(long, conflicts_with = "host")]
        port: Option<String>,
        /// Ask over Wi-Fi instead of USB (IP, hostname or coffee.local).
        #[arg(long)]
        host: Option<String>,
    },
    /// Stream console events (and optionally the raw .clog log) for a while.
    Logs {
        #[arg(long)]
        port: Option<String>,
        #[arg(long, default_value_t = 10)]
        seconds: u64,
        /// Also save every byte received (the .clog stream) to this file.
        #[arg(long)]
        clog: Option<PathBuf>,
    },
    /// Update the firmware over Wi-Fi.
    Ota {
        #[arg(long, default_value = "coffee.local")]
        host: String,
        /// App image to send (default: build the firmware and make one with espflash save-image).
        #[arg(long)]
        file: Option<PathBuf>,
        /// Seconds to wait for the board to come back.
        #[arg(long, default_value_t = 90)]
        wait: u64,
    },
    /// Reboot the board (pending settings are saved first).
    Reboot {
        #[arg(long)]
        port: Option<String>,
    },
    /// Engine config (JSON) over USB.
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Send a raw console line and print the reply (see `help`).
    Send {
        #[arg(long)]
        port: Option<String>,
        line: String,
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },
}

#[derive(Subcommand, Debug)]
enum WifiCmd {
    /// Store credentials and wait until the board has an IP address.
    Set {
        #[arg(long)]
        ssid: String,
        /// Omit for an open network.
        #[arg(long, default_value = "")]
        password: String,
        #[arg(long)]
        port: Option<String>,
        /// Seconds to wait for an IP address (0 = don't wait).
        #[arg(long, default_value_t = 45)]
        wait: u64,
    },
    /// Forget the credentials.
    Clear {
        #[arg(long)]
        port: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCmd {
    Get {
        #[arg(long)]
        port: Option<String>,
    },
    /// Replace the engine config with this JSON (a file path or a literal).
    Set {
        #[arg(long)]
        port: Option<String>,
        json: String,
    },
}

/// Firmware directory: $COFFEE_ROOT, else the source tree this binary was built from.
fn firmware_dir() -> PathBuf {
    let root = std::env::var_os("COFFEE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."));
    root.join("firmware/esp32")
}

const ELF: &str = "target/riscv32imac-unknown-none-elf/release/coffee-firmware";

fn need_tool(name: &str, hint: &str) -> Result<()> {
    match Command::new(name).arg("--version").output() {
        Ok(o) if o.status.success() => Ok(()),
        _ => Err(anyhow!(NoDevice(format!("`{name}` not found; install it with: {hint}")))),
    }
}

fn run(cmd: &mut Command, what: &str) -> Result<()> {
    let st = cmd.status().with_context(|| format!("running {what}"))?;
    if !st.success() {
        bail!("{what} failed ({st})");
    }
    Ok(())
}

fn build_firmware(bridge: bool) -> Result<PathBuf> {
    let dir = firmware_dir();
    let mut c = Command::new("cargo");
    c.current_dir(&dir).args(["build", "--release"]);
    if bridge {
        c.args(["--features", "bridge"]);
    }
    // cargo's output goes to stderr, keeping stdout clean for --json.
    c.stdout(std::process::Stdio::from(std::io::stderr()));
    run(&mut c, "cargo build (firmware)")?;
    Ok(dir.join(ELF))
}

fn console(port: Option<&str>, line: &str, timeout: Duration) -> Result<Value> {
    let path = device::pick_port(port)?;
    let mut p = device::open(&path)?;
    let reply = device::request(&mut *p, line, timeout, &mut |_| {})?;
    if reply["ok"] == false {
        bail!("board: {}", reply["error"].as_str().unwrap_or("error"));
    }
    Ok(reply)
}

fn http_json(host: &str, method: &str, path: &str, body: &[u8], timeout: Duration) -> Result<Value> {
    let (status, b) = device::http(host, method, path, body, timeout)?;
    let v: Value = serde_json::from_slice(&b).unwrap_or(Value::Null);
    if status != 200 {
        bail!("HTTP {status}: {}", v["error"].as_str().unwrap_or(""));
    }
    Ok(v)
}

/// Returns (json, human text).
fn exec(cli: &Cli) -> Result<(Value, String)> {
    match &cli.cmd {
        Cmd::Ports => {
            let ps = device::ports();
            let text = if ps.is_empty() {
                "no serial ports".to_string()
            } else {
                ps.iter()
                    .map(|p| {
                        format!(
                            "{}  {}{}",
                            p.path,
                            p.product.clone().unwrap_or_default(),
                            if p.espressif { "  (Espressif)" } else { "" }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((json!({"ok": true, "ports": ps.iter().map(|p| p.to_json()).collect::<Vec<_>>()}), text))
        }
        Cmd::Flash { port, bridge, no_build, wait } => {
            need_tool("espflash", "cargo install espflash --locked")?;
            let path = device::pick_port(port.as_deref())?;
            let elf = if *no_build { firmware_dir().join(ELF) } else { build_firmware(*bridge)? };
            if !elf.exists() {
                bail!("no firmware build at {}", elf.display());
            }
            let parts = firmware_dir().join("partitions.csv");
            let mut c = Command::new("espflash");
            c.args(["flash", "--chip", "esp32c6", "--flash-size", "4mb", "--non-interactive"])
                .arg("--partition-table")
                .arg(&parts)
                .args(["--erase-parts", "otadata", "--port", &path])
                .arg(&elf)
                .stdout(std::process::Stdio::from(std::io::stderr()));
            run(&mut c, "espflash flash")?;
            let mut boot = Value::Null;
            if *wait > 0 && !bridge {
                // The app announces itself; `status` confirms the console works.
                let deadline = Instant::now() + Duration::from_secs(*wait);
                let mut last_err = None;
                while Instant::now() < deadline {
                    match console(Some(&path), "status", Duration::from_secs(4)) {
                        Ok(v) => {
                            boot = v;
                            break;
                        }
                        Err(e) => last_err = Some(e),
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
                if boot.is_null() {
                    return Err(last_err.unwrap_or_else(|| anyhow!(TimedOut("board didn't come back after flashing".into()))));
                }
            }
            let text = match boot.get("version") {
                Some(v) => format!("flashed {} on {path}; firmware {v} running", elf.display()),
                None => format!("flashed {} on {path}", elf.display()),
            };
            Ok((json!({"ok": true, "port": path, "elf": elf, "bridge": bridge, "status": boot}), text))
        }
        Cmd::Wifi { cmd: WifiCmd::Set { ssid, password, port, wait } } => {
            if let Err(e) = coffee_board::console::check_wifi(ssid, password) {
                return Err(anyhow!(BadArgs(e.into())));
            }
            let path = device::pick_port(port.as_deref())?;
            console(Some(&path), &device::wifi_set_line(ssid, password), Duration::from_secs(8))?;
            if *wait == 0 {
                return Ok((json!({"ok": true, "ssid": ssid}), format!("stored Wi-Fi '{ssid}'")));
            }
            let deadline = Instant::now() + Duration::from_secs(*wait);
            let mut last = Value::Null;
            while Instant::now() < deadline {
                if let Ok(s) = console(Some(&path), "status", Duration::from_secs(4)) {
                    if let Some(ip) = s["wifi"]["ip"].as_str() {
                        let ip = ip.to_string();
                        return Ok((
                            json!({"ok": true, "ssid": ssid, "ip": ip, "url": format!("http://{ip}/"), "rssi": s["wifi"]["rssi"]}),
                            format!("connected to '{ssid}': http://{ip}/  (also http://coffee.local/)"),
                        ));
                    }
                    last = s;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            let why = last["wifi"]["last_error"].as_str().unwrap_or("no IP yet");
            Err(anyhow!(TimedOut(format!("credentials stored, but no IP after {wait} s ({why})"))))
        }
        Cmd::Wifi { cmd: WifiCmd::Clear { port } } => {
            console(port.as_deref(), "wifi clear", Duration::from_secs(8))?;
            Ok((json!({"ok": true}), "Wi-Fi credentials cleared".into()))
        }
        Cmd::Status { port, host } => {
            let v = match host {
                Some(h) => http_json(h, "GET", "/api/device", &[], Duration::from_secs(8))?,
                None => console(port.as_deref(), "status", Duration::from_secs(5))?,
            };
            let w = &v["wifi"];
            let text = format!(
                "firmware {}  up {} s  state {}  temp {} C  pressure {} bar  faults {}\nwifi: {}",
                v["version"].as_str().unwrap_or("?"),
                v["uptime_s"],
                v["state"].as_str().unwrap_or("?"),
                v["temp_c"],
                v["pressure_bar"],
                v["faults"],
                match (w["ssid"].as_str(), w["ip"].as_str()) {
                    (Some(s), Some(ip)) => format!("'{s}' http://{ip}/ rssi {}", w["rssi"]),
                    (Some(s), None) => format!("'{s}' not connected ({})", w["last_error"].as_str().unwrap_or("connecting")),
                    _ => "not configured (coffee wifi set --ssid ... --password ...)".into(),
                }
            );
            Ok((v, text))
        }
        Cmd::Logs { port, seconds, clog } => {
            let path = device::pick_port(port.as_deref())?;
            let mut p = device::open(&path)?;
            let mut out = clog.as_ref().map(std::fs::File::create).transpose()?;
            let mut de = coffee_board::console::Deframer::default();
            let (mut events, mut bytes) = (Vec::new(), 0usize);
            let end = Instant::now() + Duration::from_secs(*seconds);
            let mut buf = [0u8; 4096];
            while Instant::now() < end {
                let n = match p.read(&mut buf) {
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => 0,
                    Err(e) => return Err(e.into()),
                };
                bytes += n;
                if let Some(f) = out.as_mut() {
                    use std::io::Write;
                    f.write_all(&buf[..n])?;
                }
                for fr in de.push(&buf[..n]) {
                    let v: Value = serde_json::from_str(&fr).unwrap_or(Value::String(fr));
                    if !cli.json {
                        println!("{v}");
                    }
                    events.push(v);
                }
            }
            let text = format!("{bytes} bytes in {seconds} s, {} events", events.len());
            Ok((json!({"ok": true, "bytes": bytes, "events": events, "clog": clog}), text))
        }
        Cmd::Ota { host, file, wait } => {
            let image = match file {
                Some(f) => f.clone(),
                None => {
                    need_tool("espflash", "cargo install espflash --locked")?;
                    let elf = build_firmware(false)?;
                    let img = firmware_dir().join("target/coffee-firmware-ota.bin");
                    let mut c = Command::new("espflash");
                    c.args(["save-image", "--chip", "esp32c6", "--flash-size", "4mb"])
                        .arg(&elf)
                        .arg(&img)
                        .stdout(std::process::Stdio::from(std::io::stderr()));
                    run(&mut c, "espflash save-image")?;
                    img
                }
            };
            let data = std::fs::read(&image).with_context(|| format!("reading {}", image.display()))?;
            let before = http_json(host, "GET", "/api/device", &[], Duration::from_secs(8)).ok();
            let r = http_json(host, "POST", "/api/ota", &data, Duration::from_secs(120))?;
            // Wait for it to reboot into the new image and answer again.
            std::thread::sleep(Duration::from_secs(3));
            let deadline = Instant::now() + Duration::from_secs(*wait);
            while Instant::now() < deadline {
                if let Ok(after) = http_json(host, "GET", "/api/device", &[], Duration::from_secs(4)) {
                    let text = format!(
                        "updated {host}: {} -> {} ({} bytes)",
                        before.as_ref().and_then(|b| b["version"].as_str()).unwrap_or("?"),
                        after["version"].as_str().unwrap_or("?"),
                        data.len()
                    );
                    return Ok((json!({"ok": true, "written": r["written"], "status": after}), text));
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            Err(anyhow!(TimedOut(format!("image sent, but {host} didn't come back within {wait} s"))))
        }
        Cmd::Reboot { port } => {
            console(port.as_deref(), "reboot", Duration::from_secs(5))?;
            Ok((json!({"ok": true}), "rebooting".into()))
        }
        Cmd::Config { cmd: ConfigCmd::Get { port } } => {
            let v = console(port.as_deref(), "config get", Duration::from_secs(5))?;
            let text = serde_json::to_string_pretty(&v["config"])?;
            Ok((v, text))
        }
        Cmd::Config { cmd: ConfigCmd::Set { port, json: j } } => {
            let text = if Path::new(j).exists() { std::fs::read_to_string(j)? } else { j.clone() };
            let v: Value = serde_json::from_str(&text).map_err(|e| anyhow!(BadArgs(format!("config must be JSON: {e}"))))?;
            console(port.as_deref(), &format!("config set {v}"), Duration::from_secs(5))?;
            Ok((json!({"ok": true}), "config updated".into()))
        }
        Cmd::Send { port, line, timeout } => {
            let path = device::pick_port(port.as_deref())?;
            let mut p = device::open(&path)?;
            let v = device::request(&mut *p, line, Duration::from_secs(*timeout), &mut |_| {})?;
            let text = v.to_string();
            if v["ok"] == false {
                bail!("board: {}", v["error"].as_str().unwrap_or("error"));
            }
            Ok((v, text))
        }
    }
}

fn exit_code(e: &anyhow::Error) -> i32 {
    if e.downcast_ref::<BadArgs>().is_some() {
        2
    } else if e.downcast_ref::<NoDevice>().is_some() {
        3
    } else if e.downcast_ref::<TimedOut>().is_some() {
        4
    } else {
        1
    }
}

fn main() {
    let cli = Cli::parse();
    match exec(&cli) {
        Ok((v, text)) => {
            if cli.json {
                println!("{v}");
            } else if !text.is_empty() {
                println!("{text}");
            }
        }
        Err(e) => {
            let code = exit_code(&e);
            if cli.json {
                println!("{}", json!({"ok": false, "error": format!("{e:#}"), "code": code}));
            } else {
                eprintln!("error: {e:#}");
            }
            std::process::exit(code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("coffee").chain(args.iter().copied()))
    }

    #[test]
    fn parses_the_agent_commands() {
        let c = parse(&["--json", "wifi", "set", "--ssid", "My Net", "--password", "secret123"]).unwrap();
        assert!(c.json);
        match c.cmd {
            Cmd::Wifi { cmd: WifiCmd::Set { ssid, password, wait, port } } => {
                assert_eq!((ssid.as_str(), password.as_str(), wait, port), ("My Net", "secret123", 45, None));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(parse(&["flash", "--bridge", "--port", "/dev/ttyACM0"]).unwrap().cmd, Cmd::Flash { bridge: true, .. }));
        assert!(matches!(parse(&["status", "--host", "coffee.local"]).unwrap().cmd, Cmd::Status { host: Some(_), .. }));
        assert!(matches!(parse(&["ota"]).unwrap().cmd, Cmd::Ota { ref host, .. } if host == "coffee.local"));
        assert!(matches!(parse(&["logs", "--seconds", "3"]).unwrap().cmd, Cmd::Logs { seconds: 3, .. }));
        assert!(matches!(parse(&["send", "help"]).unwrap().cmd, Cmd::Send { .. }));
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(parse(&["wifi", "set"]).is_err()); // --ssid required
        assert!(parse(&["status", "--port", "x", "--host", "y"]).is_err()); // one or the other
        assert!(parse(&["frobnicate"]).is_err());
    }

    #[test]
    fn exit_codes() {
        assert_eq!(exit_code(&anyhow!(NoDevice("x".into()))), 3);
        assert_eq!(exit_code(&anyhow!(TimedOut("x".into()))), 4);
        assert_eq!(exit_code(&anyhow!(BadArgs("x".into()))), 2);
        assert_eq!(exit_code(&anyhow!("other")), 1);
    }

    #[test]
    fn finds_the_firmware() {
        assert!(firmware_dir().join("Cargo.toml").exists());
        assert!(firmware_dir().join("partitions.csv").exists());
    }
}
