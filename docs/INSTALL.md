# Install plan

The plan from a clean PC to coffee, for the **v2 controller board** (CIRCUIT.md), which switches
the machine directly. A module build ([OFF_THE_SHELF.md](OFF_THE_SHELF.md)) follows the same steps,
with modules in place of the board.

> ⚠️ **Mains voltage.** The board carries mains on its left half. Work only with the machine
> unplugged and, after it's been on, cooled. Keep the stock steam thermostat and thermal fuse
> in series with the heater: they are the protection that doesn't depend on software. If you
> aren't comfortable with mains wiring, have the install (steps 4–6) checked by an electrician.

## 0. What you need

| | |
|---|---|
| Board | v2 controller (order: §1), plus a 3D-printed PETG/ASA cover for the mains side |
| Sensors | K thermocouple: **ungrounded**, M4 thread. Pressure transducer: 0–1.6 MPa, 5 V, 0.5–4.5 V, G1/4. GaggiMate plumbing kit |
| Wire | 1.0 mm² (18 AWG) silicone for heater, pump and valve; 0.5 mm² for L/N supply; 24 AWG for signals; fibreglass sleeve near the boiler |
| Crimps | 6.3 mm Faston receptacles, fully insulated (the stock harness uses 6.3 mm tabs); bootlace ferrules for the screw terminals |
| Hardware | M3 nylon standoffs (×4); M3 screw + nylon shoulder washer for the heater triac tab; thermal pad |
| Tools | crimper, multimeter, power meter (Kill-A-Watt), 0.1 g scale, blind basket |

## 1. Order the board
1. `cd hardware/controller && .venv/bin/python controller.py && python3 layout.py` (230 V), or
   `MAINS_V=120 …` and `layout.py --mains 120` (120 V). Both runs regenerate everything in `build/`.
2. Open `build/coffee-controller-230v.kicad_pcb` in KiCad 7+. Review the DRC report (`*-drc.rpt`),
   **and look at the mains side yourself before ordering**. It was placed by code and autorouted,
   and nobody has reviewed it.
3. Fab settings:
   - 2 layers, 1.6 mm FR4, **2 oz copper** (the heater path carries up to 11.4 A at 120 V);
   - green mask both sides, HASL lead-free;
   - **route the two slots** (Edge.Cuts includes them).
4. Assembly: SMD parts by the fab (JLC/PCBWay) from `bom-<V>v.csv`. Hand-solder the THT parts:
   triacs, optos, PSU, fuse, MOV, X2 caps, terminals.
5. **Conformal coat the mains half** (left of the band) after testing. It raises the functional
   insulation between mains nets (1 mm spacing) to the pollution-degree-1 class.

## 2. Software and emulation (your PC)

```sh
rustup target add riscv32imac-unknown-none-elf wasm32-unknown-unknown
cargo install espflash trunk
cargo build --release && (cd ui && trunk build --release)
./target/release/coffee-emu test                  # all scenarios pass
./target/release/coffee-emu serve --spawn-engine  # UI at http://localhost:8080
```

S3 firmware: `cd firmware/esp32 && source ../../out/export-esp.sh && cargo build-s3`.
On WSL, attach the board's USB with `usbipd` to get `/dev/ttyACM0`.

## 3. Bench bring-up: USB only, no mains
Power over USB-C. D2 feeds the board from VBUS; nothing on the mains side is live.
1. **Rails:** 3.3 V at C3. The board enumerates as an ESP32-S3 USB serial/JTAG device.
2. **Flash** (`cargo run-s3`). This is the only time the board is plugged into a PC.
3. **HIL:** flash the bridge build (`cargo build-s3-bridge`, then run
   `coffee-emu serve --engine-serial /dev/ttyACM0`). The real chip now runs the real engine
   against the emulated machine. Pull a few shots and run the Experiments scenarios.
4. **Machine firmware.** Check each sensor and the switches:
   - thermocouple reads room temperature, and ~35 °C when pinched;
   - transducer reads 0 bar;
   - brew and steam inputs toggle when shorted to GND on J8.
5. **Driver LEDs D3–D5** light with heater, pump and valve demand. Hold RESET and they must all
   go dark: the gate pull-downs keep the loads off during boot and flashing.

## 4. Fit sensors and plumbing (machine unplugged)
1. Unplug, drain, cool. **Photograph all the wiring** before touching it.
2. **Thermocouple:** remove the brew thermostat (the 107 °C one on the boiler). Screw the M4 probe
   into its hole **hand-tight plus a nip**, because the boss is soft. Keep its two spade wires:
   they become the heater feed in §5.
3. **Pressure:** fit the plumbing kit's T at the pump outlet, with the transducer on the G1/4 port.
   Run the pump with the case open (step 7 below) and check for leaks cold *and* hot.
4. Route the probe and transducer leads away from the boiler and mains wires, with fibreglass
   sleeve near anything hot.

## 5. Wire the board

| Board | Goes to | Notes |
|---|---|---|
| **J2 Heater** (1, 2) | the two wires that went to the **brew thermostat** | The triac replaces the thermostat. The steam thermostat and thermal fuse stay in series, untouched |
| **J1 Mains in** (1 = L, 2 = N) | L and N after the machine's power switch | Supplies the board, pump and valve only (~0.5 A) |
| **J3.1 Pump** | the pump's live wire, previously fed from the brew switch | |
| **J3.2 Valve** | the 3-way valve's live wire, previously fed from the brew switch | Pump and valve neutrals stay where they are |
| **J8 Switches** (1 brew, 2 steam, 3 GND) | the brew and steam switches, **disconnected from mains** and rewired as dry contacts | The switch lamps go dark. The steam switch must no longer bypass anything: the board owns the heater |
| J5 Thermocouple | probe: yellow = 1 (T+), red = 2 (T−) | ANSI colours |
| J6 Pressure | transducer: 1 = +5 V, 2 = signal, 3 = GND | |
| J7 Group NTC (opt) | 100k NTC under a group screw | |
| J9 I²C (opt) | display / scale bridge | |

Make up short pigtails: ferrule on one end into the screw terminal, a fully insulated 6.3 mm Faston
receptacle on the other onto the stock harness. Nothing in the machine gets cut.

**Mounting**
- The board sits on four nylon standoffs on the side panel, away from the boiler.
- **Bolt the heater triac Q1's tab to the chassis** with thermal pad and a nylon shoulder washer.
  The BTA16 tab is insulated, but the washer keeps the screw from biting into the tab.
  Dissipation is about 7 W at 230 V and 12 W at 120 V, and the chassis is the heatsink.
- Clip the printed cover over the mains half.

**Checks before mains (multimeter, unplugged)**
- J1 L–N: open (the PSU input is several kΩ or more).
- J2 1–2: open (the triac is off).
- J3 to L: open.
- No continuity from any mains terminal to the chassis, to J8 or to USB GND.
- Earth: chassis to the plug's earth pin < 0.5 Ω.

## 6. First power-on
1. Disconnect J2 (heater). Plug the machine into the power meter. Power on.
2. The UI is reachable over Wi-Fi (no USB from now on). Check:
   - mains frequency reads 50/60 Hz from the zero-cross;
   - temperature is plausible;
   - pressure reads 0 bar.
3. **Pump and valve:** run a flush with no portafilter. Water flows, and pressure rises with the
   blind basket. Check for leaks at the T.
4. **Heater:** power off and reconnect J2. Power on and watch the first heat-up live in the UI,
   hand on the plug.
   - Expect ~1.37 kW on the meter (230 V) until near the setpoint, then pulsing.
   - The Q1 tab should stay comfortably warm, not hot. Below 60 °C after a 30-minute idle is fine.
5. Pull a shot.

## 7. Calibrate
Record the logs in [CALIBRATION.md](CALIBRATION.md): cold heat-up, blind basket, free flow, shots
at two grinds. Fit the model with `coffee-emu fit`, re-run `coffee-emu test` against the fitted
model, and copy the fitted values into the engine config. From then on the emulator *is* your
machine: try profiles there first.
