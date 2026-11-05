# Install plan

The plan from a clean PC to coffee, for the **v18 low-voltage controller board** (CIRCUIT.md) and
the certified modules that switch the machine (BUY.md, WIRING.md). The machine is a US 120 V
Gaggia Classic Pro.

> ⚠️ **Mains voltage.** The board itself carries only 5 V. The mains is in the machine's wiring and
> the modules: the heater and pump SSRs, the valve relay and the 5 V supply. Work only with the
> machine unplugged and, after it's been on, cooled. Keep the stock steam thermostat and thermal
> fuse in series with the heater: they're the protection that doesn't depend on software. If you
> aren't comfortable with mains wiring, have steps 4–6 checked by an electrician.

## 0. What you need
- **The board:** ordered assembled from JLCPCB (§1).
- **Everything off the board:** [BUY.md](BUY.md) has the exact parts, verified links and datasheets:
  - modules: heater SSR + heatsink, pump SSR, valve relay, 5 V supply, fuse;
  - wire and crimps;
  - JST GH leads;
  - thermistor, pressure transducer and antenna.
- **A printed holder** for the modules and the board. Use flame-retardant PC (V-0), not PLA or PETG. The heater SSR goes on its aluminium heatsink, never on plastic.
- **Tools:** crimper, multimeter, power meter (Kill-A-Watt), 0.1 g scale, blind basket, thermometer for calibration.

## 1. Order the board
1. Regenerate everything:

   ```sh
   cd hardware/controller
   .venv/bin/python controller.py                      # schematic + checks -> build/parts-120v.json
   python3 sim/run.py                                  # circuit simulations (61 checks)
   source ~/opt/kienv.sh
   PLACE_JSON=plan.json FASTROUTE=… python3 ext_route.py fastroute 120   # place, route (fastroute), DRC, gerbers
   python3 jlc.py                                      # JLC BOM + position file, checked against JLC's footprints
   ```
2. **Files to upload** (current ones are in `build/`):
   - `coffee-v18-gerbers.zip`;
   - `coffee-v18-bom.csv`;
   - `coffee-v18-cpl.csv`.
3. **PCB options (all JLC defaults, the cheapest tier):**
   - 2 layers, 1.6 mm FR-4, **1 oz** copper;
   - HASL lead-free, green solder mask;
   - 43 × 27 mm, no castellations. The drill file has 4 plated slots, for the USB-C shell legs: standard, no option to change;
   - **edge rails:** if JLC adds rails for assembly (it usually does for a board this small), put them on the **left and right short edges only**, and add the remark "J4 USB-C overhangs the bottom long edge by 0.46 mm; no rail or V-cut on the bottom edge";
   - **via covering: Tented** (the default). Stitching and signal vias sit under the module's body between its pads; the Gerbers have no mask openings on vias.
4. **Assembly:** Economic PCBA, **top side only**, 2 boards (the other 3 come bare).
   - Everything is SMD, so there's nothing to hand-solder.
   - In JLC's placement preview, glance over the seven GH connectors, the USB-C, the module and the diodes (all rotations were solved against JLC's footprints; D2's from a same-package record, cross-checked against JLC's drawings).
   - JLC lists 4 extended parts (GH connector, USB-C, ESP32-C6 module, LDO); everything else is basic.
5. No conformal coating needed: there's no mains on the board.

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
Power over USB-C: D2 feeds the board from VBUS. USB and the 5 V module may both be connected: the module then powers the board and D2 keeps it out of the PC. (With USB alone, ~4.6 V also appears on J1; harmless.)
1. **Rails:** 3.3 V on C7. The board enumerates as an ESP32-C6 USB Serial/JTAG device.
2. **Flash and join Wi-Fi:**
   - Plug USB-C into the PC and run `coffee flash`, then `coffee wifi set --ssid … --password …`.
   - The second command prints the board's address (`http://coffee.local/` works too).
   - No buttons: the C6 resets itself into download mode over USB.
   - This is the only time the board needs a PC. Later updates go over Wi-Fi (`coffee ota`).
   - If a bad firmware ever stops USB working: short the **BOOT** pad (TP1, on the underside) to the USB-C shell (GND) with tweezers while plugging in, then flash again.
3. **Sensors:** plug in the thermistor, transducer and a switch lead:
   - temperature reads room temperature, and rises when you hold the probe's tab;
   - **transducer (no datasheet, BUY.md §10b):** before connecting it, find its +5 V, GND and output
     wires with a meter, wire them to J6 pins 1, 3 and 2, and check the output reads ~0.5 V at
     atmospheric pressure. The UI then reads about 0 bar;
   - brew and steam inputs toggle when J7 pin 1 or 2 is shorted to pin 3;
   - **unplug the probe:** the UI shows a thermistor fault within a second, and the heater output (J2, measured as in step 4) goes off. Do this check here, on USB: in the machine it would mean reaching past live terminals.
4. **Outputs:** put a load across pin 1 and pin 2 of J2, J3 and J8: a 1–10 kΩ resistor, or the module itself. With nothing plugged in, an off output's pin 2 floats, and a meter alone can read anything from 0 to 5 V. Then measure pin 1 to pin 2:
   - ~5 V when the UI turns that load on (the MOSFET pulls pin 2 to GND);
   - ~0 V when off, and while the board resets or flashes: the gate pull-downs hold every output off;
   - with a module plugged in, its own LED shows the same.

## 4. Fit sensors and plumbing (machine unplugged)
1. Unplug, drain, cool. **Photograph all the wiring** before touching it.
2. **Thermistor:** remove the brew thermostat (the 107 °C one on the boiler).
   - The probe (Molex 218537-3919) is a stainless ring tab with a Ø4.2 mm hole (Molex drawing), so it
     fits under an M4 screw. GaggiMate's GCP install guide gives the brew-thermostat seat as M4
     ("screw in the M4 thermocouple into the boiler": docs.gaggimate.eu/docs/installation-gcp).
     Meter-check the probe first (WIRING.md check 8).
   - Clean the seat, add a thin film of thermal paste, then put the ring tab flat on the seat under
     one screw, **hand-tight plus a nip**: the boss is soft, and over-tightening can strip it.
   - The probe's body is isolated from its leads (1500 VAC, Molex drawing), so the earthed boiler
     doesn't touch the NTC circuit. Its 300 mm PTFE leads go to J5's GH lead with a soldered,
     heat-shrunk splice away from the boiler (WIRING.md).
   - Keep the thermostat's two spade wires: they become the heater feed in §5.
3. **Pressure:** fit the plumbing kit's T at the pump outlet, with the transducer on the G1/4 port.
   You'll check for leaks cold *and* hot in §6.
4. Route the probe and transducer leads away from the boiler and the mains wires, with fibreglass
   sleeve near anything hot.

## 5. Wire the machine and the board
Follow [WIRING.md](WIRING.md): the wiring diagram and a terminal-by-terminal table for the 5 V
supply, the heater SSR (in place of the brew thermostat), the pump SSR, the valve relay and the
board's JST GH leads, plus the multimeter checks to do before mains.

## 6. First power-on
Leave every lead plugged in, the heater SSR's included. The steam thermostat and thermal fuse are
still in series with the heater. Don't unplug J2 to "test the pump first": the firmware sees full
heat with no temperature rise and, within about 2 minutes, latches a `HEATER_INEFFECTIVE` fault
(clear it in the UI, or reboot).

The firmware heats from the moment it boots, and §4 drained the boiler and opened the plumbing.
So **fill the tank before you switch on, and prime straight away**: the first minute is a cold
prime and leak check.
1. Fill the water tank. No portafilter, a jug under the group.
2. Plug the machine into the power meter, hand on the plug, and power on. **Straight away, switch
   the brew switch on** (the firmware starts a brew on it even while heating) and leave it on until
   water comes out of the group steadily, then switch it off:
   - the pump runs and the relay LED is on;
   - no leaks at the T: look now, while it's still cold. If it leaks, switch off at the wall.
   - A drained pump pulls air for the first seconds, so the engine may stop with `NO_WATER`
     (no pressure within 6 s). That's expected here: switch the brew switch off, clear the fault
     with the **Clear** button next to the fault on the brew page, and switch it on again until water flows.
3. Then let it heat.
   - The board boots from the 5 V module.
   - The heater SSR's LED comes on and the meter reads ~1.37 kW while it heats.
   - If anything smells, smokes or reads wrong, switch off at the wall.
4. In the UI (`http://coffee.local/`, or the IP from `coffee status --host …`; no USB from now on):
   - temperature rises steadily from room temperature;
   - pressure reads about 0 bar.
5. **Pump and valve:** with the blind basket, run a flush and check:
   - pressure rises;
   - no leaks at the T, now hot.
   - Listen while the pump is off: no hum (the pump SSR leaks only 0.1 mA).
6. **Heater:** watch the first heat-up to setpoint in the UI.
   - Expect ~1.37 kW until near the setpoint, then pulsing.
   - With the heater commanded off, the meter should drop to a few watts.
   - The heater SSR's heatsink should stay warm, not hot: below 60 °C after a 30-minute idle is fine.
7. Pull a shot.

## 7. Calibrate
1. **Thermistor, two points:** CALIBRATION.md, "NTC". Do the boiling-water point with the
   **machine unplugged and the board on USB**: unscrew the ring tab and hold it in a pot of boiling
   water. The firmware stores both points. This also cancels the probe's ±1 % tolerance and its tiny self-heating.
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
