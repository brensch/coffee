# Controller board v23: low voltage only

A 37.8 × 26.4 mm two-layer board, exactly the rectangle of its eight JST GH connectors (butted 4 + 4
along the long edges) and the ESP32 module between them. Every part is on the top side, so JLCPCB's
one-sided assembly fits all of it and there is nothing to solder. It carries
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
| J1 | JST GH 3-pin (1 +5 V, 2–3 GND) | from the 5 V AC-DC module |
| Q4 | AO3401A P-MOSFET, gate straight to GND (v23; Vgs −5 V, inside its ±12 V) | reverse-polarity protection: a swapped lead leaves it off (datasheet leakage ≤ 1 µA; 5 nA simulated). Drop ≤ 15 mV at 250 mA (60 mΩ max) vs ~0.35 V for a diode, so the transducer keeps a full 5 V |
| D2 | B5819W Schottky | USB VBUS (from the programming pads) → +5 V for bench power. With both connected, the module (5.0 V) powers the board and D2 keeps it out of the PC. With USB alone, Q4 conducts backwards and ~4.6 V sits on J1 / the module's output: harmless (it only charges the unpowered module's output capacitor) |
| C6, C7 | 22 µF X5R 0805 | +5 V and +3.3 V bulk, ceramic (nothing to dry out at 60–70 °C) |
| U5 | TI TLV1117LV33 (SOT-223) | 3.3 V, 1 A, stable with ≥ 1 µF ceramic at any ESR (TI SBVS160C). **Chosen for heat:** from 5 V it drops 1.7 V; at the C6's ~100 mA average with Wi-Fi listening that is ~0.17 W. Its RθJA of 62.9 °C/W makes that +11 °C. The SOT-23-5 ME6211 used up to v17 is rated 300 mW, which implies ~333 °C/W, or +57 °C: past its 125 °C junction limit at 70 °C ambient. Input 5.5 V max recommended (6 V absolute); the board's +5 V is ≤ 5.25 V |
| C3, C4 | 10 µF + 100 nF | C3 at the LDO output (TI asks for ≥ 1 µF), C4 (0402) at the module's top-left corner. v23 has no room beside the 3V3 pin itself: J10 sits right over it, and J10's own 3V3 pin feeds the same rail there |

### Controller
| Part | Value | Why |
|---|---|---|
| U4 | ESP32-C6-MINI-1U-H4 | Wi-Fi 6 / BLE, external antenna on **W.FL / MHF III** (the steel case blocks a PCB antenna), ≤ 2.33 dBi |
| R10, C5 | 10k / 1 µF | EN reset delay (Espressif) |
| R11 | 10k | GPIO8 high (needed for download mode from the BOOT pad) |
| TP1 | bare pad on the **underside** | BOOT (GPIO9), recovery only: the C6 resets itself into download mode over USB |
| TP2 | bare pad on the underside | STATUS (GPIO23): the firmware's heartbeat. v23 dropped the on-board LED (D1, R12): the board sits out of sight and the front-panel LEDs show the state. Clip an LED + 1k to GND here when debugging |
| J4 | 4 bare pads on the **underside** (2.54 mm, nothing fitted) | USB 2.0 for the first flash, logs and bench power: 1 = VBUS, 2 = D−, 3 = D+, 4 = GND, D± on GPIO12/13. Tack a USB lead on (red, white, green, black) or hold a pin header against them. v23 replaced the USB-C socket: after the first flash, updates go over Wi-Fi |

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
| Front panel, J7 (7-pin) → GPIO14/6/7 in, GPIO21/22/16 out | R29/R18/R19 1k series at the pins, R28/R20/R21 4.7k pull-ups on the GPIO side (v23); R30–R32 330 Ω LED resistors | the stock rocker switches as dry contacts. Closed: 3.3 V × 1k / 5.7k = 0.58 V at the GPIO, under the C6's 0.83 V low threshold; ~0.6 mA wetting current; a 5 V plug in the wrong socket pushes ~1.5 mA through the 1k into a clamp diode |
| Scale, J10 (6-pin) → GPIO17 (SCK) out, GPIO4/5 (DOUT) in | R36 10k SCK pull-down (no series resistors since v23, as GaggiMate and Gaggiuino) | dual HX711 next to the load cells (5 V analog, 3V3 logic); only slow digital lines on the lead. v19–v22 had a bug: R36 sat on a separate net and pulled nothing down |
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
| 6 / 7 | SW_BREW / SW_STEAM | | 23 | STATUS LED |
| 8 | pull-up (strap) | | 9 | BOOT pad (strap) |
| 14 | SW_POWER (front panel) | | 21 / 22 / 16 (TXD0) | LED power / coffee / steam |
| 17 (RXD0) | SCALE_SCK | | 4 / 5 | SCALE_DOUT1 / DOUT2 (straps only SDIO edges) |

The analog inputs are on ADC1, which works alongside Wi-Fi. No output sits on a strapping pin (GPIO 4, 5, 8, 9, 15). `controller.py` checks this map against the firmware on every run.

## Layout

**Board:** 37.8 × 26.4 mm, 2 layers, 1 oz, every part on the top side. The board is the rectangle of the connectors and the module: four GH connectors butted along each long edge, and the ESP32 module at the right end touching both rows. Passives are 0402 (JLC basic parts; Economic PCBA places 0402).

**Connectors:** openings outward past the long edges:
- top edge (left to right): 5 V in (J1), pressure (J6), thermistor (J5), scale (J10, 6-pin, over the module: its data pins land on the module's DOUT pins);
- bottom edge: valve (J8), heater (J2), front panel (J7, 7-pin, beside the module's switch and LED pins), pump (J3, under the module).

**Earlier versions:** v21 (40 × 24 mm, connectors hand-soldered underneath) and v22 (44.5 × 29.4 mm, all on top) are in `versions/` with their files still in `build/` (`coffee-v21-*`, `coffee-v22-*`). They have the USB-C socket and the R36 bug above.

All eight are JST GH (1.25 mm, positive latch, which suits a machine shaken by a vibration pump) and side entry, so the cables leave past the long edges.

**Placement:** by hand, judged by eye between routing runs: `check_place.py` draws the ratsnest and courtyard overlaps; it moves nothing.

**Routing:** end to end by **fastroute 0.1.14**. KiCad adds the ground pours on both layers afterwards, plus GND stitching vias wherever both layers are free. The current board has 0 unconnected, 68 routed vias and 89 stitching vias. Every ground pad of the power parts and the module sits in one top-layer pour. All vias are tented: the Gerbers have no mask openings on them.

**Rules:** JLCPCB standard capabilities only, the cheapest tier:
- 0.2 mm tracks and gaps (0.15 mm DRC minimum; JLC's is 0.127 mm);
- 0.3 mm drills and 0.6 mm vias;
- 0.3 mm copper to the board edge;
- 0.2 mm hole to copper.

**KiCad DRC:** 0 violations, 0 unconnected, run on a fresh load of the saved board file. The report still lists library-table notes, because the footprints are embedded and no KiCad library table exists on the build machine.

**Silkscreen:** labels are on the **bottom**, behind each connector: a word behind every pin and the connector's name, plus the programming-pad names (5V, D−, D+, GND), BOOT, STATUS, the title and the bean logo. J7 (front panel) is labelled FRONT.

The top is too dense for text and keeps only the parts' own outlines and pin-1 marks.

**JLC:** 21 BOM lines, all placed by JLC on the top side; 5 extended part types (the three GH connector sizes, the ESP32-C6 module, the LDO), the rest basic. The 0402 100 nF and 22 nF are pinned to basic parts (C1525, C1532): the matcher picked extended ones. U5 is pinned to C15578, TI's own part: JLC also lists a different maker's 12 V part under the same name. Two gates check the files:
- `jlc_cpl.py` solves every part's position and rotation against JLC's own footprints; all 47 placed parts land on their pads (the two new 0402 capacitors solved from same-package records: EasyEDA's API refused this machine).
- `jlc_pins.py` then compares JLC's pin names with the schematic's for every polarised part: 70 pin functions, 0 mismatches. This gate exists because the v15 position file had the status LED reversed.

## Simulation (ngspice, `sim/run.py`)
61 checks, all passing:
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
