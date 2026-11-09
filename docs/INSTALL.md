# Install plan

The plan from a clean PC to coffee, for the **v39 controller board** (CIRCUIT.md) in its printed holder (hardware/holder), which switches the
pump and the 3-way valve itself and powers itself from mains, plus the external heater SSR (BUY.md,
WIRING.md). The machine is a US 120 V Gaggia Classic Pro.

> ⚠️ **Mains voltage, on the board too.** The board's top half (the spades, fuse, the IRM module's AC end,
> triacs and snubbers) is at 120 V whenever the machine is plugged in with the power rocker on, on both
> sides of the board. So are the heater
> SSR and the machine's own wiring. Work only with the machine unplugged and, after it's been on, cooled.
> The board lives in its printed holder, never loose and never against the steel. The stock steam
> thermostat and thermal fuse stay in series with the heater: they're the protection that doesn't depend
> on software. If you aren't comfortable with mains wiring, have steps 5–6 checked by an electrician.

## 0. What you need
- **The board:** ordered from JLCPCB (§1).
- **Everything off the board:** [BUY.md](BUY.md) has the exact parts, links and datasheets:
  - the heater SSR;
  - 18 AWG wire and 6.3 mm insulated receptacles for the four mains spades;
  - ready-made JST GH leads (3, 4, 5 and 6 pin), nothing to crimp at the board;
  - the thermocouple, pressure transducer and antenna.
- **The printed holder** (hardware/holder/README.md, BUY.md #19): `holder.stl` and `holder_peg.stl` in
  ASA (not PETG or PLA: the case runs 60–70 °C), and six 10 × 3 mm H/SH-grade magnets. The board is 80 × 77 mm
  with no holes; it slides up into the holder from below. The holder is 52 mm deep off the side panel; leave
  ~60 mm.
- **Tools:**
  - a 6.3 mm receptacle crimper (for the one new pump lead) and a small flat screwdriver;
  - a multimeter and a power meter (Kill-A-Watt);
  - a **GFCI** (plug-in RCD) for the first mains power-ups;
  - a 0.1 g scale, a blind basket, and a thermometer for calibration.

## 1. Order the board
1. Regenerate and check everything:

   ```sh
   cd hardware/controller
   .venv/bin/python controller.py         # schematic + design checks + mis-plug check -> build/
   python3 sim/run.py                     # circuit simulations (79 checks)
   source ~/opt/kienv.sh
   PLACE_JSON=plan.json FASTROUTE=… python3 ext_route.py fastroute 120   # place, route, DRC, gerbers
   python3 isolation_check.py build/coffee-controller-120v.kicad_pcb     # mains-to-SELV >= 6 mm
   python3 jlc.py && python3 jlc.py --tht hand    # full-assembly and SMD-only order files
   python3 cost.py                        # price both
   ```
2. **Files to upload** (in `build/`), one of two sets:
   - **Full assembly (recommended):** `coffee-v33-gerbers.zip`, `coffee-v33-bom.csv`, `coffee-v33-cpl.csv`.
     JLC solders everything, the five through-hole parts included (the IRM and the four spades).
   - **SMD only:** the same gerbers, plus `coffee-v33-smd-bom.csv` and `coffee-v33-smd-cpl.csv`. Then you
     solder the parts in `coffee-v33-smd-hand-solder.csv` (the IRM-10-5 and four TE 63824-1 tabs, which
     you buy from LCSC). Each mains joint must be a full, shiny fillet. Trim the leads to 1.5 mm or less
     on the underside, then clean off the flux.
   - About $184 vs $173 for 5 boards (CIRCUIT.md, "Ordering").
3. **PCB options:**
   - 2 layers, 1.6 mm FR-4, 1 oz, HASL lead-free, green mask;
   - 98 × 55.5 mm;
   - the Edge.Cuts layer has **two milled slots** (under the opto-triacs), and JLC mills them as part of
     the outline. Check that they show in JLC's Gerber viewer;
   - via covering: **tented**.
4. **Assembly:** standard PCBA (THT parts need it), top side, 2 or 5 boards.
   - In JLC's placement preview, check that:
     - the two DPAK triacs have the tab over the large pad;
     - the opto-triacs are the right way round (pin 1 dot toward the SELV side);
     - the GH sockets open outward;
     - the module is placed correctly.
   - 19 extended part types; the rest are basic.
5. **No conformal coating:** the isolation is designed as air gaps and slots, and a coating would only
   hide a bad joint.

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
Power over USB-C: D2 feeds the board's +5 V from VBUS. **The four spades stay unconnected** on the bench:
with no mains, the mains half is dead, and nothing on the board is above 5 V. (In the machine, USB and
the IRM may both be connected: the IRM then powers the board, and D2 keeps it out of the PC.)
1. **Rails:** 3.3 V on C7 (by the LDO). The board enumerates as an ESP32-C6 USB Serial/JTAG device.
2. **Flash and join Wi-Fi:**
   - Plug USB-C into the PC and run `coffee flash`, then `coffee wifi set --ssid … --password …`. The
     status LED glows while the chip is in programming mode and flickers at every reset.
   - The second command prints the board's address (`http://coffee.local/` works too).
   - Later updates go over Wi-Fi (`coffee ota`). The board only accepts one with the power rocker **off**
     and the steam rocker **on**, so nobody on the network can replace the firmware without someone at
     the machine.
   - If a bad firmware ever stops USB working: short the **BOOT** pad (TP1) to the USB-C shell (GND)
     while plugging in, then flash again.
3. **Sensors:** clamp the thermocouple into J5 (the screw terminal), the transducer (J6) and the front-panel
   lead (J7):
   - **Temperature** reads room temperature, within about a degree of the board's cold-junction
     reading in the UI, and rises when you hold the probe's tip.
   - **Polarity:** if the temperature *falls* when you warm the tip, TC+ and TC− are swapped. The firmware
     raises `probe_reversed` once it's ~25 °C off. Swap the two wires in the terminal (WIRING check 8
     identifies them: alumel, TC−, is the magnetic wire).
   - **Unplug the probe:** the UI shows `probe_open` within about a second, and the heater output goes
     off (step 4). Do this check here, on USB.
   - **Transducer (no maker datasheet):** wire it per its label or listing to J6: 1 signal, 2 +5 V,
     3 GND. For this first test, put a **220 Ω resistor in series with the +5 V wire**: it limits the
     current to ~20 mA if the legend is wrong. It should read ~0.5 V at atmospheric pressure, and the UI
     about 0 bar. Then remove the resistor.
   - **Inputs:** brew and steam toggle when J7 pin 1 or 2 is shorted to pin 3 (GND).
   - **Lamps:** the panel LEDs on J7.4–7.6 and the board's own D3–D5 follow the UI's front-panel card.
   - **Scale:** with the scale on J10, the cup weight changes when you press on the tray.
   - **Calibrate the thermocouple now:** CALIBRATION.md, "Temperature" (room temperature, then the tip
     in boiling water).
4. **Outputs:**
   - **Heater:** on J2, +5 V is pin 1 and the heater output is pin 2. Put a 1–10 kΩ resistor (or the SSR
     itself) from pin 1 to pin 2, then measure across pins 1–2: ~5 V when the UI turns the heater on,
     ~0 V when it's off and while the board resets or flashes.
   - **Pump and valve:** on USB, the opto LEDs are driven but the triacs have no mains, so only the board
     LEDs H, P and V show them. Each should light exactly when the UI commands it.

## 4. Fit sensors and plumbing (machine unplugged)
Follow GaggiMate's Classic Pro guide (<https://docs.gaggimate.eu/docs/installation-gcp>) for the
mechanical steps; it has photos.
1. Unplug, drain, cool. **Photograph all the wiring** before touching it.
2. **Thermocouple:** remove the brew thermostat (the 107 °C one on the left of the boiler). GaggiMate:
   to reach it, loosen the steam valve and unscrew the group head so the boiler can tilt; put both
   back afterwards, torqued as they were.
   - The M4 probe screws straight into the thermostat's seat ("screw in the M4 thermocouple into the
     boiler": docs.gaggimate.eu/docs/installation-gcp). Meter-check it first (WIRING.md check 8: it must
     be ungrounded).
   - A thin film of thermal paste on the thread, then **hand-tight plus a nip**: the boss is soft, and
     over-tightening can strip it.
   - Its two bare wires clamp into the board's J5 screw terminal: TC+ in pin 1, TC− in pin 2. **Never splice it:** any joint to copper away from the board is an unmeasured
     junction.
   - Keep the thermostat's two spade wires: they become the heater feed in §5.
3. **Pressure:** GaggiMate's plumbing kit puts a G1/8 elbow and push-fit T between the pump and the
   OPV, and the transducer sits remote on 4/6 mm PTFE tube with a G1/4 push fitting screwed onto it
   (hand-tight, with its gasket), lying flat at the bottom of the machine. If you take the pump's
   push-in thermal fuse out to do this, **clip it back** (WIRING check 4a confirms it). You'll check
   for leaks cold *and* hot in §6.
4. Route the probe and transducer leads away from the boiler and the mains wires (the probe's own
   leads are PTFE in fibreglass, rated 250 °C; keep the splices ≥ 50 mm off the boiler).

## 5. Wire the machine and the board
Follow [WIRING.md](WIRING.md): the wiring diagram and a terminal-by-terminal table:
- the board's four mains spades (N, L, P, 3);
- the heater SSR (in place of the brew thermostat);
- the board's JST GH leads.

It also lists the multimeter checks to do before mains. Plug the low-voltage leads and the thermocouple onto the board in
your hand, slide it up into its holder and peg it, then push the four mains receptacles on through the wire
slot (hardware/holder/README.md, "Fit it").

## 6. First power-on
Leave every lead plugged in, the heater SSR's included. The steam thermostat and thermal fuse are
still in series with the heater. Don't unplug J2 to "test the pump first": the firmware sees full
heat with no temperature rise and, within about 2 minutes, latches a `HEATER_INEFFECTIVE` fault
(clear it in the UI, or reboot).

**Never plug a USB cable into the board while the machine is plugged in with the case open.** The
isolation makes USB safe in itself, but your hands would be beside the live mains half. After the
first flash, updates go over Wi-Fi; if you need USB again, unplug the machine first.

**Close the case before any mains power-on**, every time: top cover on with its two earth wires
reconnected (WIRING check 9), side panels on. The board's mains half and the heater SSR's screw
terminals are live and not touch-protected. To look at something inside, unplug first. **Plug the
machine in through a GFCI** for the first power-ups.

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
   - The board boots from its IRM module.
   - The heater SSR's LED comes on and the meter reads ~1.37 kW while it heats.
   - If anything smells, smokes or reads wrong, switch off at the wall.
5. In the UI (`http://coffee.local/`, or the IP from `coffee status --host …`; no USB from now on):
   - temperature rises steadily from room temperature;
   - pressure reads about 0 bar.
6. **Pump and valve:** with the blind basket, run a flush and check that pressure rises. Then switch
   off, unplug, and check the paper towel again for hot leaks.
   - Listen while the pump is off: no hum (the snubber leaks about 1 mA, too little to move it).
7. **Heater:** watch the first heat-up to setpoint in the UI.
   - Expect ~1.37 kW until near the setpoint, then pulsing.
   - With the heater commanded off, the meter should drop to a few watts.
   - The heater SSR's base should stay warm, not hot: below 60 °C after a 30-minute idle is fine.
8. **Temperatures inside the case:** after a 30-minute idle and a 2-minute steam session, unplug
   and measure straight away (a thermometer probe or IR thermometer): the heater SSR's base
   < 80 °C, the air at the board < 70 °C, and the triacs' tabs (Q5, Q6) < 100 °C. Over any of those:
   move that part further from the boiler.
9. Pull a shot.

## 7. Calibrate
1. **Thermocouple, two points:** done on the bench in §3, before the probe went on the boiler
   (CALIBRATION.md). The firmware stored both points. This also cancels the input filter's gain error
   and the open-probe bias offset.
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
