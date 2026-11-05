# Hardware

Target: **Gaggia Classic Pro, US 120 V / 60 Hz** (aluminium boiler). For why this machine, see the README.

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
- **The board:** from JLCPCB, top side assembled. The files are `build/coffee-v21-{gerbers.zip,bom.csv,cpl.csv}`; you solder the eight connectors onto the bottom yourself (INSTALL.md §1 step 6); the order settings are in INSTALL.md §1.
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

The board's JST GH leads drive the module inputs:
- J2 → heater SSR;
- J3 → pump SSR;
- J8 → valve SSR board.

On each, pin 1 = +5 V and pin 2 = OUT.

## Enclosure
- **Mounting (as GaggiMate):** the heater SSR bolts to the inside of the rear panel at the ventilation slots (M4 bolts or cable ties through the slots). The board, the two SSR boards and the 5 V supply sit on a plastic part (a printed holder or bracket), never directly on the steel: their undersides carry mains. No DIN rail.
- **Heater SSR:** on the steel rear panel (its heatsink), never on plastic. It dissipates about 12 W at full power.
- **Board:** has no mounting holes. Coordinates below are top view, origin at the top-left corner, x right and y down, in mm, board 40 × 24:
  - **Underneath:** the eight GH connectors hang below the board along both long edges (y 0–6.4 and 17.6–24). The middle strip, y 6.4–17.6, is flat apart from TP1 and the USB-C shell legs (x 1.3–7.4, at y 7.2–8.2 and 15.8–16.8).
  - **Rest it on a pedestal** under that middle strip, at least 5 mm tall so the connector bodies clear the floor. Leave a hole in it at **x 15, y 12**, at least 4 mm across, for tweezers on the BOOT pad (TP1), and relieve it where the USB-C legs come through.
  - **Clips over the top face** (no part within 1.5 mm of the edge): right edge y 11.5–17.2, left edge y 0–6.6 and y 20.4–24.
  - **USB-C:** overhangs the left short edge by 0.96 mm from y 6.7 to 17.3. Leave room there for a plug.
- **Connectors:** eight exit sideways past the long edges (four each), so leave about 8 mm of free space outside both long edges for the GH plugs and their wires.
- **Wiring:** keep the mains wiring and the 5 V leads apart (≥ 6 mm, or a barrier), anchor every wire near its terminal, and use fibreglass sleeve near the boiler.
- **Antenna:** route it out through a plastic part. The case is steel.
