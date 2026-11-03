# coffee firmware (ESP32-C6)

The firmware for the v6 controller board (ESP32-C6-MINI-1U-H4, 4 MB flash). It runs the same
`coffee_node::Node` as the PC engine, and adds:
- the crystal-timed control loop;
- Wi-Fi (station, DHCP, `coffee.local`);
- the web UI and HTTP API;
- WebSocket telemetry;
- flash storage;
- OTA updates;
- a USB console for provisioning.

See the comments in `src/main.rs` for how the parts fit together and why the control loop can't
be starved by networking.

Everything below is driven by the `coffee` CLI (`crates/coffee-cli`), which is written to be used
by an AI agent or a script.

## The `coffee` CLI

```sh
cargo build --release -p coffee-cli     # -> target/release/coffee
cargo install espflash --locked         # flashing and OTA images need espflash
```

Rules an agent can rely on:
- **Non-interactive:** nothing prompts, and every wait has a timeout (`--wait`, `--seconds`,
  `--timeout`).
- **`--json`** prints exactly one JSON object on stdout: `{"ok": true, ...}` or
  `{"ok": false, "error": "...", "code": N}`. Build and flash progress goes to stderr.
- **Exit codes:**

  | Code | Meaning |
  |---|---|
  | 0 | done |
  | 1 | failed: the board said no, or a build/flash failed |
  | 2 | bad arguments: clap usage, or values the board would reject |
  | 3 | no board, port, host or tool found |
  | 4 | timed out waiting for the board |
- **Ports:** `--port` is optional. With exactly one Espressif USB device (VID `303a`) attached,
  it's picked automatically. `coffee ports` lists them.

| Command | What it does |
|---|---|
| `coffee ports` | serial ports, Espressif boards first |
| `coffee flash [--port P] [--bridge] [--no-build] [--wait 15]` | `cargo build --release`, then `espflash flash` with `partitions.csv` (erasing `otadata`, so it boots the fresh image), then waits until the board answers `status` |
| `coffee wifi set --ssid S [--password P] [--wait 45]` | stores credentials on the board and waits for an IP. JSON: `{"ok":true,"ip":"…","url":"http://…/"}`. No password = open network |
| `coffee wifi clear` | forgets them |
| `coffee status [--port P \| --host H]` | version, uptime, Wi-Fi (`ssid`, `ip`, `rssi`, `last_error`), state, temperature, pressure, faults. `--host` asks over Wi-Fi (`GET /api/device`) |
| `coffee logs [--seconds 10] [--clog out.clog]` | prints console events (boot, wifi, ip, storage errors) and optionally saves the raw byte stream: the binary `.clog` log that `coffee-emu fit` reads |
| `coffee ota [--host coffee.local] [--file img.bin] [--wait 90]` | builds an app image (`espflash save-image`), POSTs it to `/api/ota`, waits for the board to come back, and reports the version before and after |
| `coffee reboot` | flushes pending settings, then resets |
| `coffee config get` / `coffee config set <json-or-file>` | the engine config over USB |
| `coffee send "<console line>"` | any console command (see below); prints the reply |

`--host` accepts an IP, a hostname or `coffee.local`. If the system resolver can't do mDNS, the
CLI sends its own query.

## Blank board to brewing over Wi-Fi

```sh
coffee --json ports                                   # board visible? (WSL: usbipd attach first)
coffee --json flash                                   # build + flash + wait for it to answer
coffee --json wifi set --ssid "HomeNet" --password "…" # -> {"ok":true,"ip":"192.168.1.50",...}
coffee --json status --host coffee.local              # same status, now over Wi-Fi
# UI: http://coffee.local/  (or the ip above). API: http://coffee.local/api/status
```

Later updates need no cable: `coffee --json ota --host coffee.local`.

Wi-Fi credentials, config, profiles and shots live in the `storage` partition. A USB flash or an
OTA update rewrites only the app, so they survive. Only `espflash erase-flash` wipes them.

## The USB console (for tools other than the CLI)

The USB Serial/JTAG port carries the binary `.clog` log from the board to the host. Commands go
the other way as text lines ending in `\n`. Every reply and event comes back as a framed JSON
object: `STX "COFFEE " json ETX "\n"` (bytes `0x02` … `0x03`). Skip everything outside the frames.
Frames with an `"ok"` key are replies; frames with an `"event"` key are events.

| Line | Reply |
|---|---|
| `status` | `{"ok":true,"version":…,"uptime_s":…,"heap_free":…,"storage_pending":…,"wifi":{…},"state":…,"temp_c":…,"pressure_bar":…,"faults":[…]}` |
| `wifi set "<ssid>" "<password>"` | `{"ok":true,"ssid":…}`, then poll `status` until `wifi.ip` appears |
| `wifi clear` | `{"ok":true}` |
| `config get` / `config set <json>` | `{"ok":true,"config":{…}}` / `{"ok":true}` |
| `reboot` | `{"ok":true}`, then the board resets |
| `help` | the command list |

Quote arguments containing spaces with `"…"`; inside quotes, `\"` and `\\` escape. Errors come back
as `{"ok":false,"error":"…"}`. Opening the port can reset the chip; the CLI re-sends a command
once if it gets no answer within half its timeout.

## HTTP

| Request | |
|---|---|
| `GET /` and any non-API path | the web UI (gzip, embedded at build time from `ui/dist`; run `trunk build --release` in `ui/` before building the firmware) |
| `GET /api/ws` | WebSocket: current telemetry, then every node event as JSON, the same as the PC engine |
| `GET /api/device` | firmware status (as the console's `status`) |
| `POST /api/ota` | body = app image from `espflash save-image --chip esp32c6`; refused (409) during a shot |
| other `/api/*` | `Node::handle_http`, the same API as the PC engine |

## How the network reaches the controller

The control task owns the node and never waits on the network. `/api/*` requests and the console's `config` commands are queued, and the control task handles up to 2 per mains cycle, after the outputs, within a 1 ms budget. A full queue answers **503** at once; no answer within 1.5 s gives **504**. Both are safe to retry. `status` and `/api/device` read a telemetry snapshot and never queue. Details: `docs/ARCHITECTURE.md` ("Firmware ownership").

## Build directly

```sh
cargo build --release                    # machine firmware (Wi-Fi, UI, storage, OTA)
cargo build --release --features bridge  # hardware-in-the-loop (emulator drives the chip; no Wi-Fi)
cargo run --release                      # build + flash + monitor (the runner uses partitions.csv)
```

Image: ~1.24 MB (UI ~0.33 MB of that), against a 1.63 MB OTA slot (`partitions.csv`).
