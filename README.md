# coffee

An espresso machine controller built emulation-first, in Rust. It's in the spirit of GaggiMate/Gaggiuino, with these differences:

- **One product, two shells.** Everything the machine does (engine, profiles, shot history, the whole HTTP API, logging) lives in the `no_std` crate `coffee-node`. The PC binary and the ESP32 firmware are thin wrappers that differ only in plumbing.
- **A physics emulator stands in for the machine.** It models the boiler and water thermals, the vibratory pump, the over-pressure valve, steam saturation, the puck, the cup and scale, and sensor quirks. It can inject faults and runs coupled in lockstep with the engine.
- **A scenario language scripts tests.** Timed and event-triggered actions, fault injection and expectations make a CI suite: 19 scenarios, about 2.5 hours of machine time in about 80 s.
- **Real-machine logs calibrate the model.** PC and firmware write the same compact binary log of the hardware boundary (`.clog`, about 2.7 bytes per tick, about 1 MB per machine-hour). `coffee-emu fit` replays it and adjusts model parameters until the simulated sensors match.
- **The web UI is all Rust (Leptos).**
  - **Machine** page: everything live from telemetry (same on the ESP32), including an animated machine diagram, shot chart, controls and temperature.
  - **Experiments**: emulator ground truth beside what the engine believes, fault injection, scenarios, model parameters.
  - Profiles, History (grind advice against a reference shot) and Settings.
  - Brewing is refused until the machine is Ready (water at temperature, group heat-soaked) unless you choose *Brew anyway*. The physical switch always works.

Target machine: **Gaggia Classic Pro E24, US 120 V** (models for 120 V, 230 V, and the 2019 aluminium boiler are included). See [docs/HARDWARE.md](docs/HARDWARE.md).

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
coffee-emu import-decent path/to/*.tcl      # convert Decent Espresso profiles
python3 tools/profile_check.py profiles/community   # brew every profile in the emulator
```

Firmware: ESP32-C6 (stable Rust), flashed and provisioned with the `coffee` CLI (non-interactive, `--json`; see [firmware/esp32/README.md](firmware/esp32/README.md)):

```sh
coffee flash                                  # build + flash over USB (needs espflash)
coffee wifi set --ssid HomeNet --password ... # store Wi-Fi, wait for an IP
open http://coffee.local/                     # the same UI and API, served by the board
coffee ota --host coffee.local                # later updates over Wi-Fi
coffee-emu capture --port /dev/ttyACM0 --out real.clog   # record its log
coffee flash --bridge && coffee-emu serve --engine-serial /dev/ttyACM0   # hardware-in-the-loop
```

## Hardware: the v6 controller board

One 72 × 56.5 mm board with 51 parts. It sits inside the machine and switches the heater, pump and
valve itself. The design rule is that anything the firmware can do, the firmware does. What's left on
the board:
- isolation;
- three zero-cross triacs (800 V parts);
- an ESP32-C6-MINI-1U (105 °C rated, U.FL antenna);
- a certified 5 V module;
- a thermistor input and a pressure input, read by the C6's own ADC.

| Item | What | Notes |
|---|---|---|
| Board | v6 controller, JLCPCB assembles the SMD side | `hardware/controller` generates everything: schematic, PCB, DRC, fab and JLC files. You hand-solder the through-hole parts. See [docs/CIRCUIT.md](docs/CIRCUIT.md). |
| Boiler temperature | 100k B3950 NTC thermistor, M4 thread or ring lug | Goes in the brew-thermostat hole, hand-tight. Two-point calibration. |
| Pressure | Transducer 0–1.6 MPa, 5 V, 0.5–4.5 V, G1/4 + [GaggiMate plumbing kit](https://shop.gaggimate.eu/products/gaggimate-plumbing-kit) | |
| Antenna | 2.4 GHz, U.FL pigtail | Outside the steel case. |
| Scale *(optional)* | Bluetooth scale (Decent / Bookoo / Acaia) | Weight stop. Firmware driver not written yet. |

The full list and the reasons behind it are in [docs/HARDWARE.md](docs/HARDWARE.md); fitting it is
[docs/INSTALL.md](docs/INSTALL.md). Off-the-shelf alternatives are compared in
[docs/OFF_THE_SHELF.md](docs/OFF_THE_SHELF.md).

**Keep in the machine:** the steam thermostat (or a 165–170 °C limiter) and the thermal fuse, in
series with the heater.

## Layout

| Path | What |
|---|---|
| `crates/coffee-io` | Hardware boundary: per-mains-cycle `SensorFrame` / `ActuatorFrame` (NTC and pressure in ADC millivolts), lockstep wire protocol. `no_std`. |
| `crates/coffee-board` | Pure firmware logic, host-tested: crystal cycle clock, ADC oversampling, switch debounce, status LED, grid-drift stroke model. `no_std`. |
| `crates/coffee-core` | The engine: sensor conversion, filters, PID + feed-forward, pump-model flow linearisation, profiles, state machine, safety. `no_std + alloc`. |
| `crates/coffee-api` | HTTP/WS API types, shot recorder, grind advice, GaggiMate importer. Shared by engine, firmware and UI. |
| `crates/coffee-node` | **The product**: engine plus shot recorder, profile library (built-ins compiled in), config, the HTTP API as a pure function, telemetry, binary log (`tlog`) and shot (`shotpack`) formats. `no_std + alloc`. |
| `crates/coffee-engine` | PC shell: lockstep link (stdio / TCP), file storage (`--data`), axum passing `/api/*` to the node, UI hosting. |
| `crates/coffee-physics` | Coupled machine model with fault injection. Parameters live in `models/*.toml`. |
| `crates/coffee-scenario` | Scenario DSL: parser and runner. |
| `crates/coffee-emu` | Emulator suite: `run`, `test`, `serve` (live lab), `replay`, `fit`, `import-gaggimate`. |
| `ui/` | Leptos web UI. |
| `hardware/controller` | Controller board as code: SKiDL schematic with isolation and pin-map checks, KiCad layout with the mains lint, Freerouting, ngspice simulations, 3D renders, JLCPCB files, audit pack. |
| `tools/` | `iterate.sh`: rebuild, run diagnostic shots, print control-quality metrics. |
| `crates/coffee-cli` | `coffee`: flash, Wi-Fi provisioning, status, logs and OTA for the board. Non-interactive, `--json`, exit codes. |
| `firmware/esp32` | ESP32-C6 firmware: machine mode (control loop, Wi-Fi, embedded UI + API, flash storage, OTA, USB console) and USB bridge mode. |
| `models/`, `profiles/`, `scenarios/` | Machine models, shot profiles (built-ins + `profiles/community`: 47 Decent and 4 GaggiMate profiles, with their own licenses), test scenarios. |

Docs: [PROFILES](docs/PROFILES.md) · [ARCHITECTURE](docs/ARCHITECTURE.md) · [HARDWARE & BOM](docs/HARDWARE.md) · [CIRCUIT](docs/CIRCUIT.md) · [OFF-THE-SHELF](docs/OFF_THE_SHELF.md) · [INSTALL](docs/INSTALL.md) · [CALIBRATION](docs/CALIBRATION.md) · [MODELS](docs/MODELS.md) · [SCENARIOS](docs/SCENARIOS.md)

## Status

- **Working and tested in emulation:**
  - engine, physics, DSL, test suite
  - calibration loop (including a test that recovers known parameters from synthetic data)
  - live lab and UI
  - GaggiMate import
- **Compiles, not yet run on hardware:** ESP32-C6 firmware: machine mode (Wi-Fi, web UI and API, WebSocket telemetry, flash storage, OTA, USB provisioning) and bridge mode; the `coffee` CLI.
- **Designed, not yet reviewed or built:** the v6 controller board (routed, DRC-checked, simulated; audit pack in `hardware/controller/build/`).
- **Not built yet:**
  - a Bluetooth scale driver
  - the motorised grinder
- **Every model parameter is an estimate until fitted to logs from your machine.** See [docs/MODELS.md](docs/MODELS.md).

⚠️ This controls a mains-powered heater and a 15 bar pump. Keep the machine's hardware thermal protection in series. See the safety section in [docs/HARDWARE.md](docs/HARDWARE.md).
