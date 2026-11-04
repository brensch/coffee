# Controller board v17: low voltage only

A 37.3 × 27 mm two-layer board with every part on the top side, assembled by JLCPCB. It carries
**no mains**:
- **The board's job:** read the sensors and drive three 5 V outputs.
- **Mains switching:** certified modules mounted beside it in the machine do that:
  - a zero-cross SSR for the heater (in place of the brew thermostat);
  - a zero-cross SSR without a snubber for the vibration pump;
  - a 5 V relay module for the 3-way valve;
  - a certified 5 V AC-DC module that powers the board.

Parts and links: [BUY.md](BUY.md). Wiring: [WIRING.md](WIRING.md).

Schematic as code: `hardware/controller/controller.py` (SKiDL). Simulations:
`hardware/controller/sim/run.py` → [sim/REPORT.md](../hardware/controller/sim/REPORT.md), 60/60 pass.

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
| Q4 + R26 | AO3401A P-MOSFET, 10k gate to GND | reverse-polarity protection: a swapped lead leaves it off (5 nA into the board). Drop ~10 mV vs ~0.35 V for a diode, so the transducer keeps a full 5 V |
| D2 | B5819W Schottky | USB VBUS → +5 V for bench power. With both connected, the module (5.0 V) powers the board and D2 keeps it out of the PC. With USB alone, Q4 conducts backwards and ~4.6 V sits on J1 / the module's output: harmless (it only charges the unpowered module's output capacitor) |
| C6, C7 | 22 µF X5R 0805 | +5 V and +3.3 V bulk, ceramic (nothing to dry out at 60–70 °C) |
| U5 | ME6211C33 (SOT-23-5), EN tied to VIN | 3.3 V, 500 mA, stable with ceramic caps (the AMS1117 needs tantalum ESR) |
| C3, C4 | 10 µF + 100 nF beside the module's 3V3 pin | decoupling: C4's pads sit level with the module's 3V3 and GND pins, C3 just below |

### Controller
| Part | Value | Why |
|---|---|---|
| U4 | ESP32-C6-MINI-1U-H4 | Wi-Fi 6 / BLE, external antenna on **W.FL / MHF III** (the steel case blocks a PCB antenna), ≤ 2.33 dBi |
| R10, C5 | 10k / 1 µF | EN reset delay (Espressif) |
| R11 | 10k | GPIO8 high (needed for download mode from the BOOT pad) |
| TP1 | bare pad on the **underside** | BOOT (GPIO9), recovery only: the C6 resets itself into download mode over USB |
| D1, R12 | red LED, 1k on GPIO23 | status, ~1.3 mA |
| J4, R13, R14 | HRO TYPE-C-31-M-12, 5.1k CC to GND | USB 2.0 for flashing, logs and bench power; D± on GPIO12/13. Locating-post holes 1.1 mm (KiCad's and JLC's footprints put the posts 0.18 mm apart; the 0.5 mm posts seat either way). The shell legs get paste so reflow solders them. The shell overhangs the bottom edge by 0.46 mm, per the datasheet's PCB-edge line |

### Outputs (J2 heater, J3 pump, J8 valve)
GPIO → AO3400A gate, **3.3 kΩ gate pull-down**. The MOSFET sinks the module's input, and every connector uses the same pinout:
- pin 1 = +5 V;
- pin 2 = OUT (pulled low = on);
- pin 3 = GND.

Notes per part:
- **3.3 kΩ pull-down:** holds the gate under 0.47 V even against a 20 kΩ internal pull-up while the pin floats during reset, boot or flashing. A 100 kΩ pull-down would let a 45 kΩ pull-up switch the heater on at boot (2.3 V on the gate, simulated).
- **J8 (valve):** drives a relay module, so it has a 1N4148W flyback diode. The simulated turn-off spike is 5.8 V. The Phoenix module also has its own suppression diode; the board's diode covers a bare relay or coil plugged in instead.
- **J2 and J3:** the SSR inputs aren't inductive, so they need no diode (3.9–5.1 V spike with 1 µH of cable).
- **No LED per output:** every module shows its input on its own LED.
- **Margins, worst case** (GPIO at 3.0 V, AO3400A at its maximum threshold, +5 V at 4.6 V from USB):
  - the pump SSR sees 4.6 V, over its 3.0 V turn-on;
  - the heater SSR sees 4.6 V, over 3.0 V;
  - the relay module sees 4.6 V, over the 4.2 V bottom of its input range. This is the tightest margin. On USB alone, a weak port (4.75 V, minus 0.35–0.45 V across D2) can leave it just under 4.2 V. In the machine, the 5 V module gives it 5.0 V.

### Sensors
| Input | Parts | Notes |
|---|---|---|
| Thermistor, J5 → R27 1k → GPIO1 (ADC1) | R15 8.2k to 3V3, C8 100 nF, R27 1k series | 100 kΩ probe (Semitec 104NT glass bead, B 4267; firmware default) in the brew-thermostat hole. 1.52 V at 93 °C, 26 mV/°C there. R27 carries no current in use (no error) and limits a 5 V plug pushed into J5 by mistake to ~1.2 mA into the pin. **Open probe:** the node reads the 3.3 V rail; the firmware calls ≥ 3200 mV a fault (heater off), 100 mV under the rail because the ADC is only specified to ±40 mV there. That also faults a boiler colder than ~6 °C, the safe way to be wrong |
| Pressure, J6 → GPIO0 (ADC1) | R16 10k / R17 20k divider, C9 100 nF | 0.5–4.5 V ratiometric transducer on +5 V. 4.5 V → 2.97 V; −3 dB at 233 Hz keeps the pump's 60 Hz pulses |
| Switches, J7 → GPIO6/7 | R20/R21 4.7k pull-ups, R18/R19 1k series | the stock brew/steam switches as dry contacts. 0.7 mA wetting current; a 5 V plug in the wrong socket pushes ~1.7 mA into a clamp diode |
| Rail sense → GPIO2/3 | R22/R23 (3V3/2), R24/R25 (5V/3), C10, C11 | the ADC measures absolute volts; measuring both rails lets the firmware divide them out (ratiometric) |

**1 % resistors instead of 0.1 %.** Every resistor is a JLC basic part, which saves three extended-part fees. The cost in accuracy, before the calibration that removes it:
- thermistor reference: 0.34 °C at 93 °C;
- pressure gain: 0.06 bar at 9 bar.

### Pin map (ESP32-C6)
| GPIO | Net | | GPIO | Net |
|---|---|---|---|---|
| 0 | P_ADC (pressure) | | 12 / 13 | USB D− / D+ |
| 1 | NTC | | 18 | VALVE_CMD |
| 2 | V3_SNS | | 19 | PUMP_CMD |
| 3 | V5_SNS | | 20 | HEATER_CMD |
| 6 / 7 | SW_BREW / SW_STEAM | | 23 | STATUS LED |
| 8 | pull-up (strap) | | 9 | BOOT pad (strap) |

The analog inputs are on ADC1, which works alongside Wi-Fi. No output sits on a strapping pin (GPIO 4, 5, 8, 9, 15). `controller.py` checks this map against the firmware on every run.

## Layout

**Board:** 37.3 × 27 mm, 2 layers, 1 oz, every part on the top side.

**Connectors** sit on the edge nearest the pins they serve:
- top: 5 V in;
- right: heater and pump;
- bottom: thermistor, USB-C, valve;
- left: pressure and switches.

All seven are JST GH (1.25 mm, positive latch, which suits a machine shaken by a vibration pump) and side entry, so the cables leave along the board edges.

**Placement:** by hand, judged by eye between routing runs: `check_place.py` draws the ratsnest and courtyard overlaps; it moves nothing.

**Routing:** end to end by **fastroute 0.1.14**. KiCad adds the ground pours on both layers afterwards, plus GND stitching vias on a 1.2 mm grid wherever both layers are free. The current board has 0 unconnected, 49 routed vias and 148 stitching vias. Every ground pad of the power parts and the module sits in one top-layer pour. All vias are tented: the Gerbers have no mask openings on them.

**Rules:** JLCPCB standard capabilities only, the cheapest tier:
- 0.2 mm tracks and gaps (0.15 mm DRC minimum; JLC's is 0.127 mm);
- 0.3 mm drills and 0.6 mm vias;
- 0.3 mm copper to the board edge.

**KiCad DRC:** 0 violations, 0 unconnected. The report still lists library-table notes, because the footprints are embedded and no KiCad library table exists on the build machine.

**Silkscreen:** labels are on the **bottom**, which carries no parts:
- a word behind every connector pin;
- each connector's name under its body;
- the logo, the title and BOOT.

The top is too dense for text and keeps only the parts' own outlines and pin-1 marks.

**JLC:** 20 BOM lines; 4 extended part types (GH connector, USB-C, ESP32-C6 module, LDO), the rest basic. Two gates check the files:
- `jlc_cpl.py` solves every part's position and rotation against JLC's own footprints; all 47 placed parts land on their pads.
- `jlc_pins.py` then compares JLC's pin names with the schematic's for every polarised part: 94 pin functions, 0 mismatches. This gate exists because the v15 position file had the status LED reversed.

## Simulation (ngspice, `sim/run.py`)
60 checks, all passing:
- each output at its worst corners with each module type;
- outputs off during reset against a weak internal pull-up;
- relay flyback and SSR turn-off spikes;
- reverse polarity;
- the 3.3 V rail under 500 mA Wi-Fi bursts from 4.65–5.0 V (≥ 3.29 V);
- the pressure filter and ADC kick-back;
- thermistor range, sensitivity and self-heating;
- the 1 % resistor error budget;
- note: the module loads are behavioural (an SSR input as a current limit, the relay module as 131 Ω + 50 mH), so the on-state checks confirm the MOSFET's drop and the +5 V corner rather than model the modules' internals;
- a 5 V plug in the thermistor socket (1.2 mA into GPIO1 through R27).

The models are behavioural and fitted to datasheet values, so the point is the margins at the corners.

## Status and what needs a human
- **D2 orientation (verified):** its rotation came from the same-package 1N4148W record while EasyEDA's API refused this machine; JLC's own footprint and symbol drawings confirm pad 1 on the same side and pin 1 = cathode for both.
- **Pump SSR on the real pump:** on the first install, count strokes and listen for hum while it's off. This is the one module behaviour not verified against the Ulka's half-wave diode load.
- **Valve coil:** read its label to confirm the current (expected 80–150 mA; the relay's contacts have huge margin).
- **Thermistor:** the firmware default B is 4267 (the Semitec 104NT probe in BUY.md); a different probe needs its B set. The two-point calibration removes the rest.
- **First article:** bench bring-up on USB (INSTALL.md §3) before anything touches the machine.
