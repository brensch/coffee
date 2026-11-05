# Install plan

The plan from a clean PC to coffee, for the **v24 low-voltage controller board** (CIRCUIT.md) and
the certified modules that switch the machine (BUY.md, WIRING.md). The machine is a US 120 V
Gaggia Classic Pro.

> ⚠️ **Mains voltage.** The board itself carries only 5 V. The mains is in the machine's wiring and
> the modules: the heater SSR, the pump and valve SSR boards and the 5 V supply. Work only with the
> machine unplugged and, after it's been on, cooled. Keep the stock steam thermostat and thermal
> fuse in series with the heater: they're the protection that doesn't depend on software. If you
> aren't comfortable with mains wiring, have steps 4–6 checked by an electrician.

## 0. What you need
- **The board:** ordered assembled from JLCPCB (§1).
- **Everything off the board:** [BUY.md](BUY.md) has the exact parts, verified links and datasheets:
  - modules: heater SSR, two 1-channel SSR boards (pump, valve) + snubber, 5 V supply;
  - wire and crimps;
  - JST GH leads;
  - thermistor, pressure transducer and antenna.
- **A printed holder** (or plastic bracket) for the board, the two SSR boards and the 5 V supply: their undersides carry mains, so never mount them on the steel. Use flame-retardant PC (V-0), not PLA or PETG. The heater SSR bolts to the rear panel, never on plastic.
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
   - `coffee-v24-gerbers.zip`;
   - `coffee-v24-bom.csv`;
   - `coffee-v24-cpl.csv`.
   - Every part is on the top side; JLC places all of it, and there is nothing to solder.
   - (Older versions' files stay in `build/` for comparison: `coffee-v21-*`, `coffee-v22-*`, `coffee-v23-*`. v24 fixes the independent review's should-fix items: see docs/REVIEW-v23.md.)
3. **PCB options (all JLC defaults, the cheapest tier):**
   - 2 layers, 1.6 mm FR-4, **1 oz** copper;
   - HASL lead-free, green solder mask;
   - 46.6 × 26.4 mm, no castellations. The drill file has 4 plated slots, for the USB-C shell legs: standard, no option to change;
   - **edge rails:** if JLC adds rails for assembly (it usually does for a board this small), put them on the **right short edge** only (the connectors sit flush with both long edges), and add the remark "J4 USB-C overhangs the left short edge by 0.96 mm; no rail or V-cut on the left edge";
   - **via covering: Tented** (the default). Stitching and signal vias sit under the module's body between its pads; the Gerbers have no mask openings on vias.
4. **Assembly:** Economic PCBA, **top side only**, 2 boards (the other 3 come bare).
   - The BOOT pad (TP1) on the underside is bare copper: nothing to place there.
   - In JLC's placement preview, glance over the connectors (openings outward), the module and D2 (all rotations were solved against JLC's footprints; D2's and the two 0402 capacitors' from same-package records).
   - JLC lists 7 extended parts (the 2-, 3-, 6- and 7-pin GH connectors, the USB-C, the ESP32-C6 module, the LDO); everything else, including every 0402, is basic.
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
Power over USB-C: D2 feeds the board from VBUS. On the bench, leave the 5 V module unplugged from J1: on USB alone, Q4 passes ~4.6 V back out of J1. (In the machine, USB and the module may both be connected: the module then powers the board and D2 keeps it out of the PC.)
1. **Rails:** 3.3 V on C7 (by the LDO). The board enumerates as an ESP32-C6 USB Serial/JTAG device.
2. **Flash and join Wi-Fi:**
   - Plug USB-C into the PC and run `coffee flash`, then `coffee wifi set --ssid … --password …`. The status LED glows while the chip is in programming mode and flickers at every reset.
   - The second command prints the board's address (`http://coffee.local/` works too).
   - No buttons: the C6 resets itself into download mode over USB.
   - This is the only time the board needs a PC. Later updates go over Wi-Fi (`coffee ota`).
   - If a bad firmware ever stops USB working: short the **BOOT** pad (TP1, on the underside) to the USB-C shell (GND) with tweezers while plugging in, then flash again.
3. **Sensors:** plug in the thermistor, transducer and a switch lead:
   - temperature reads room temperature, and rises when you hold the probe's tab;
   - **transducer (no datasheet, BUY.md #11):** use the colour legend printed on its label or the
     listing to wire +5 V, GND and output to J6 pins 1, 3 and 2, with a **220 Ω resistor in series
     with the +5 V wire** for this first test (it limits the current to ~20 mA if the legend is
     wrong). The output should read ~0.5 V at atmospheric pressure, and the UI about 0 bar. If it
     doesn't, unplug and swap only the two wires you're unsure of. Then remove the resistor;
   - power, brew and steam inputs toggle when J7 pin 1, 2 or 3 is shorted to pin 4. **With J7.1 open the engine sits in Standby** (heater, pump and valve off): short J7.1–J7.4 (or wire the power rocker) to test the outputs;
   - the three panel LEDs on J7.5–7.7 follow the UI's front-panel card;
   - with the scale on J10, the UI's cup weight changes when you press on the tray;
   - **calibrate the thermistor now,** before it goes on the boiler: both points of CALIBRATION.md "NTC" (room temperature, then the ring tab in a pot of boiling water). On USB the heater can't run, and nothing is live;
   - **unplug the probe:** the UI shows a thermistor fault within a second, and the heater output (J2, measured as in step 4) goes off. Do this check here, on USB: in the machine it would mean reaching past live terminals.
4. **Outputs:** put a load across pin 1 and pin 2 of J2, J3 and J8: a 1–10 kΩ resistor, or the module itself. With nothing plugged in, an off output's pin 2 floats, and a meter alone can read anything from 0 to 5 V. Then measure pin 1 to pin 2:
   - ~5 V when the UI turns that load on (the MOSFET pulls pin 2 to GND);
   - ~0 V when off, and while the board resets or flashes: the gate pull-downs hold every output off;
   - with a module plugged in, its own LED shows the same.

## 4. Fit sensors and plumbing (machine unplugged)
Follow GaggiMate's Classic Pro guide (<https://docs.gaggimate.eu/docs/installation-gcp>) for the
mechanical steps; it has photos.
1. Unplug, drain, cool. **Photograph all the wiring** before touching it.
2. **Thermistor:** remove the brew thermostat (the 107 °C one on the left of the boiler). GaggiMate:
   to reach it, loosen the steam valve and unscrew the group head so the boiler can tilt; put both
   back afterwards, torqued as they were.
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
3. **Pressure:** GaggiMate's plumbing kit puts a G1/8 elbow and push-fit T between the pump and the
   OPV, and the transducer sits remote on 4/6 mm PTFE tube with a G1/4 push fitting screwed onto it
   (hand-tight, with its gasket), lying flat at the bottom of the machine. If you take the pump's
   push-in thermal fuse out to do this, **clip it back** (WIRING check 4a confirms it). You'll check
   for leaks cold *and* hot in §6.
4. Route the probe and transducer leads away from the boiler and the mains wires (the probe's own
   leads are PTFE in fibreglass, rated 250 °C; keep the splices ≥ 50 mm off the boiler).

## 5. Wire the machine and the board
Follow [WIRING.md](WIRING.md): the wiring diagram and a terminal-by-terminal table for the 5 V
supply, the heater SSR (in place of the brew thermostat), the pump and valve SSR boards and the
board's JST GH leads, plus the multimeter checks to do before mains.

## 6. First power-on
Leave every lead plugged in, the heater SSR's included. The steam thermostat and thermal fuse are
still in series with the heater. Don't unplug J2 to "test the pump first": the firmware sees full
heat with no temperature rise and, within about 2 minutes, latches a `HEATER_INEFFECTIVE` fault
(clear it in the UI, or reboot).

**Close the case before any mains power-on**, every time: top cover on with its two earth wires
reconnected (WIRING check 9), side panels on. The SSR boards' and the heater SSR's screw terminals
are live and not touch-protected. To look at something inside, unplug first.

The firmware heats from the moment it boots, and §4 drained the boiler and opened the plumbing.
So **fill the tank before you switch on, and prime straight away**: the first minute is a cold
prime.
1. Fill the water tank. No portafilter, a jug under the group. Put a dry paper towel under the
   pump/T area inside the case before closing it.
2. Plug the machine into the power meter, hand on the plug, and power on. **Straight away, switch
   the brew switch on** (the firmware starts a brew on it even while heating) and leave it on until
   water comes out of the group steadily, then switch it off:
   - the pump runs and water flows;
   - A drained pump pulls air for the first seconds, so the engine may stop with `NO_WATER`
     (no pressure within 6 s). That's expected here: switch the brew switch off, clear the fault
     with the **Clear** button next to the fault on the brew page, and switch it on again until water flows.
3. **Cold leak check:** switch off, **unplug**, open the case and look at the paper towel and the T.
   Dry: close the case and carry on. Wet: fix the fitting first.
4. Plug in, power on, and let it heat.
   - The board boots from the 5 V module.
   - The heater SSR's LED comes on and the meter reads ~1.37 kW while it heats.
   - If anything smells, smokes or reads wrong, switch off at the wall.
5. In the UI (`http://coffee.local/`, or the IP from `coffee status --host …`; no USB from now on):
   - temperature rises steadily from room temperature;
   - pressure reads about 0 bar.
6. **Pump and valve:** with the blind basket, run a flush and check that pressure rises. Then switch
   off, unplug, and check the paper towel again for hot leaks.
   - Listen while the pump is off: no hum (the pump SSR board leaks at most ~1 mA).
7. **Heater:** watch the first heat-up to setpoint in the UI.
   - Expect ~1.37 kW until near the setpoint, then pulsing.
   - With the heater commanded off, the meter should drop to a few watts.
   - The heater SSR's base should stay warm, not hot: below 60 °C after a 30-minute idle is fine.
8. **Temperatures inside the case:** after a 30-minute idle and a 2-minute steam session, unplug
   and measure straight away (a thermometer probe or IR thermometer): the heater SSR's base
   < 80 °C (above that, add the Auber HS25ET heatsink, BUY.md #1), the air at the SSR boards and
   the 5 V supply < 60 °C, the M10/M11 joints < 105 °C. Over any of those: move that
   part further from the boiler.
9. Pull a shot.

## 7. Calibrate
1. **Thermistor, two points:** done on the bench in §3, before the probe went on the boiler
   (CALIBRATION.md, "NTC"). The firmware stored both points. This also cancels the probe's ±1 % tolerance and its tiny self-heating.
2. **Pressure:** the zero is taken automatically at every cold power-on. For the span, compare
   against a portafilter gauge at 9 bar (case closed, machine running).
3. **Machine model:** record the logs in [CALIBRATION.md](CALIBRATION.md):
   - cold heat-up;
   - blind basket;
   - free flow;
   - shots at two grinds.

   Fit the model with `coffee-emu fit`, re-run `coffee-emu test` against the fitted model, and
   copy the fitted values into the engine config. From then on the emulator *is* your machine:
   try profiles there first.
