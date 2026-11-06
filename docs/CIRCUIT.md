# Controller board v33: mains for the small loads on board

v33 is v32 plus one part, R54, the pump bleeder that the independent review found missing ([REVIEW-v32.md](REVIEW-v32.md), M1).

A 98 × 55.5 mm two-layer board in two halves:
- **Mains half (left):**
  - four 6.3 mm spade tabs, N L P 3 (Gaggiuino's order);
  - a 2 A slow-blow fuse;
  - a Mean Well **IRM-10-5** isolated 5 V supply;
  - two zero-cross opto-triac channels that switch the **vibration pump** and the **3-way valve**.
- **SELV half (right):**
  - the ESP32-C6 module and the thermocouple and pressure front ends;
  - five JST GH sockets along the top edge, plus USB-C.
- **Between the halves:** a 7 mm copper-free band with milled slots. Only the two opto-triacs and the IRM module cross it.

The **heater (1370 W, 11.4 A) stays off the board**, on its external SSR driven from J2. The stock steam
thermostat (145 °C) and thermal fuse (184 °C) stay in series with it. They are the over-temperature
protection that doesn't depend on any of this.

Parts and links: [BUY.md](BUY.md). Wiring: [WIRING.md](WIRING.md). Mis-plug safety: [SWAP-SAFETY.md](SWAP-SAFETY.md).

Schematic as code: `hardware/controller/controller.py` (SKiDL). Simulations: `hardware/controller/sim/run.py`
→ [sim/REPORT.md](../hardware/controller/sim/REPORT.md), 79/79 pass. Every part was checked against its
maker's datasheet (pin map, package, and each value the design relies on); the quotes and links are in
[`evidence/parts.jsonl`](evidence/parts.jsonl).

## Why some mains came back onto the board

v15–v31 carried no mains: certified SSR boards switched the pump and valve, and a separate 5 V module
powered the board. That cost two SSR boards, a module, a hand-made valve snubber and six extra mains
leads, all screwed together loose inside the case. Gaggiuino's boards show that the small loads can sit
on the controller safely if the isolation is designed properly. v32 does that, with three differences
from Gaggiuino's board:
- **The heater stays external.** At 11 A it needs heavy copper and a heatsink, and a failure there is the
  dangerous one. A factory SSR on the rear panel handles it.
- **The snubbers use rated parts.** The capacitors are Class X2 (Murata GA355, 250 VAC); Gaggiuino uses an
  unrated MLCC there. The resistors are 1 W 2512s.
- **The valve gets a low-holding-current triac** (T410, IH 15 mA). When the triac drops out, the coil is
  still carrying current, and it dumps that into the snubber: V ≈ IH·√(L/C). A T435 on the valve gives
  690 V at 132 V line, over its 600 V rating. The T410 gives 456 V.

## Mains section

| Part | Value | Why |
|---|---|---|
| J12–J15 | TE 63824-1 6.3 mm PCB tabs: **N, L, P (pump), 3 (valve)** | Gaggiuino's order. Fully insulated receptacles on 18 AWG. N feeds only the IRM: the pump and valve neutrals stay on the machine's own neutral wiring |
| F1 | Littelfuse 0443002.DR, 2 A slow-blow Nano2, 250 VAC, 50 A breaking | in the live feed, ahead of everything on the board (IRM, both triacs). Load 0.67 A continuous vs 1.45 A after re-rating at 70 °C; its I²t of 18 A²s rides out the IRM's inrush |
| PS1 | Mean Well IRM-10-5, 5 V 2 A | certified (UL/IEC 62368-1), 4.2 kVac isolation. Its AC and DC pins sit 38.5 mm apart, so it bridges the band by itself. Peak load 0.45 A (Wi-Fi + both opto LEDs + heater SSR input) vs 1.0 A allowed at 70 °C (Mean Well's 50 % derating) |
| U8, U9 | onsemi MOC3083SR2M zero-cross opto-triac | 800 V, 4170 Vrms. The wide-lead SMD package has an 8.03 mm row gap, plus a slot under the body. It fires only within 20 V of a zero crossing, so there's no inrush or EMI from switching mid-cycle |
| R47, R48 | 330 Ω, from +5 V | opto LED current 9.8–11.1 mA at every corner, vs the 5 mA IFT max (2×) and the 50 mA rating. Low-side switched by the AO3400A on the command GPIO |
| R49, R50 | 330 Ω 1206 | opto pin 6 to MT2: holds the opto's turn-on surge under its 1 A peak (onsemi AN-3004) |
| Q5 | ST T435-600B (DPAK), pump | IGT/IH 35 mA, 400 V/µs, snubberless-class. Pump 0.45 A rms, half-wave (its own diode): 86.8 °C junction at 70 °C ambient on its copper (97 °C at a 1.4 A peak allowance) |
| Q6 | ST T410-600B (DPAK), valve | IH 15 mA (see above). 40 V/µs: the snubber holds the turn-off to 23 V/µs. ~0.1 W, 75 °C junction |
| R53 | 1 kΩ 1206, gate to MT1 | dV/dt immunity for the T410's sensitive 10 mA gate (AN-3004: 330 Ω–1 kΩ) |
| R54 | 51 kΩ, Vishay CRCW251251K0JNEG (500 V, 1 W), across the pump triac | **v33.** The pump's series diode lets C15 charge to the line peak and then blocks its discharge: ~170 V DC would sit across the triac, and the zero-cross MOC3083 only fires under its inhibit voltage (12 V typ, 20 V max), so the pump would never stroke again. R54 drains it (τ 1.1 ms). Sized for the tight case, the pump running every cycle: the triac drops out late in the cycle, leaving ~5 ms before the next positive zero crossing. 51k gets it to 1.8 V there (5 V even with 2.5× the assumed coil inductance); 100k left 13 V. 187 V peak (37 % of its rating), 171 mW while the pump is off. The valve needs none (no diode) |
| R51/C15, R52/C16 | 100 Ω 1 W 2512 + 22 nF X2 (Murata GA355) | RC snubber across each triac. Pump turn-off 171–190 V, valve 429–456 V (limit 480 V = 80 % of 600 V); 2.5–2.9 kV without. Off-state leakage ~1 mA: under anything that holds the valve in (~50 mA) or moves the pump |

**No MOV.** The IRM passes a 1 kV L–N surge (EN 61000-4-5) by itself, and the triacs and optos are
600/800 V parts. A worn MOV fails as a short, which the fuse would then have to clear.

**Isolation (verified on the routed board, `isolation_check.py`):**
- **Mains to SELV:** 7.83 mm minimum, edge to edge, on both layers. The limit is 6.0 mm: reinforced
  isolation at 120 V working, with margin. Through-hole pads count on both layers. Measured on the opto
  pins against the GND pour.
- **The band:** x 40–47 mm, with no copper on either layer. A milled slot under each opto cuts the direct
  surface path under its body. It doesn't extend past the pad rows, so the 8.03 mm row gap alone is
  what meets the requirement.
- **Between mains nets:** 2.0 mm (KiCad DRC rules in `coffee-controller-120v.kicad_dru`); 0.5 mm only
  between pads at the same potential.
- **Bridging parts:** only their own certified internal barrier crosses the band. The IRM's DC pins are
  35.5 mm from its AC pins; each opto's rows are 7.75 mm apart (pad to pad).
- **Pours:** the triac tabs (MT2 = AC_L_F) sit on a top-layer AC_L_F copper pour that spreads their heat.
  The GND pour and its stitching vias are confined to the SELV half.

**Thermals:** in a 60–70 °C case, every mains part is inside its rating, with the margins above. The IRM
is the tallest part (21.5 mm) and runs at under half its derated load.

## SELV section

### Power
| Part | Value | Why |
|---|---|---|
| D2 | B5819W Schottky | USB VBUS → +5 V for bench work without mains. With mains present, the IRM (5.0 V) powers the board and D2 keeps it out of the PC |
| U5 | TI TLV1117LV33 (SOT-223) | 3.3 V, ceramic-stable. 1.7 V drop at ~100 mA = 0.17 W, +11 °C. 3.29 V minimum under 500 mA Wi-Fi bursts (simulated) |
| C3, C4, C6, C7 | 10 µF, 100 nF, 22 µF ×2 | LDO output, module pin, and bulk on both rails. All ceramic: nothing to dry out at 70 °C |
| R24/R25, C11 → GPIO3 | 5 V / 3 | +5 V sense: the pressure transducer is ratiometric to +5 V, and the firmware scales it out |

v31's external 5 V input (J1), its reverse-polarity FET (Q4) and the 3.3 V sense (R22/R23/C10) are gone.
The supply is on the board now, and the thermocouple ADC doesn't depend on the 3.3 V rail.

### Controller
| Part | Value | Why |
|---|---|---|
| U4 | ESP32-C6-MINI-1U-H4 | Wi-Fi 6 / BLE, external antenna on W.FL / MHF III (the steel case blocks a PCB antenna) |
| R10, C5 | 10k / 1 µF | EN reset delay |
| R11, R46 | 4.7k | I2C pull-ups on GPIO8/9. They double as the strap pull-ups (both pins must read high at reset) |
| TP1 | pad on SCL | BOOT, recovery only (USB resets into download mode by itself) |
| D1, R12 | status LED on GPIO16 (U0TXD) | flickers with the ROM boot log, then shows the heartbeat |
| J4, R13, R14 | USB-C, 5.1k CC | flashing, logs, bench power |

### Outputs
GPIO → AO3400A, **3.3 kΩ gate pull-down** (holds each output off while the pin floats at reset and boot):
- **Heater (Q1):** low-side on J2.2 (HEATER_OUT). J2.1 is +5 V to the SSR's input +; J2.3–4 are empty.
  J2 is the only 4-pin socket.
- **Pump, valve (Q2, Q3):** each sinks its opto LED's current (above). No lead involved.
- **D6–D8:** board LEDs showing H, P and V, each on its output's gate drive.

### Sensors
| Input | Parts | Notes |
|---|---|---|
| **Thermocouple**, J5 (GH 2-pin: 1 TC+, 2 TC− = GND) | U6 MCP3421 (I2C 0x68, 18-bit, PGA 8: 2 µV/LSB, ~0.05 °C), U7 TMP117 cold junction (0x48, ±0.2 °C) at J5's pads, R43/R44 1k + C12 1 µF input filter, R45 10 MΩ open-probe bias | Gaggiuino's ungrounded M4 K-type probe in the brew-thermostat hole. Its own wires are crimped into the GH plug: any splice to copper elsewhere would be an unmeasured second junction. 170 °C reads 6.15 mV of the ±256 mV range. **Open probe:** the 10M against the ADC's ~281k input gives 90 mV, which the firmware calls a fault (anything over ~15 mV, ~370 °C). With the probe in, the bias costs 0.16 °C, constant, calibrated out. The 2k of input resistors against the ADC's input impedance is a fixed −0.7 % gain error, also corrected in firmware |
| Pressure, J6 (GH 3-pin: SIG / +5V / GND) → GPIO0 | R16 47k / R17 100k, C9 22 nF | Gaggiuino's 0–1.2 MPa transducer (0.5–4.5 V ratiometric): 4.5 V → 3.06 V at the pin. The −3 dB corner is at 257 Hz, so the pump's 60 Hz stroke pulses pass at −0.3 dB: the firmware can see each stroke |
| Front panel, J7 (7-pin) → GPIO14/6/7 in, GPIO21/22/23 out | 1k series + 10k pull-ups; 330 Ω LED resistors; D3–D5 mirror the three lamps on the board | stock rockers as dry contacts; the three neon lamps replaced by 5 V LEDs |
| Scale, J10 (7-pin) → GPIO17 SCK, GPIO4/5 DOUT | R36 10k SCK pull-down | dual HX711 on 3.3 V |

### Pin map (ESP32-C6)
| GPIO | Net | | GPIO | Net |
|---|---|---|---|---|
| 0 | P_ADC (pressure) | | 12 / 13 | USB D− / D+ |
| 3 | V5_SNS | | 18 / 19 / 20 | VALVE / PUMP / HEATER command |
| 6 / 7 / 14 | SW_BREW / SW_STEAM / SW_POWER | | 16 (TXD0) | status LED |
| 8 / 9 | I2C SDA / SCL (straps, pulled up) | | 21 / 22 / 23 | LED power / coffee / steam |
| 17 (RXD0) | SCALE_SCK | | 4 / 5 | SCALE_DOUT1 / DOUT2 |

`controller.py` checks this map against `FIRMWARE_PINS` on every run.

## Layout

- **Board:** 98 × 55.5 mm, two layers, 1 oz, JLC standard rules. Three M3 holes, two in the mains half and
  one in the SELV half (plan.json `H1`–`H3`). It mounts in a printed tray (BUY.md #19).
- **Mains half:**
  - the spades run down the left edge, so their receptacles push on sideways, clear of the board;
  - the fuse sits by L;
  - the IRM spans the bottom middle, across the band;
  - the two triac channels are rows at the top: the triac on its copper, its snubber beside it, and the opto at the band.
- **SELV half:**
  - the GH row runs along the top edge: J2 heater SSR (4), J6 pressure (3), J7 front panel (7), J10 scale (7), with J5 thermocouple (2) on the right edge and U7 beside its pads;
  - the module sits in the middle;
  - the LED row S P C S H P V is along the bottom.
- **Mis-plug safety:** every lead's size is unique except the two 7-pins, and `swap_check.py` proves those
  are harmless in each other's sockets.
- **Routing:** fastroute 0.1.14, end to end: 0 unrouted, 86 routed vias, plus 84 GND stitching vias.
- **KiCad DRC:** clean with the mains rules; only the library-table notes remain.
- **JLC checks:**
  - `jlc_cpl.py`: all 83 placed parts land on JLC's own pads;
  - `jlc_pins.py`: 126 pin functions match JLC's symbols (triac A1/A2 = MT1/MT2, IRM ±V = ±Vo, opto pin 5 SUB = NC).

## Ordering: full assembly vs SMD only

`build/coffee-v33-{gerbers.zip,bom.csv,cpl.csv}` has JLC solder everything, including the IRM and the four
spades (through-hole). `build/coffee-v33-smd-{bom,cpl}.csv` leaves those five parts out, and
`coffee-v33-smd-hand-solder.csv` lists them. `python3 hardware/controller/cost.py` prices both from JLC's
live tiered prices:

| 5 boards | JLC | you | total |
|---|---|---|---|
| Full assembly | ~$184 | – | **~$184** |
| SMD only | ~$146 | IRM + 4 tabs ~$27 | **~$173** + soldering 5 × 12 joints |

The ~$11 difference is two extended-part fees plus JLC's THT setup. Full assembly is recommended: the
IRM's pins carry mains, and a factory joint is worth $2 a board.

## What needs a human
- **First article on USB** (INSTALL.md §3), with no mains connected. Check:
  - the thermocouple reads room temperature within a degree of the TMP117;
  - unplugging the probe raises the probe-open fault;
  - each output's board LED follows its command.
- **First mains power-up through a GFCI.** The board sits in its tray with every receptacle on and
  insulated.
- **Pump on the real pump:** count the strokes (one per enabled mains cycle: R54's job), and listen for hum
  while it's off. The snubber and R54 leak about 1 + 3 mA.
- **Valve coil:** read its label. The design assumes 80–150 mA.
