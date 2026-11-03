# Install plan

The plan from a clean PC to coffee, for the **v6 controller board** (CIRCUIT.md), which switches
the machine directly. The machine is a US 120 V Gaggia Classic Pro. A module build
([OFF_THE_SHELF.md](OFF_THE_SHELF.md)) follows the same steps, with modules in place of the board.

> ⚠️ **Mains voltage.** The board carries mains on its left half. Work only with the machine
> unplugged and, after it's been on, cooled. Keep the stock steam thermostat and thermal fuse
> in series with the heater: they're the protection that doesn't depend on software. If you
> aren't comfortable with mains wiring, have the install (steps 4–6) checked by an electrician.

## 0. What you need

| | |
|---|---|
| Board | v6 controller (order: §1), in a 3D-printed enclosure: **flame-retardant** filament (PC, or a V-0 rated PETG/ASA), not PLA |
| Thermistor | **100 kΩ NTC, B = 3950, ±1 %**, sensor **insulated from the probe body**, rated to ≥ 150 °C, PTFE or silicone leads, ~300 mm. Two mounts work: an **M4 × 0.7 threaded probe** that screws straight into the brew-thermostat hole, or a **ring-lug (eye) sensor with a 4 mm hole**, clamped to the boiler under an M4 stainless screw in that hole, with thermal paste. Fit a JST-PH-2 plug. (No specific listing has been verified yet.) |
| Pressure transducer | 0–1.6 MPa (0–16 bar), 5 V supply, 0.5–4.5 V output, G1/4. Plus a GaggiMate-style T plumbing kit |
| Antenna | 2.4 GHz antenna with a U.FL (IPEX MHF1) pigtail, ~100–150 mm |
| Wire | 1.0 mm² (18 AWG) silicone for heater, pump and valve; 0.5 mm² for L/N supply; 24 AWG for signals; fibreglass sleeve near the boiler |
| Crimps | 6.3 mm Faston receptacles, fully insulated (the stock harness uses 6.3 mm tabs); bootlace ferrules for the screw terminals; JST-PH housings and crimps |
| Hardware | M3 nylon standoffs (×3); M3 screw + nylon shoulder washer + thermal pad for the heater triac tab |
| Tools | crimper, multimeter, power meter (Kill-A-Watt), 0.1 g scale, blind basket, thermometer for calibration |

## 1. Order the board
1. Regenerate everything:

   ```sh
   cd hardware/controller
   .venv/bin/python controller.py && source ~/opt/kienv.sh && python3 layout.py && python3 jlc.py
   ```
2. Open `build/coffee-controller-120v.kicad_pcb` in KiCad 7+. Read the DRC report (`*-drc.rpt`),
   **and look at the mains side yourself before ordering**. It was placed by code and autorouted,
   and nobody has reviewed it yet. The audit pack (`python3 audit.py`) is for a second opinion.
3. Fab settings:
   - 2 layers, 1.6 mm FR4, **2 oz copper** (the heater path carries 11.4 A);
   - HASL lead-free;
   - **route the two slots** (Edge.Cuts includes them).
4. **Assembly.** JLCPCB places the SMD parts from `build/jlc-120v/bom.csv` + `cpl.csv`: the C6
   module, USB-C, LDO, valve triac and passives. Check rotations in JLC's preview.
5. **You hand-solder** the through-hole parts listed in `build/jlc-120v/hand.csv`:
   - terminals, triacs Q1/Q2, optos U1–U3;
   - PSU, fuse, MOV, JST connectors.
6. **Conformal coat the mains half** (left of the band) after testing.

## 2. Software and emulation (your PC)

```sh
rustup target add riscv32imac-unknown-none-elf wasm32-unknown-unknown
cargo install espflash trunk
cargo build --release && (cd ui && trunk build --release)   # also builds the `coffee` CLI
./target/release/coffee-emu test                  # all scenarios pass
./target/release/coffee-emu serve --spawn-engine  # UI at http://localhost:8080
```

Firmware: the `coffee` CLI builds, flashes and provisions it (build the UI first: the firmware
embeds `ui/dist`). The details are in [`firmware/esp32/README.md`](../firmware/esp32/README.md).
On WSL, attach the board's USB with `usbipd` to get `/dev/ttyACM0`.

## 3. Bench bring-up: USB only, no mains
Power over USB-C. D2 feeds the board from VBUS, and nothing on the mains side is live.
1. **Rails:** 3.3 V on C7. The board enumerates as an ESP32-C6 USB Serial/JTAG device.
2. **Flash and join Wi-Fi:** plug USB-C into the PC and run `coffee flash`, then
   `coffee wifi set --ssid … --password …`. The second prints the board's address
   (`http://coffee.local/` works too). No buttons; a blank module takes the firmware directly.
   This is the only time the board needs a PC: later updates go over Wi-Fi (`coffee ota`).
   - If a bad firmware ever stops USB working: short the **BOOT** pad (TP1, underside) to **GND**
     (TP2) with tweezers while plugging in, then flash again.
3. **Sensors.** Plug in the thermistor, transducer and a switch pigtail:
   - temperature reads room temperature, and rises when you hold the probe;
   - pressure reads about 0 bar;
   - brew and steam inputs toggle when shorted to GND on J7.
4. **Outputs:** put a multimeter across each opto's LED resistor (R7 heater, R8 pump, R9 valve).
   It should read ~1.5 V when the UI turns that load on, and 0 V while the board resets or
   flashes.

## 4. Fit sensors and plumbing (machine unplugged)
1. Unplug, drain, cool. **Photograph all the wiring** before touching it.
2. **Thermistor:** remove the brew thermostat (the 107 °C one on the boiler).
   - Fit the probe into its M4 hole **hand-tight plus a nip**: the boss is soft, and over-tightening
     can shear the stud.
   - Use a dab of thermal paste.
   - Keep the thermostat's two spade wires: they become the heater feed in §5.
3. **Pressure:** fit the plumbing kit's T at the pump outlet, with the transducer on the G1/4 port.
   You'll check for leaks cold *and* hot in §6.
4. Route the probe and transducer leads away from the boiler and the mains wires, with fibreglass
   sleeve near anything hot.

## 5. Wire the board

| Board | Goes to | Notes |
|---|---|---|
| **J2 Heater** (1, 2) | the two wires that went to the **brew thermostat** | The triac replaces the thermostat. The steam thermostat and thermal fuse stay in series, untouched |
| **J1 Mains in** (1 = L, 2 = N) | L and N after the machine's power switch | Supplies the board, pump and valve only (~0.5 A) |
| **J3.1 Valve** | the 3-way valve's live wire, previously fed from the brew switch | Pump and valve neutrals stay where they are |
| **J3.2 Pump** | the pump's live wire, previously fed from the brew switch | |
| **J7 Switches** (1 brew, 2 steam, 3 GND) | the brew and steam switches, **disconnected from mains** and rewired as dry contacts | The switch lamps go dark. The steam switch must no longer bypass anything: the board owns the heater |
| **J5 Thermistor** (1, 2) | the NTC probe | No polarity |
| **J6 Pressure** (1 = +5 V, 2 = signal, 3 = GND) | the transducer | |
| **U4 antenna** (U.FL) | the antenna pigtail, out through a plastic part or vent | The case is steel: an antenna left inside it won't reach the Wi-Fi |

Make up short pigtails: a ferrule on one end into the screw terminal, a fully insulated 6.3 mm
Faston receptacle on the other onto the stock harness. Nothing in the machine gets cut.

**Mounting**
- The board sits in its printed enclosure on three nylon standoffs on the side panel, away from
  the boiler.
- **Bolt the heater triac Q1's tab to the chassis** with a thermal pad and a nylon shoulder washer.
  - The BTA24's tab is insulated; the washer keeps the screw from biting into it.
  - It dissipates ~11 W, and the chassis is the heatsink.
  - The bolt is also the board's fourth fixing.
- Clip the printed cover over the mains half.

**Checks before mains (multimeter, unplugged)**
- J1 L–N: open (the PSU input reads several kΩ or more).
- J2 1–2: open (the triac is off).
- J3 to L: open.
- No continuity from any mains terminal to the chassis, to J5/J6/J7 or to USB GND.
- Earth: chassis to the plug's earth pin < 0.5 Ω.

## 6. First power-on
1. Disconnect J2 (heater). Plug the machine into the power meter and power on.
2. Open the UI over Wi-Fi at `http://coffee.local/` (or the IP from `coffee status --host …`);
   no USB from now on. Check that temperature is plausible and pressure reads about 0 bar.
3. **Pump and valve:** run a flush with no portafilter, and check:
   - water flows;
   - pressure rises with the blind basket;
   - no leaks at the T.
4. **Heater:** power off and reconnect J2. Power on and watch the first heat-up live in the UI,
   hand on the plug.
   - Expect ~1.37 kW on the meter until near the setpoint, then pulsing.
   - The Q1 tab should stay warm, not hot: below 60 °C after a 30-minute idle is fine.
5. Pull a shot.

## 7. Calibrate
1. **Thermistor, two points.** Read the probe at room temperature against a thermometer. Then
   with the boiler open to boiling water (100 °C at sea level; ~95 °C at 1500 m). The firmware
   stores both points. This also cancels the probe's ±1 % tolerance and its tiny self-heating.
2. **Pressure:** read zero with the system fully vented (open the steam wand with the pump off,
   so the boiler is at atmospheric pressure). Then compare against a portafilter gauge at
   9 bar.
3. **Machine model:** record the logs in [CALIBRATION.md](CALIBRATION.md):
   - cold heat-up;
   - blind basket;
   - free flow;
   - shots at two grinds.

   Fit the model with `coffee-emu fit`, re-run `coffee-emu test` against the fitted model, and
   copy the fitted values into the engine config. From then on the emulator *is* your machine:
   try profiles there first.
