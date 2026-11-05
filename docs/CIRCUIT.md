# Controller board v27: low voltage only

A 73.2 × 25 mm two-layer board with all eight JST GH connectors in one row along its top edge, so
every cable leaves one side: it stands upright in the machine's back-right corner (where GaggiMate
mounts its controller) with the cables going up into the loom. The ESP32 module sits in the middle,
straight against the connector row (pinless side down), with a seven-LED indicator panel under it. The
USB-C is at the left end. No mounting holes: heat-resistant velcro on the flat back holds it. Every part is on the top side, so JLCPCB's one-sided assembly fits all of it and there is nothing
to solder. It carries
**no mains**:
- **The board's job:** read the sensors and drive three 5 V outputs.
- **Mains switching:** certified modules mounted beside it in the machine do that:
  - a zero-cross SSR for the heater (in place of the brew thermostat);
  - a 1-channel G3MB-202P SSR board for the vibration pump;
  - a 1-channel 5 V SSR board (Omron G3MB-202P) for the 3-way valve, with a snubber;
  - a certified 5 V AC-DC module that powers the board.

Parts and links: [BUY.md](BUY.md). Wiring: [WIRING.md](WIRING.md).

Schematic as code: `hardware/controller/controller.py` (SKiDL). Simulations:
`hardware/controller/sim/run.py` → [sim/REPORT.md](../hardware/controller/sim/REPORT.md), 61/61 pass.

Every part on the board was checked against its manufacturer's datasheet (pin map, package and every
value the design relies on); the off-board parts are checked in BUY.md.

## Why the mains came off the board

The v6–v14 boards switched the heater (12 A), pump and valve with on-board opto-triacs and triacs.
The board is the one thing that can't be tested before paying for it, and every mains risk lived on it:
- creepage and isolation design;
- 12 A copper, and the triac-pin clearance problem that no Specctra autorouter can express;
- the snubber/pump-diode interaction behind the one real-world failure report found for that circuit.

Moving every mains function to factory-tested, replaceable modules makes the board:
- testable on USB at a desk;
- routable end to end by fastroute;
- about half the price.

The old boards are kept in `hardware/controller/versions/` (v1–v14).

## Blocks

### Power in
| Part | Value | Why |
|---|---|---|
| J1 | JST GH **2-pin** (1 +5 V, 2 GND) | from the 5 V AC-DC module. 2-pin since v24, unlike the 3-pin outputs: with a 3-pin J1, swapping the supply lead and an output lead back-fed +5 V through Q4 and held that load on (review S1) |
| Q4 | AO3401A P-MOSFET, gate straight to GND (Vgs −5 V, inside its ±12 V) | reverse-polarity protection: a swapped lead leaves it off (datasheet leakage ≤ 1 µA; 5 nA simulated). Drop ≤ 15 mV at 250 mA (60 mΩ max) vs ~0.35 V for a diode, so the transducer keeps a full 5 V |
| D2 | B5819W Schottky | USB VBUS → +5 V for bench power. With both connected, the module (5.0 V) powers the board and D2 keeps it out of the PC. With USB alone, Q4 conducts backwards and ~4.6 V sits on J1 / the module's output: harmless (it only charges the unpowered module's output capacitor) |
| C6, C7 | 22 µF X5R 0805 | +5 V and +3.3 V bulk, ceramic (nothing to dry out at 60–70 °C) |
| U5 | TI TLV1117LV33 (SOT-223) | 3.3 V, 1 A, stable with ≥ 1 µF ceramic at any ESR (TI SBVS160C). **Chosen for heat:** from 5 V it drops 1.7 V; at the C6's ~100 mA average with Wi-Fi listening that is ~0.17 W. Its RθJA of 62.9 °C/W makes that +11 °C. The SOT-23-5 ME6211 used up to v17 is rated 300 mW, which implies ~333 °C/W, or +57 °C: past its 125 °C junction limit at 70 °C ambient. Input 5.5 V max recommended (6 V absolute); the board's +5 V is ≤ 5.25 V |
| C3, C4 | 10 µF + 100 nF | C3 at the LDO output (TI asks for ≥ 1 µF), C4 (0402) at the module's top-left corner. There's no room beside the 3V3 pin itself: J10 sits right over it, and J10's own 3V3 pin feeds the same rail there (the module has 10 µF + 2 × 100 nF inside) |

### Controller
| Part | Value | Why |
|---|---|---|
| U4 | ESP32-C6-MINI-1U-H4 | Wi-Fi 6 / BLE, external antenna on **W.FL / MHF III** (the steel case blocks a PCB antenna), ≤ 2.33 dBi |
| R10, C5 | 10k / 1 µF | EN reset delay (Espressif) |
| R11 | 10k | GPIO8 high (needed for download mode from the BOOT pad) |
| TP1 | bare pad on the **underside** | BOOT (GPIO9), recovery only: the C6 resets itself into download mode over USB |
| D1, R12 | red LED, 1k on **GPIO16 (U0TXD)** | status, ~1.3 mA. On the UART TX pin on purpose (v24): the ROM prints its boot log there, so the LED flickers at every reset and glows while the chip waits in download mode (programming); then the firmware drives it as a heartbeat. The steam lamp moved to GPIO23 (it used to light at every boot on this pin: review M1). The LED's datasheet numbers pin ① = anode, the opposite of KiCad's pad 1; the CPL gate maps it, but glance at D1 in JLC's preview |
| J4, R13, R14 | HRO TYPE-C-31-M-12, 5.1k CC to GND | USB 2.0 for flashing, logs and bench power; D± on GPIO12/13. Back at the left edge in v24 (v23 had bare pads). Drilled to HRO's recommended layout: locating holes Ø0.60 mm for the Ø0.50 mm posts, shell slots 0.6 × 1.7 and 0.6 × 1.4 mm; the shell legs get paste so reflow solders them. The shell overhangs the left edge by 0.96 mm |

### Outputs (J2 heater, J3 pump, J8 valve)
GPIO → AO3400A gate, **3.3 kΩ gate pull-down**. The MOSFET sinks the module's input, and every connector uses the same pinout:
- pin 1 = +5 V;
- pin 2 = OUT (pulled low = on);
- pin 3 = GND.

Notes per part:
- **3.3 kΩ pull-down:** holds the gate under 0.47 V even against a 20 kΩ internal pull-up while the pin floats during reset, boot or flashing. A 100 kΩ pull-down would let a 45 kΩ pull-up switch the heater on at boot (2.3 V on the gate, simulated).
- **No flyback diodes:** all three outputs drive SSR inputs, which aren't inductive (3.9–5.6 V spike with 1 µH of cable, simulated). v23 dropped the valve's 1N4148W; a relay module, if you ever use one, brings its own.
- **No LED per output:** every module shows its input on its own LED.
- **Margins, worst case** (GPIO at 2.64 V, the C6's guaranteed 0.8 × VDD output high; AO3400A at its maximum threshold; +5 V at 4.6 V from USB):
  - the heater SSR sees 4.6 V, over 3.0 V;
  - each SSR board sees ~4.6 V on VCC (a 5 V board; the G3MB-202P itself is a 5 V-input part). On USB alone, check at bring-up that the board LEDs and SSRs switch.

### Sensors
| Input | Parts | Notes |
|---|---|---|
| Thermistor, J5 → R27 1k → GPIO1 (ADC1) | R15 4.7k to 3V3, C8 100 nF, R27 1k series | **Molex 218537-3919** 50 kΩ ±1 % ring-tab probe (−40…250 °C, 1500 VAC body isolation, PTFE leads; Molex sales drawing) under an M4 screw at the brew-thermostat seat. The firmware uses a Steinhart-Hart fit to Molex's R-T table (within ~1 °C of it from 25 to 200 °C; the table's own spread is ±3.6 % at 95 °C, ~±0.9 °C) and compensates the probe's lag for control (`ntc.probe_lag_s`, 15 s per Molex). 4.7k sits near the probe's 3.9 kΩ at 93 °C: 1.50 V there, 26 mV/°C. Self-heating 0.57 mW, ~0.6 °C in still air (1 mW/°C), less on the boiler; calibrated out. R27 carries no current in use (no error) and limits a 5 V plug pushed into J5 by mistake to ~1.2 mA into the pin. **Open probe:** the node reads the 3.3 V rail; the firmware calls ≥ 3200 mV a fault (heater off), 100 mV under the rail because the ADC is only specified to ±40 mV there. That also faults a boiler colder than ~2 °C. An open probe that reads under the threshold anyway, or a probe stuck at any value, is caught by HEATER_INEFFECTIVE: an energy balance of the heat delivered against the rise the reading shows (≈ 2 minutes) |
| Pressure, J6 → GPIO0 (ADC1) | R16 47k / R17 100k divider, C9 22 nF | Any 0.5–4.5 V ratiometric 5 V transducer: GaggiMate's 0–1.6 MPa sensor by choice (BUY.md §10b), or TE's M3200 (whose datasheet asks for a load > 100 kΩ, hence the 147 kΩ divider). 4.5 V → 3.06 V; −3 dB at ~226 Hz keeps the pump's 60 Hz pulses. On USB power +5 V is ~4.5–4.6 V, so pressure readings there are only indicative |
| Front panel, J7 (7-pin) → GPIO14/6/7 in, GPIO21/22/23 out | R29/R18/R19 1k series at the pins, R28/R20/R21 **10k** pull-ups on the GPIO side; R30–R32 330 Ω LED resistors (steam lamp on GPIO23 since v24) | the stock rocker switches as dry contacts. Closed: 3.3 V × 1k / (1k + 10k ∥ 45k internal) = 0.36 V at the GPIO, 0.46 V under the C6's 0.825 V low threshold (4.7k left only 0.2 V: review S2); ~0.36 mA wetting current; a 5 V plug in the wrong socket pushes ~1.5 mA through the 1k into a clamp diode |
| Scale, J10 (6-pin) → GPIO17 (SCK) out, GPIO4/5 (DOUT) in | R36 10k SCK pull-down; the firmware turns on GPIO4/5's internal pull-ups, so a missing scale reads "not ready" instead of noise (review S5). No series resistors, as GaggiMate and Gaggiuino | dual HX711 next to the load cells (5 V analog, 3V3 logic); only slow digital lines on the lead. v19–v22 had a bug: R36 sat on a separate net and pulled nothing down |
| Rail sense → GPIO2/3 | R22/R23 (3V3/2), R24/R25 (5V/3), C10, C11 | the ADC measures absolute volts; measuring both rails lets the firmware divide them out (ratiometric) |

**1 % resistors instead of 0.1 %.** Every resistor is a JLC basic part, which saves three extended-part fees. The cost in accuracy, before the calibration that removes it:
- thermistor reference: about 0.3 °C at 93 °C;
- pressure gain: about 0.06 bar at 9 bar.

### Pin map (ESP32-C6)
| GPIO | Net | | GPIO | Net |
|---|---|---|---|---|
| 0 | P_ADC (pressure) | | 12 / 13 | USB D− / D+ |
| 1 | NTC | | 18 | VALVE_CMD |
| 2 | V3_SNS | | 19 | PUMP_CMD |
| 3 | V5_SNS | | 20 | HEATER_CMD |
| 6 / 7 | SW_BREW / SW_STEAM | | 16 (TXD0) | STATUS LED (also shows the ROM boot log) |
| 8 | pull-up (strap) | | 9 | BOOT pad (strap) |
| 14 | SW_POWER (front panel) | | 21 / 22 / 23 | LED power / coffee / steam |
| 17 (RXD0) | SCALE_SCK | | 4 / 5 | SCALE_DOUT1 / DOUT2 (straps only SDIO edges) |

The analog inputs are on ADC1, which works alongside Wi-Fi. No output sits on a strapping pin (GPIO 4, 5, 8, 9, 15). `controller.py` checks this map against the firmware on every run.

## Layout

**Board:** 73.2 × 25 mm, 2 layers, 1 oz, every part on the top side, 0603 passives (all JLC basic). No mounting holes (v27): the back is flat apart from the bare BOOT pad and the USB-C's four solder legs at the left end, so a strip of heat-resistant velcro holds it.

**Connectors:** one row along the top edge, openings outward (up when mounted), left to right: 5 V in (J1, 2-pin), valve (J8), heater (J2), pump (J3), scale (J10, 6-pin), front panel (J7, 7-pin), pressure (J6), thermistor (J5). The module sits straight under the scale connector, touching the row; every other connector's parts sit directly under its own pins (drivers under the outputs, dividers under the sensors, the switch and LED resistors under the front panel). USB-C (J4) at the left short edge.

**Indicator panel (v27):** seven red LEDs in a row under the module, labelled on the top silkscreen S P C S H P V:
- **S**: status (GPIO16 / U0TXD: boot log, programming, heartbeat);
- **P C S**: the three front-panel lamps (power, coffee, steam), each on the same GPIO as its lamp through its own 1k (so the board shows exactly what the lamps get);
- **H P V**: heater, pump, valve, each on its output's gate drive through 1k, lit exactly while the board turns that SSR on.
All are KT-0603R (C2286, basic) with 1k (basic): ~1.3 mA each. Added cost ~$0.10 per assembled board (parts + joints), no setup fee.

**Earlier versions:** v21 (40 × 24 mm, connectors hand-soldered underneath), v22 (44.5 × 29.4 mm), v23 (37.8 × 26.4 mm, no USB-C) and v24 (46.6 × 26.4 mm, connectors on both long edges) v25 (73.2 × 24, module at the end, 0402) and v26 (73.2 × 26, parts strip between the plugs and the module, M3 holes) are in `versions/` with their files still in `build/`. v21/v22 have the R36 bug above; v21–v23 have the review's S1/S2 issues.

All eight are JST GH (1.25 mm, positive latch, which suits a machine shaken by a vibration pump) and side entry, so the cables leave past the long edges.

**Placement:** by hand, judged by eye between routing runs: `check_place.py` draws the ratsnest and courtyard overlaps; it moves nothing.

**Routing:** end to end by **fastroute 0.1.14**. KiCad adds the ground pours on both layers afterwards, plus GND stitching vias wherever both layers are free. The current board has 0 unconnected and 89 routed vias, plus 35 GND stitching vias (a 4 mm grid). Every ground pad of the power parts and the module sits in one top-layer pour. All vias are tented: the Gerbers have no mask openings on them.

**Rules:** JLCPCB standard capabilities only, the cheapest tier:
- 0.2 mm tracks and gaps (0.15 mm DRC minimum; JLC's is 0.127 mm);
- 0.3 mm drills and 0.6 mm vias;
- 0.3 mm copper to the board edge;
- 0.2 mm hole to copper.

**KiCad DRC:** 0 violations, 0 unconnected, run on a fresh load of the saved board file. The report still lists library-table notes, because the footprints are embedded and no KiCad library table exists on the build machine.

**Silkscreen:** labels are on the **bottom**, behind each connector: a word behind every pin and the connector's name, plus USB, BOOT, the title and the bean logo. J7 (front panel) is labelled FRONT.

The top is too dense for text and keeps only the parts' own outlines and pin-1 marks.

**JLC:** 25 BOM lines, all placed by JLC on the top side; 7 extended part types (the four GH connector sizes, the USB-C, the ESP32-C6 module, the LDO), the rest basic. None of the seven has a basic or preferred-extended equivalent in JLC's library (searched 2026-11-05: no latching wire-to-board connector, USB-C or ESP32 module is basic); only the LDO does (AMS1117-3.3, C6186), kept as the TI part for ceramic-capacitor stability and lower dropout. The 0402 100 nF and 22 nF are pinned to basic parts (C1525, C1532): the matcher picked extended ones. U5 is pinned to C15578, TI's own part: JLC also lists a different maker's 12 V part under the same name. Two gates check the files:
- `jlc_cpl.py` solves every part's position and rotation against JLC's own footprints; all 64 placed parts land on their pads (the two new 0402 capacitors solved from same-package records: EasyEDA's API refused this machine).
- `jlc_pins.py` then compares JLC's pin names with the schematic's for every polarised part: 102 pin functions, 0 mismatches (every LED's polarity included). This gate exists because the v15 position file had the status LED reversed.

## Simulation (ngspice, `sim/run.py`)
54 checks, all passing (updated to v24 after the review found the v23 models stale):
- each output at its worst corners with each module type;
- outputs off during reset against a weak internal pull-up;
- SSR and SSR-board turn-off spikes (no flyback diodes);
- reverse polarity;
- the 3.3 V rail under 500 mA Wi-Fi bursts from 4.65–5.0 V (≥ 3.29 V), with the LDO's dropout at its datasheet maximum;
- the pressure filter, the transducer's > 100 kΩ load, and ADC kick-back (an estimate: the C6 datasheet doesn't give the ADC's sampling capacitance);
- thermistor range, sensitivity and self-heating;
- the 1 % resistor error budget;
- note: the module loads are behavioural (an SSR input as a current limit, the relay module as 131 Ω + 50 mH), so the on-state checks confirm the MOSFET's drop and the +5 V corner rather than model the modules' internals;
- a 5 V plug in the thermistor socket (1.2 mA into GPIO1 through R27).

The models are behavioural and fitted to datasheet values, so the point is the margins at the corners.

## Status and what needs a human
- **D2 orientation (verified):** its rotation came from the same-package 1N4148W record while EasyEDA's API refused this machine; JLC's own footprint and symbol drawings confirm pad 1 on the same side and pin 1 = cathode for both.
- **Pump SSR on the real pump:** on the first install, count strokes and listen for hum while it's off. This is the one module behaviour not verified against the Ulka's half-wave diode load.
- **Valve coil:** read its label to confirm the current (expected 80–150 mA, well inside the SSR board's 2 A; the snubber keeps a small load switching cleanly).
- **Thermistor:** the firmware defaults are the Molex 218537-3919's (Steinhart-Hart from its table, B25/95 4109, 15 s lag). A different probe needs its own coefficients. The two-point calibration removes the rest.
- **First article:** bench bring-up on USB (INSTALL.md §3) before anything touches the machine. Also check there:
  - the raw 3V3 sense (`V3_SNS` × 2) reads within ±5 % of 3300 mV: outside that the firmware reports the thermistor open (safe, but the board won't heat);
  - with J5 unplugged, the raw NTC reading is ≥ 3200 mV (the open-probe threshold);
  - with a scope if you have one, +5 V when a USB cable is hot-plugged: the TLV1117LV's absolute maximum input is 6 V.
- **Accepted risk:** nothing on the board clamps +5 V below the LDO's 6 V absolute maximum. A 5 V supply that fails high could kill the LDO and the C6; the outputs' gate pull-downs then hold the modules off, and the steam thermostat and thermal fuse stay in series with the heater.
