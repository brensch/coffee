# Hardware

Target: **Gaggia Classic Pro, US 120 V / 60 Hz** (E24, brass boiler). For why this machine, see the README.

Where to find what:
- the board (schematic as code, calculations, simulations): [CIRCUIT.md](CIRCUIT.md);
- what to buy, with verified links and datasheets: [BUY.md](BUY.md);
- how it's wired: [WIRING.md](WIRING.md);
- fitting it: [INSTALL.md](INSTALL.md);
- measuring the machine for the models: [CALIBRATION.md](CALIBRATION.md).

## Design rules

1. **Mains only where it's cheap to make safe.** Since v32 the board carries the small loads' mains on the board: the pump
   (~0.45 A), the valve (~0.1 A) and its own certified Mean Well IRM-10-5 supply. Isolation is verified
   on the routed copper (≥ 6 mm to SELV, 7.83 mm achieved; `isolation_check.py`). The 11 A heater stays
   on a factory zero-cross SSR on the rear panel. A code check fails the build if any net joins a mains
   pin to a SELV pin (only the opto-triacs and the IRM bridge the barrier, inside their certified bodies).
2. **Hardware protection stays in series with the heater.** The stock steam thermostat and the thermal fuse stay wired as they are. Software can only *ask* a failed-short SSR or triac to stop (scenario 12); both usually fail shorted. The firmware also watches the pressure for pump strokes it didn't command (a shorted pump triac) and says to unplug.
3. **Whole mains cycles only.** The engine never does phase-angle control. A zero-cross SSR switches the heater, and zero-cross opto-triacs (MOC3083) fire the pump and valve triacs. That's why no zero-cross detector, dimmer or phase timing is needed: the firmware ticks once per mains cycle from its crystal. The pump's internal diode means each enabled cycle is exactly one stroke.
4. **Outputs are off unless the firmware says otherwise.** Gate pull-downs hold every output off while the module resets, boots or is flashed. This is simulated against a 20 kΩ internal pull-up (sim/REPORT.md).
5. **The firmware does what it can; the board keeps only physics.** The board does switching and sensor-to-voltage. Debounce, calibration, linearisation, fault detection, oversampling and timing are Rust.
6. **Measure what the model needs.** Each sensor maps to model parameters. Anything we can't sense is estimated, and the UI says so.

## What you buy
- **The board:** from JLCPCB, fully assembled (`build/coffee-v33-{gerbers.zip,bom.csv,cpl.csv}`), or SMD-only
  with five through-hole parts to solder yourself (`coffee-v33-smd-*`). Order settings: INSTALL.md §1.
- **Everything else:** [BUY.md](BUY.md): the heater SSR, mains wire and receptacles, GH housings and
  contacts, the thermocouple, the pressure transducer, the antenna, and the tray.

## How it's wired (summary; details in WIRING.md)
| Load | Path |
|---|---|
| Heater | live → stock steam thermostat → thermal fuse → **heater SSR (in place of the brew thermostat)** → element → neutral |
| Pump | live → board **L** → F1 → triac Q5 → board **P** → pump → neutral |
| 3-way valve | live → board **L** → F1 → triac Q6 → board **3** → valve → neutral |
| Board supply | board **L** → F1 → IRM-10-5 → board **N** |

The heater SSR's input is the GH 4-pin from J2 (1 = +5 V to SSR +, 2 = SSR −, switched low). See WIRING.md.

## Enclosure
- **The tray:** the board sits on three M3 standoffs (≥ 4 mm) in a printed PETG/ASA tray, with a solid
  floor under the mains half: it is the insulation between the board's underside (the spades' and IRM's
  pins carry 120 V) and the case. Heat-resistant velcro holds the tray, never the bare board, on the
  right side panel. Hole positions: INSTALL.md §0.
- **Heater SSR:** on the steel rear panel (its heatsink), never on plastic. It dissipates about 12 W at
  full power.
- **Orientation:** the GH plugs exit the top edge and the mains receptacles the left edge. Leave ~15 mm
  above the board for the plugs' cable bend, ~25 mm over it for the IRM, and room at the left for the
  receptacles and their 18 AWG leads.
- **Wiring:** keep the mains wiring and the GH leads apart (≥ 6 mm, or a barrier), anchor every wire near
  its terminal, and use fibreglass sleeve near the boiler.
- **Antenna:** route the module's W.FL pigtail out through a plastic part. The case is steel.
