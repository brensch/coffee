# Hardware

Target: **Gaggia Classic Pro, US 120 V / 60 Hz** (E24, brass boiler). For why this machine, see the README.

Where to find what:
- the board (schematic as code, calculations, simulations): [CIRCUIT.md](CIRCUIT.md);
- what to buy, with verified links and datasheets: [BUY.md](BUY.md);
- how it's wired: [WIRING.md](WIRING.md);
- fitting it: [INSTALL.md](INSTALL.md);
- measuring the machine for the models: [CALIBRATION.md](CALIBRATION.md).

## Design rules

1. **No mains on our board.** The custom PCB carries only 5 V. Every mains function is a certified, replaceable, factory-tested module:
   - a zero-cross SSR for the heater, on the rear panel;
   - a 1-channel G3MB-202P SSR board for the pump;
   - a 1-channel SSR board for the 3-way valve (as GaggiMate's Lego build), with a snubber;
   - an EN 60335 / UL 62368 5 V AC-DC module.

   The board sinks each module's 5 V input with a MOSFET. A code check fails the build if any mains net appears on the board.
2. **Hardware protection stays in series with the heater.** The stock steam thermostat and the thermal fuse stay wired as they are. Software can only *ask* a failed-short SSR to stop (scenario 12); SSRs, like triacs, usually fail shorted.
3. **Whole mains cycles only.** The engine never does phase-angle control. Zero-cross SSRs switch the heater, the pump and the valve. That's why no zero-cross detector, dimmer or phase timing is needed: the firmware ticks once per mains cycle from its crystal. The pump's internal diode means each enabled cycle is exactly one stroke.
4. **Outputs are off unless the firmware says otherwise.** Gate pull-downs hold every output off while the module resets, boots or is flashed. This is simulated against a 20 kΩ internal pull-up (sim/REPORT.md).
5. **The firmware does what it can; the board keeps only physics.** The board does switching and sensor-to-voltage. Debounce, calibration, linearisation, fault detection, oversampling and timing are Rust.
6. **Measure what the model needs.** Each sensor maps to model parameters. Anything we can't sense is estimated, and the UI says so.

## What you buy
- **The board:** from JLCPCB, fully assembled (every part on the top side, nothing to solder). The files are `build/coffee-v30-{gerbers.zip,bom.csv,cpl.csv}`; the order settings are in INSTALL.md §1.
- **Everything else:** [BUY.md](BUY.md):
  - modules (heatsink optional);
  - wire and crimps;
  - JST GH leads;
  - thermistor, pressure transducer and antenna.

## How it's wired (summary; details in WIRING.md)
| Load | Path |
|---|---|
| Heater | live → stock steam thermostat → thermal fuse → **heater SSR (in place of the brew thermostat)** → element → neutral |
| Pump | switched live → **pump SSR** → pump → neutral |
| 3-way valve | switched live → **valve SSR board** → valve → neutral |
| Board | switched live → **5 V module** (no external fuse, as GaggiMate) → board J1 |

One 7-pin JST GH lead from J2 drives all three module inputs: pin 2 is +5 V, daisy-chained heater SSR 3 (+) → pump board VCC → valve board VCC; pins 3, 6, 7 are the heater, pump and valve returns (OUT, pulled to GND = on). Pins 1, 4, 5 are empty. See WIRING.md.

## Enclosure
- **Mounting (as GaggiMate):** the heater SSR bolts to the inside of the rear panel at the ventilation slots (M4 bolts or cable ties through the slots). The board, the two SSR boards and the 5 V supply sit on a plastic part (a printed holder or bracket), never directly on the steel: their undersides carry mains. No DIN rail.
- **Heater SSR:** on the steel rear panel (its heatsink), never on plastic. It dissipates about 12 W at full power.
- **Board placement (as GaggiMate):** upright in the **back-right corner**, against the right side panel, connector edge **up**: every cable leaves the top and runs up into the loom under the cup warmer, away from the boiler. Keep it on a plastic holder or nylon standoffs, never against bare steel with the copper side.
- **Board:** 64.3 × 20 mm, no mounting holes. **Mount it with heat-resistant velcro** (hook-and-loop with a high-temperature adhesive; check the adhesive's rating is well above the 60–70 °C inside the case, since ordinary velcro adhesive softens there) on the back, against the right side panel or onto a plastic plate. Coordinates: top view, origin top-left, x right, y down; the connectors are along y = 0.
  - **The back is flat** apart from the bare BOOT pad (TP1, x 13.0, y 18.0) and the USB-C's four solder legs (x 2.1–6.3, y ~7.6 and ~16.3). Put the velcro from about x 9 to the right end; it can cover TP1 (recovery only: peel the board off to reach it).
  - **Not against bare steel with exposed copper:** the back is solder-masked over a ground pour, so velcro is fine; don't press the USB-C legs onto the panel.
  - **Indicator LEDs** in the right-hand area (S P C S H P V) face out of the board's front: mount it so you can see them with the cup warmer off.
  - **USB-C:** at the left short edge (it overhangs by 0.96 mm). With the board upright, the left end points down or toward the back depending on orientation: leave room for a plug, or a short extension lead to the back.
- **Connectors:** all six exit upward past the top edge: leave about 15 mm above the board for the plugs and the cable bend.
- **Wiring:** keep the mains wiring and the 5 V leads apart (≥ 6 mm, or a barrier), anchor every wire near its terminal, and use fibreglass sleeve near the boiler.
- **Antenna:** the module's W.FL socket faces the bottom edge; route the pigtail out through a plastic part. The case is steel.
