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
coffee-emu import-decent path/to/*.tcl      # convert Decent Espresso profiles
python3 tools/profile_check.py profiles/community   # brew every profile in the emulator
```

Firmware: ESP32-C6 (default, stable Rust) or ESP32-S3 (Espressif's Xtensa toolchain):

```sh
cd firmware/esp32
# ESP32-S3 instead of C6: `cargo install espup && espup install`, source its export file, then
#   cargo +esp build-s3        (or build-s3-bridge, run-s3)
cargo run --release                         # real machine (needs espflash)
coffee-emu capture --port /dev/ttyACM0 --out real.clog   # record its log
cargo run --release --features bridge       # hardware-in-the-loop
coffee-emu serve --engine-serial /dev/ttyACM0   # emulator drives the real chip
```

## Bill of materials (v1: off-the-shelf modules)

The fast route to real hardware: modules wired together, no custom PCB. It's close to GaggiMate's
["Lego" build](https://docs.gaggimate.eu/docs/sourcing-lego/), with a few deliberate swaps.
Prices are rough USD. The audit behind each choice is in [docs/HARDWARE.md](docs/HARDWARE.md);
the custom board for later is in [docs/CIRCUIT.md](docs/CIRCUIT.md).

| Item | What to buy | Qty | ~$ | Notes |
|---|---|---|---|---|
| Controller | **ESP32-C6-DevKitC-1-N8** or **ESP32-S3-DevKitC-1-N8R2** | 1 | 10 | Firmware builds for both. S3 = more memory headroom. |
| Boiler temperature | **MAX31855** thermocouple board + **K-type thermocouple, M4 thread**, 0.5–1 m | 1 | 25 | Goes in the brew-thermostat hole, hand-tight. |
| ADC | **ADS1115** breakout | 1 | 6 | Reads the pressure sensor (plus a 5 V reference). |
| Pressure sensor | **Transducer 0–1.6 MPa, 5 V supply, 0.5–4.5 V out, G1/4 male** | 1 | 20 | |
| Plumbing | **[GaggiMate plumbing kit](https://shop.gaggimate.eu/products/gaggimate-plumbing-kit)** | 1 | 26 | Or generic: 4/6 mm PTFE tube, PL 06-01, PLF 06-02, G1/8 F-M-F tee, 1/4" silicone gasket, PTFE tape. |
| Heater relay | **Genuine zero-cross SSR, 25 A** (Crydom D2425 / Omron G3NA-225B) | 1 | 35 | Bolt to the chassis. Avoid generic "40-DA" listings: counterfeit-prone, switching 1.4 kW. |
| Pump driver + zero-cross | **RobotDyn AC dimmer module 4 A** | 1 | 5 | Drives the pump and provides the mains zero-cross signal. (Or a small zero-cross SSR plus a separate zero-cross detector.) |
| Valve relay | **Omron G3MB-202P** 5 V SSR module | 1 | 4 | Add an RC snubber (100 Ω + 100 nF X2) across the output. |
| 5 V supply | **Hi-Link HLK-5M05** + 0.5 A slow fuse + 10D561K MOV | 1 | 8 | Isolated, certified. |
| Wiring | 18 AWG silicone (mains), 22 AWG silicone (signal), 6.35 mm spade connectors, ferrules, fibreglass sleeve, heat-shrink | – | 20 | |
| Scale *(optional)* | Bluetooth scale (Decent / Bookoo / Acaia) | 1 | 60+ | Weight stop. Firmware driver not written yet. |
| **Total** | | | **≈ $160** | without the scale |

**Keep in the machine:** the steam thermostat (or a 165–170 °C limiter) and the thermal fuse, in series
with the heater. **Tools:** wire stripper, ratcheting crimper, multimeter, Phillips screwdriver,
4/5 mm Allen keys, 14 mm spanner. **Calibration extras:** plug-in power meter, probe thermometer,
0.1 g scale, blind basket ([docs/CALIBRATION.md](docs/CALIBRATION.md)).

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
| `hardware/controller` | Controller board as code (SKiDL): ERC, isolation and pin-map checks, KiCad netlist, BOM. |
| `tools/` | `iterate.sh`: rebuild, run diagnostic shots, print control-quality metrics. |
| `firmware/esp32` | ESP32 firmware, C6 or S3 (machine mode and USB bridge mode). |
| `models/`, `profiles/`, `scenarios/` | Machine models, shot profiles (built-ins + `profiles/community`: 47 Decent and 4 GaggiMate profiles, with their own licenses), test scenarios. |

Docs: [PROFILES](docs/PROFILES.md) · [ARCHITECTURE](docs/ARCHITECTURE.md) · [HARDWARE & BOM](docs/HARDWARE.md) · [CIRCUIT](docs/CIRCUIT.md) · [INSTALL](docs/INSTALL.md) · [CALIBRATION](docs/CALIBRATION.md) · [MODELS](docs/MODELS.md) · [SCENARIOS](docs/SCENARIOS.md)

## Status

- **Working and tested in emulation:**
  - engine, physics, DSL, test suite
  - calibration loop (including a test that recovers known parameters from synthetic data)
  - live lab and UI
  - GaggiMate import
- **Compiles, not yet run on hardware:** ESP32 firmware for the C6 and S3 (machine and bridge modes), and the controller board design (schematic as code, ERC-clean, KiCad netlist and BOM generated; PCB layout not done).
- **Not built yet:**
  - serving the UI from the ESP32 over Wi-Fi (planned: embassy + esp-radio + picoserve, same API)
  - a Bluetooth scale driver
  - the motorised grinder
- **Every model parameter is an estimate until fitted to logs from your machine.** See [docs/MODELS.md](docs/MODELS.md).

⚠️ This controls a mains-powered heater and a 15 bar pump. Keep the machine's hardware thermal protection in series. See the safety section in [docs/HARDWARE.md](docs/HARDWARE.md).
