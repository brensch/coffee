# coffee

An espresso machine controller built emulation-first, in Rust. It's in the spirit of GaggiMate/Gaggiuino, with these differences:

- **One product, two shells.** Everything the machine does (engine, profiles, shot history, the whole HTTP API, logging) lives in the `no_std` crate `coffee-node`. The PC binary and the ESP32 firmware are thin wrappers that differ only in plumbing.
- **A physics emulator stands in for the machine.** It models the boiler and water thermals, the vibratory pump, the over-pressure valve, steam saturation, the puck, the cup and scale, and sensor quirks. It can inject faults and runs coupled in lockstep with the engine.
- **A scenario language scripts tests.** Timed and event-triggered actions, fault injection and expectations make a CI suite: 15 scenarios, about 2.5 hours of machine time in about 80 s.
- **Real-machine logs calibrate the model.** PC and firmware write the same compact binary log of the hardware boundary (`.clog`, about 2.7 bytes per tick, about 1 MB per machine-hour). `coffee-emu fit` replays it and adjusts model parameters until the simulated sensors match.
- **The web UI is all Rust (Leptos).**
  - **Machine** page: everything live from telemetry (same on the ESP32), including an animated machine diagram, shot chart, controls and temperature.
  - **Experiments**: emulator ground truth beside what the engine believes, fault injection, scenarios, model parameters.
  - Profiles, History (grind advice against a reference shot) and Settings.
  - Brewing is refused until the machine is Ready (water at temperature, group heat-soaked) unless you choose *Brew anyway*. The physical switch always works.

Target machine: **Gaggia Classic Pro E24** (models for 230 V, 120 V, and the 2019 aluminium boiler are included). See [docs/HARDWARE.md](docs/HARDWARE.md).

## Quick start

```sh
cargo build --release                       # engine, emulator, tools
(cd ui && trunk build --release)            # web UI (needs `trunk`, wasm32 target)

./target/release/coffee-emu test            # run the scenario suite
./target/release/coffee-emu serve --spawn-engine
#   -> UI at http://127.0.0.1:8080  (Experiments tab drives the emulator)
```

Other things to try:

```sh
coffee-emu run scenarios/02_shot_classic.scn --trace out/shot.csv --events
coffee-emu signals                          # everything you can read / set / fit
coffee-emu fit --log real.clog --params thermal.boiler_mass_kg,thermal.loss_boiler_w_k --out models/mine.toml
coffee-emu import-gaggimate path/to/*.json  # convert GaggiMate profiles
```

Firmware (ESP32-C6):

```sh
cd firmware/esp32c6
cargo run --release                         # real machine (needs espflash)
coffee-emu capture --port /dev/ttyACM0 --out real.clog   # record its log
cargo run --release --features bridge       # hardware-in-the-loop
coffee-emu serve --engine-serial /dev/ttyACM0   # emulator drives the real chip
```

## Layout

| Path | What |
|---|---|
| `crates/coffee-io` | Hardware boundary: `SensorFrame` / `ActuatorFrame` (raw MAX31855 words, ADS1115 counts), lockstep wire protocol. `no_std`. |
| `crates/coffee-core` | The engine: sensor conversion, filters, PID + feed-forward, pump-model flow linearisation, profiles, state machine, safety. `no_std + alloc`. |
| `crates/coffee-api` | HTTP/WS API types, shot recorder, grind advice, GaggiMate importer. Shared by engine, firmware and UI. |
| `crates/coffee-node` | **The product**: engine plus shot recorder, profile library (built-ins compiled in), config, the HTTP API as a pure function, telemetry, binary log (`tlog`) and shot (`shotpack`) formats. `no_std + alloc`. |
| `crates/coffee-engine` | PC shell: lockstep link (stdio / TCP), file storage (`--data`), axum passing `/api/*` to the node, UI hosting. |
| `crates/coffee-physics` | Coupled machine model with fault injection. Parameters live in `models/*.toml`. |
| `crates/coffee-scenario` | Scenario DSL: parser and runner. |
| `crates/coffee-emu` | Emulator suite: `run`, `test`, `serve` (live lab), `replay`, `fit`, `import-gaggimate`. |
| `ui/` | Leptos web UI. |
| `firmware/esp32c6` | ESP32-C6 firmware (machine mode and USB bridge mode). |
| `models/`, `profiles/`, `scenarios/` | Machine models, shot profiles, test scenarios. |

Docs: [ARCHITECTURE](docs/ARCHITECTURE.md) · [HARDWARE & BOM](docs/HARDWARE.md) · [INSTALL](docs/INSTALL.md) · [CALIBRATION](docs/CALIBRATION.md) · [MODELS](docs/MODELS.md) · [SCENARIOS](docs/SCENARIOS.md)

## Status

- **Working and tested in emulation:**
  - engine, physics, DSL, test suite
  - calibration loop (including a test that recovers known parameters from synthetic data)
  - live lab and UI
  - GaggiMate import
- **Compiles, not yet run on hardware:** ESP32-C6 firmware (machine and bridge modes).
- **Not built yet:**
  - serving the UI from the ESP32 over Wi-Fi (planned: embassy + esp-radio + picoserve, same API)
  - a Bluetooth scale driver
  - the motorised grinder
- **Every model parameter is an estimate until fitted to logs from your machine.** See [docs/MODELS.md](docs/MODELS.md).

⚠️ This controls a mains-powered heater and a 15 bar pump. Keep the machine's hardware thermal protection in series. See the safety section in [docs/HARDWARE.md](docs/HARDWARE.md).
