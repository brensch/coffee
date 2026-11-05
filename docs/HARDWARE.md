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
- **The board:** from JLCPCB, fully assembled (every part on the top side, nothing to solder). The files are `build/coffee-v23-{gerbers.zip,bom.csv,cpl.csv}`; the order settings are in INSTALL.md §1.
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
- **Board:** has no mounting holes. Coordinates below are top view, origin at the top-left corner, x right and y down, in mm, board 37.8 × 26.4:
  - **Underneath:** flat. The only features are bare pads: the USB programming pads (J4, x 17–23, y 8.5–17), BOOT (TP1, x 24.6, y 17.0) and STATUS (TP2, x 30.0, y 16.0).
  - **Rest it on a 1 mm ledge** around the whole perimeter (an open frame), so the pads stay reachable from below.
  - **Clips over the top face** (no part within 1.5 mm of the edge): the left edge y 17.2–20.0 and the top-right corner (top edge x 36.6–37.8). The connector bodies run flush to both long edges, so a clip there would press on a connector; two clips plus the ledge, or a light strap across the module, hold it.
- **Connectors:** eight exit sideways past the long edges (four each), so leave about 8 mm of free space outside both long edges for the GH plugs and their wires.
- **Wiring:** keep the mains wiring and the 5 V leads apart (≥ 6 mm, or a barrier), anchor every wire near its terminal, and use fibreglass sleeve near the boiler.
- **Antenna:** route it out through a plastic part. The case is steel.
