# Hardware

Target: **Gaggia Classic Pro E24** (brass boiler). For why this machine, see the
README. This page is the bill of materials, the audit behind each choice, the
wiring, and the PCB and enclosure plan. To install it, see [INSTALL.md](INSTALL.md).
To measure the machine for the models, see [CALIBRATION.md](CALIBRATION.md).

## Design rules (from the audit)

1. **No mains on anything we design.** Every mains-side part is a certified,
   off-the-shelf module (SSRs, AC-DC supply). Our electronics, and the PCB we
   design, are SELV (≤5 V) only. That leaves one exception, zero-cross sensing;
   see below.
2. **Hardware protection stays in series with the heater.** Keep the stock
   steam thermostat (or fit a 165–170 °C manual-reset limiter) and the thermal
   fuse. Software can only *ask* a failed-shorted SSR to stop (scenario 12).
3. **Whole half-cycle switching only.** The engine never does phase-angle
   control. So zero-cross SSRs drive all three loads, including the pump. No
   triac dimmer module is needed for the pump (Gaggiuino needs one because it
   does phase control).
4. **Outputs are applied just before the next zero crossing** (firmware
   `OUTPUT_LEAD_US`). This is because a zero-cross SSR can only turn on near 0 V.
   The emulator models the resulting one-half-cycle latency.
5. **Measure what the model needs.** Each sensor below maps to model
   parameters. Anything we can't sense is estimated, and the UI says so.

## Bill of materials (v1: modules, no custom PCB)

Prices are rough (USD, 2026) for planning.

| # | Part | Qty | ~$ | Why / audit notes |
|---|---|---|---|---|
| 1 | **ESP32-C6-DevKitC-1-N8** | 1 | 10 | Stable Rust (`esp-hal`), Wi-Fi 6/BLE (scale, UI). Native USB for flashing, the binary log and the HIL bridge. |
| 2 | **MAX31855K breakout** (Adafruit 269 or clone) | 1 | 15 | K-type with cold-junction compensation and open/short fault bits. The engine decodes the raw word, fault bits included. |
| 3 | **K-type thermocouple, M4 thread**, ≥1 m, glass-fibre or PTFE lead | 1 | 8 | Goes in the brew-thermostat hole. **Hand-tight**: the M4 stud snaps. Measures the boiler *wall* (the engine estimates the water). |
| 4 | **Pressure transducer 0–1.6 MPa, 5 V, 0.5–4.5 V**, G1/8 | 1 | 20 | 16 bar range covers the pump's ~15 bar dead-head. Prefer a **GCP T-fitting kit** (tee, PTFE line, olives) over improvised plumbing. |
| 5 | **ADS1115 breakout** | 1 | 6 | 16-bit ADC (the ESP's ADC is too non-linear). AIN0 = pressure through a 2/3 divider (10k/20k). **Audit:** the transducer is *ratiometric* to its 5 V supply, so wire **AIN1 = 5 V rail through a second 2/3 divider** for supply correction. Wiring is ready; firmware support is a TODO. |
| 6 | **Heater SSR, zero-cross, 25 A**, genuine (Crydom D2425 / Omron G3NA-225B) + bolted to the chassis or a heatsink | 1 | 25–40 | Element is ~6 A at 230 V or ~10 A at 120 V. Counterfeit "Fotek" SSRs are a known fire risk. **Audit:** input must work at 3.3–5 V: D2425 is 3–32 V. |
| 7 | **Pump + solenoid SSRs, zero-cross, ≥2 A** (Omron G3MB-202P modules or Crydom CX240D5) | 2 | 6 | The pump is ~0.2 A but inductive with a diode, so **add an RC snubber (100 Ω + 100 nF X2)** across each output. |
| 8 | **Zero-cross detector**: H11AA1 or LTV-814 opto + 2× 47 kΩ 0.5 W (mains side), or a RobotDyn dimmer module used *only* for its Z-C pin | 1 | 3–8 | The one mains-touching part we can't buy certified and cheap. The RobotDyn module works but its isolation is unverified. On the PCB it gets a routed isolation slot (≥6.4 mm). |
| 9 | **HLK-PM05** (5 V 3 W AC-DC) + 0.5 A slow fuse + 10D561K MOV | 1 | 6 | ESP + sensors draw ~150 mA. Fuse and MOV on its mains input. |
| 10 | **PC817 optocouplers** or direct contacts (see switches) | 2 | 1 | Brew and steam switches become logic inputs (GaggiMate does the same; the switch lamps go dark). |
| 11 | **Logic-level MOSFETs (AO3400) + 1k/10k resistors** | 3 | 1 | Drive the SSR inputs from 5 V, so SSRs see a solid on-voltage rather than marginal 3.3 V. |
| 12 | High-temperature wire (silicone 18 AWG for mains, 22–26 AWG signal), ferrules, Faston 6.3 mm crimps, heat-shrink, fibreglass sleeve | – | 20 | It's hot inside the case (60–70 °C near the boiler). No PVC near the boiler. |
| 13 | **Bluetooth scale** with a known protocol (Decent Scale, Bookoo, Acaia) | 1 | 60–200 | Weight stop and puck model. **Audit:** the firmware BLE driver isn't written yet. Until then, shots stop on time or profile exits. |
| 14 | Optional: 12 bar OPV spring (US 120 V units) | 1 | 5 | The stock US 9 bar OPV opens gradually and fights pressure control. |
| 15 | Optional: **100k NTC ring-lug thermistor** (3950) under a group screw + 100k reference resistor | 1 | 3 | Measures the group head on ADS1115 **AIN2**, replacing the estimate's guesswork about pre-warming flushes or a portafilter left on the counter. It doesn't see the portafilter itself (removable), which stays estimated and is calibrated once (CALIBRATION.md test 9). Firmware support is a TODO; the PCB reserves the connector. |

**Calibration tools** (see CALIBRATION.md): a plug-in power meter (Kill-A-Watt),
a multimeter, a fast probe thermometer (Thermapen-style), a kitchen scale (0.1 g),
a blind basket, a stopwatch. Optional: a Scace device (borrow one).

### Things the audit removed or changed

- **Triac dimmer for the pump:** dropped. With whole-cycle control, a
  zero-cross SSR does the same job with certified isolation.
- **Driving SSRs straight from 3.3 V GPIO:** replaced with MOSFET drivers at 5 V.
- **An ESP ADC for pressure:** never used. ADS1115 only, plus 5 V ratiometric
  correction.
- **A flow meter:** not needed. Flow is estimated from the pump model, and
  calibrated once with a scale (CALIBRATION.md §Pump). It can be added later
  for validation.
- **A water-level sensor:** skipped for v1. "No water" is detected from pump
  duty against pressure (scenario 13).

## Wiring (ESP32-C6)

| Signal | GPIO | Goes to |
|---|---|---|
| Zero-cross in | 4 | ZC opto output (pull-down), one rising edge per crossing |
| Heater SSR | 5 | MOSFET → heater SSR input |
| Pump SSR | 18 | MOSFET → pump SSR input |
| Solenoid SSR | 19 | MOSFET → solenoid SSR input |
| Brew switch | 20 | switch to GND (pull-up), via PC817 if long leads |
| Steam switch | 21 | switch to GND (pull-up) |
| MAX31855 SCK / MISO / CS | 6 / 2 / 7 | thermocouple amp |
| ADS1115 SDA / SCL | 22 / 23 | 0x48; AIN0 pressure, AIN1 5 V monitor |

Mains side (unchanged machine wiring except where noted):
- **Heater:** live → (stock steam thermostat / limiter) → (thermal fuse) →
  heater SSR → element → neutral.
- **Pump:** live → pump SSR (+ snubber) → pump (thermal fuse if fitted) → neutral.
- **Solenoid:** live → solenoid SSR (+ snubber) → 3-way valve → neutral.
- **Brew-thermostat terminals:** bridged (the probe replaces it), as in GaggiMate's guide.

## Should we design a PCB? Yes, in two steps

**Step 1: modules on a carrier (now).** Build v1 from the modules above on
perfboard or a breadboard. Bring it up first in **HIL bridge mode**: the real
ESP32 runs against the emulator, with no mains connected. Then install it. This
proves the pin map, timing and sensors before we freeze a board.

**Step 2: a custom SELV controller board.** Once v1 works:

- ESP32-C6-WROOM-1 module (or a DevKitC socket for easy swapping), USB-C.
- MAX31855 and ADS1115 on board. Thermocouple mini-connector; 3-pin JST for the
  transducer (5 V, signal, GND) with dividers, the 5 V monitor and ESD/TVS
  protection.
- 3× AO3400 SSR drivers with LED indicators to 2-pin screw terminals.
- 2× switch inputs with RC filter and ESD; a spare I2C header (display, scale bridge).
- 2-pin connector to ADS1115 AIN2 for the optional group-head NTC.
- **Isolated zero-cross section**: H11AA1 with ≥6.4 mm creepage and a milled
  slot. This is the only mains on the board, behind a fuse, clearly marked.
- 5 V input from the HLK-PM05 mounted *off-board* by default (an on-board
  footprint behind the same isolation slot is optional).
- 2-layer, ~60 × 80 mm, JLCPCB assembly. About $30–60 for 5 assembled boards.

I can write the board as code (atopile or SKiDL) to produce the KiCad
schematic, netlist and BOM. The layout gets autorouted (freerouting) and then
needs a human review before ordering. That's the next step if you want it.

## Enclosure

- **Where:** inside the Classic's case, on the side or rear panel, as far from
  the boiler as possible. The heater SSR bolts to the metal chassis (it's the
  heatsink) and is **not** in the printed box.
- **Material:** PETG minimum, ASA or PC better. PLA softens at ~55 °C, and the
  case interior reaches 60–70 °C near the boiler.
- **Design:** parametric OpenSCAD around the final board. Standoffs, strain
  relief for the thermocouple and transducer leads, vents, a USB-C cut-out
  reachable from outside. Designed after the board's dimensions are fixed. To
  fit it, I'll need a few measurements of the free space inside your case.
