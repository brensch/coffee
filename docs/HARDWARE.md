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
   - a zero-cross SSR for the heater (on an aluminium heatsink);
   - a zero-cross SSR without a snubber for the pump;
   - a relay module for the 3-way valve;
   - an EN 60335 / UL 62368 5 V AC-DC module.

   The board sinks each module's 5 V input with a MOSFET. A code check fails the build if any mains net appears on the board.
2. **Hardware protection stays in series with the heater.** The stock steam thermostat and the thermal fuse stay wired as they are. Software can only *ask* a failed-short SSR to stop (scenario 12); SSRs, like triacs, usually fail shorted.
3. **Whole mains cycles only.** The engine never does phase-angle control. Zero-cross SSRs switch the heater and the pump, and the relay switches the valve. That's why no zero-cross detector, dimmer or phase timing is needed: the firmware ticks once per mains cycle from its crystal. The pump's internal diode means each enabled cycle is exactly one stroke.
4. **Outputs are off unless the firmware says otherwise.** Gate pull-downs hold every output off while the module resets, boots or is flashed. This is simulated against a 20 kΩ internal pull-up (sim/REPORT.md).
5. **The firmware does what it can; the board keeps only physics.** The board does switching and sensor-to-voltage. Debounce, calibration, linearisation, fault detection, oversampling and timing are Rust.
6. **Measure what the model needs.** Each sensor maps to model parameters. Anything we can't sense is estimated, and the UI says so.

## What you buy
- **The board:** from JLCPCB, fully assembled, all SMD on one side. The files are `build/coffee-v17-{gerbers.zip,bom.csv,cpl.csv}`; the order settings are in INSTALL.md §1.
- **Everything else:** [BUY.md](BUY.md):
  - modules, heatsink and fuse;
  - wire and crimps;
  - JST GH leads;
  - thermistor, pressure transducer and antenna.

## How it's wired (summary; details in WIRING.md)
| Load | Path |
|---|---|
| Heater | live → stock steam thermostat → thermal fuse → **heater SSR (in place of the brew thermostat)** → element → neutral |
| Pump | switched live → **pump SSR** → pump → neutral |
| 3-way valve | switched live → **relay contact (NO)** → valve → neutral |
| Board | switched live → 1 A slow-blow fuse → **5 V module** → board J1 |

The board's JST GH leads drive the module inputs:
- J2 → heater SSR;
- J3 → pump SSR;
- J8 → relay coil.

On each, pin 1 = +5 V and pin 2 = OUT.

## Enclosure
- **Holder:** one 3D-printed holder carries the board, the pump SSR, the relay (on a short DIN rail) and the 5 V module. Use flame-retardant polycarbonate (V-0), with brass heat-set inserts so no screw clamps bare plastic.
- **Heater SSR:** goes on its aluminium heatsink, never on plastic. It dissipates about 12 W at full power.
- **Board:** has no mounting holes. The holder grips its edges. Coordinates below are top view, origin at the top-left corner, x right and y down, in mm, board 37.3 × 27:
  - **Ledge:** the underside has no parts, so the board can sit on a ledge 1 mm wide around its whole perimeter. The ledge only touches solder mask over the ground pour, which stops 0.3 mm short of the edge.
  - **USB-C notch:** the USB-C shell overhangs the bottom edge by 0.46 mm from x 16.4 to 25.4 (its datasheet's PCB-edge line). Leave the ledge open there, and leave room for a plug.
  - **Clips over the top face:** a lip may reach 1.5 mm onto the top face only where no part sits within 1.5 mm of the edge:
    - the bottom-left corner: bottom edge x 0–7.3 and left edge y 22.7–27;
    - the right edge, y 17.1–27;
    - the bottom edge, x 35.7–37.3.

    Three clips there, plus the ledge, hold the board. Every other stretch of edge has a connector or a part at it.
  - **BOOT pad:** TP1 is on the underside at x 25.5, y 11.0. Leave a hole under it, at least 4 mm across, for tweezers.
  - **Connectors:** all seven exit sideways past the edges, so leave about 8 mm of free space outside every edge for the GH plugs and their wires.
- **Wiring:** keep the mains wiring and the 5 V leads apart (≥ 6 mm, or a barrier), anchor every wire near its terminal, and use fibreglass sleeve near the boiler.
- **Antenna:** route it out through a plastic part. The case is steel.
