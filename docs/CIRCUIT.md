# Controller circuit

The controller board, as code: [`hardware/controller/controller.py`](../hardware/controller/controller.py)
(SKiDL). Running it does four things:
- runs an electrical rules check
- checks **isolation** (mains nets may only touch the zero-cross input)
- checks that the **firmware pin map** lands on the right signals
- checks that every SSR gate is held off at boot

Then it writes a KiCad netlist and a BOM:

```sh
python3 -m venv .venv && .venv/bin/pip install -r hardware/controller/requirements.txt
.venv/bin/python hardware/controller/controller.py              # 230 V build
MAINS_V=120 .venv/bin/python hardware/controller/controller.py  # 120 V build
# -> hardware/controller/build/coffee-controller-<V>v.net, bom-<V>v.csv
```

The 230 V and 120 V builds differ only in the two mains resistors of the zero-cross input.

## What gets instrumented

| Quantity | Sensor | Interface | Engine use |
|---|---|---|---|
| Boiler wall temperature | K thermocouple (ungrounded, M4) | MAX31855 → SPI | temperature loop, water observer, faults |
| Pump-side pressure | 0–1.6 MPa ratiometric transducer | ADS1115 AIN0 | pressure loop, flow estimate, OPV/no-water faults |
| 5 V supply | divider | ADS1115 AIN1 | ratiometric correction of pressure (firmware TODO) |
| Group head temperature *(opt)* | 100k NTC ring lug | ADS1115 AIN2 | replaces the portafilter-model guess (TODO) |
| Spare analog *(opt)* | e.g. tank level | ADS1115 AIN3 | – |
| Mains phase / frequency | H11AA1 zero-cross | GPIO4 edge | the tick clock, SSR timing, mains fault |
| Heater current *(opt)* | SCT-013-020 clamp CT | ESP ADC (GPIO0) | proves the heater draws current (SSR stuck on/off, open element) (TODO) |
| Brew / steam switches | front-panel switches as dry contacts | GPIO20/21 | user input |
| Cup weight | Bluetooth scale | radio | weight stop, puck model (driver TODO) |
| **Actuators** | heater / pump / valve SSRs | GPIO5/18/19 → MOSFET → SSR input | – |

## Block diagram

```
 MAINS (machine wiring, unchanged except SSRs)            SELV (≤ 5 V) controller board
 ────────────────────────────────────────────        ┌───────────────────────────────────────────┐
  L/N ─ fuse ─ HLK-5M05 (5 V, isolated) ──── 5V ───────▶ J2 ─ D1 ┐                                 │
                                                     │ USB-C ─ D2 ┴─ +5V ─ AMS1117 ─ +3V3        │
  L/N ─────────────────────────────────── J1 ║slot║ F1 R4 R5 ─▶ H11AA1 ─▶ ZC (GPIO4)               │
                                                     │                                           │
  heater:  L ─ steam stat ─ fuse ─ SSR1 ─ element ─ N  ◀─ J8 ◀─ Q1 ◀─ GPIO5                       │
  pump:    L ─ SSR2(+snubber) ─ pump ─ N               ◀─ J9 ◀─ Q2 ◀─ GPIO18     ESP32-C6-WROOM-1 │
  valve:   L ─ SSR3(+snubber) ─ 3-way valve ─ N        ◀─ J10◀─ Q3 ◀─ GPIO19                      │
                                                     │                                           │
  thermocouple (in brew-thermostat hole) ────────────▶ J4 ─ MAX31855 ─ SPI (6/2/7)                 │
  pressure transducer (pump outlet tee) ─────────────▶ J5 ─ ÷⅔ ─ ADS1115 AIN0 ┐                    │
                                       +5V ─ ÷⅔ ─────────────────────── AIN1 ├ I2C (22/23)        │
  group NTC (opt) ───────────────────────────────────▶ J6 ─ 10k ref ─── AIN2 │                    │
  spare (opt) ───────────────────────────────────────▶ J7 ──────────── AIN3 ┘                    │
  heater CT clamp (opt, around live wire) ───────────▶ J12 ─ bias ─ GPIO0 (ADC)                    │
  brew / steam switches (dry contacts) ──────────────▶ J11 ─ RC + ESD ─ GPIO20/21                  │
                                                     └───────────────────────────────────────────┘
```

## Blocks and calculations

### Power
- **Off-board HLK-5M05** (5 W, 1 A, isolated, certified) with a 0.5 A slow fuse and a 10D561K MOV on its mains side. The earlier plan used the 3 W HLK-PM05: the budget below fits, but margin matters at 60 °C inside the case.
- **OR-ing:** D1 (from the HLK) and D2 (from USB VBUS) feed +5V, so the board can be powered from USB on the bench without backfeeding the PC. The SS14 drops about 0.3 V, giving 4.7 V; the transducer's ratiometric correction (AIN1) absorbs that.
- **3.3 V: AMS1117-3.3 in SOT-223**, not a SOT-23 LDO:

  | | dropout × current | dissipation |
  |---|---|---|
  | Wi-Fi transmit peak | (4.7 − 3.3) V × 0.35 A | ≈ 0.5 W |
  | average | 1.4 V × ~0.12 A | ≈ 0.17 W |

  SOT-223 with a copper pour (about 60–90 °C/W) gives about +15 °C on average. A SOT-23-5 (about 250 °C/W) would run 40 °C+ hotter and hit thermal limit at peaks.
- **Budget (5 V):**
  - ESP32 via LDO: 0.12 A average, 0.35 A peak
  - transducer: ~5 mA
  - 3 SSR inputs: ~45 mA
  - LEDs: ~12 mA
  - **Total: ≈ 0.2 A average, 0.42 A peak.**

### MCU (ESP32-C6-WROOM-1-N8)
- Decoupling 10 µF + 100 nF at pin 2. EN RC (10k / 1 µF) plus a RESET button. A BOOT button on GPIO9 (low at reset = download mode).
- **Native USB** (GPIO12/13 = D−/D+) behind a USBLC6-2SC6, with 5.1k CC pull-downs (USB-C device). That one port does flashing, the `.clog` stream and the HIL bridge.
- **Strapping pins** (datasheet table 4-1):
  - GPIO8 and GPIO15 are left unconnected.
  - GPIO9 has the BOOT button.
  - GPIO4/GPIO5 (MTMS/MTDI) carry zero-cross and the heater drive. Those straps only select an SDIO clock edge, which we don't use, so this is harmless.
- Pins used match `firmware/esp32/src/main.rs`; the build fails if they drift. New on the board: GPIO0 (heater CT), GPIO3 (status LED), GPIO10 (ADS1115 data-ready). Spare: GPIO1 (ADC), GPIO11, UART0 on J13.

### Zero cross: the only mains on the board
```
 L ─ F1 100 mA ─ R4 ─┐            ┌─ C (pin 5) ─┬─ R6 47k ─ +3V3
                     ├ H11AA1 1/2 │             ├─ C8 100 pF ─ GND
 N ───────────── R5 ─┘   (LEDs)   └─ E (pin 4) ─ GND        ZC ─▶ GPIO4
            ║ ≥ 6.4 mm creepage, milled slot under the opto ║
```
- **230 V:** R4 = R5 = 47 kΩ, 1 W, 2512, with working voltage ≥ 250 V.
  - Peak LED current: 325 V / 94 kΩ = **3.5 mA**.
  - Dissipation: 230² / 94k = 0.56 W total, **0.28 W per resistor** (50% derated).
- **120 V:** R4 = R5 = 22 kΩ. Peak 170 V / 44 kΩ = 3.9 mA; 0.16 W each.
- **Pulse width.** The output stays low while the transistor can sink the pull-up current.
  - With 47k (0.07 mA) and a worst-case CTR of about 15%, the LED current must exceed about 0.47 mA. That's |v| > 44 V, so the output is **high for ~0.9 ms, centred on each crossing**.
  - With 10k it would be about 4.4 ms wide. Hence 47k.
- **Timing.** The firmware uses the rising edge, which is about 0.45 ms before the true crossing; `OUTPUT_LEAD_US` absorbs it. Improvement (TODO): timestamp both edges and use the midpoint.
- **H11AA1** in the wide SMD package (9.53 mm row spacing) for creepage. The milled slot runs under the package between pins 1–2 and 4–6. Mains copper keeps ≥ 6.4 mm from everything else.

### Thermocouple (MAX31855K)
- **The probe must be ungrounded (insulated junction).** The boiler is earthed, and the board's ground reaches the PC over USB. A grounded-junction probe shorts T− to ground: the MAX31855 then reports SCG faults, or worse, picks up ground-loop noise.
- 10 nF across T+/T− (datasheet) and 100 nF on VCC. CS has a 10k pull-up so it's deselected during boot.
- Place the MAX31855 **away from the LDO and the SSR connectors**: its internal cold-junction sensor *is* the board temperature. Run T+/T− as a symmetric pair.

### Pressure, 5 V monitor, NTC, spare (ADS1115)
- **ADC setup:** ±4.096 V range, 860 SPS, address 0x48, I2C pull-ups 4.7k, ALERT/RDY to GPIO10.
- **AIN0, pressure:** 10k / 20k (0.1%) divider, so 4.5 V becomes 3.0 V. That's below VDD + 0.3 V even at full scale (3.3 V is never exceeded), with a PESD5V0 on the connector side.
  - Resolution: 125 µV LSB over 2.67 V for 16 bar ≈ **0.75 mbar** per count.
  - 100 nF filter: fc ≈ 240 Hz. That keeps the pump's 50 Hz stroke ripple, which the engine uses, and kills RF.
- **AIN1, 5 V monitor:** the same divider on +5V. The transducer output is *ratiometric*: pressure = (V0 / V5 − 0.1) / 0.8 × 16 bar. Measuring V5 removes the ±5% supply error and the OR-diode drop.
- **AIN2, group NTC (optional):** 10k 0.1% reference to 3.3 V. A 100k B3950 NTC gives ~1.4 V at 90 °C and ~3.0 V at 25 °C.
- **AIN3, spare** analog input (1k + 100 nF), with 3.3 V on the connector.

### SSR drivers (×3)
```
 +5V ─┬─────────────── J8.1 (SSR in +)
      └─ R 1k ─ LED ─┐
 GPIO ─ R 100 Ω ─┬── Q AO3400A gate        drain ─ J8.2 (SSR in −)
                 └─ R 100k ─ GND           source ─ GND
```
- **SSRs:**
  - Heater: Crydom D2425 / Omron G3NA-225B (input 3–32 V).
  - Pump and valve: G3MB-202P or Crydom CX240D5, with a 100 Ω + 100 nF X2 snubber across each output.
  - All **zero-cross types**. The engine only switches whole half-cycles, and the firmware applies outputs about 1.5 ms before the next crossing.
- The **100k gate pull-down holds all loads off** while the ESP32 boots, resets or is being flashed (design check #3).
- AO3400A: Vgs(th) ≤ 1.45 V, comfortably on at 3.3 V for a ~15 mA SSR input. The LEDs mirror each output.

### Switches
- Brew and steam rewired as **dry contacts** to J11, which is how GaggiMate does it. The switch lamps go dark.
- Each input: 10k pull-up, 1k series, 100 nF (0.1 ms), PESD5V0 at the connector. The leads run near mains wiring inside the case.

### Heater current (optional)
- **SCT-013-020** clamp CT (20 A gives 1 V, burden built in) around the heater *live* wire. It's non-invasive and inherently isolated.
- Biased at 1.65 V (10k/10k + 10 µF) into GPIO0 (ADC1_CH0) through 1k / 1 nF.
- The ESP samples it to confirm that "heater commanded" matches "current flowing". That catches the SSR stuck-on/off and open-element faults directly, rather than inferring them from temperature after 60 s.

## Layout rules
- 2-layer, about 60 × 80 mm, with a solid ground pour on the bottom.
- **Isolation:** the J1/F1/R4/R5/U5-pins-1-2 island is separated by a ≥ 6.4 mm milled slot plus clearance, and labelled MAINS. Keep it at one board edge.
- MAX31855 at the opposite edge from the LDO and J8–J10. ESP32 antenna at a board edge with the keep-out from the datasheet (no copper under it).
- AMS1117 tab on a ≥ 2 cm² copper pour.
- **Conformal coat optional; enclosure required:** PETG/ASA, the case reaches 60–70 °C (HARDWARE.md).

## Board BOM (generated)
The exact list with references, values, footprints and populate flags is
`hardware/controller/build/bom-230v.csv` (and `-120v`): 86 parts, 53 line items. Key ICs:

| Ref | Part | MPN |
|---|---|---|
| U2 | ESP32-C6 module, 8 MB | ESP32-C6-WROOM-1-N8 |
| U3 | thermocouple converter | MAX31855KASA+T |
| U4 | 16-bit ADC | ADS1115IDGSR |
| U5 | AC-input optocoupler | onsemi H11AA1SR2M |
| U1 | 3.3 V LDO (SOT-223) | AMS1117-3.3 |
| U6 | USB ESD | USBLC6-2SC6 |
| Q1–Q3 | SSR drivers | AO3400A |
| J3 | USB-C | GCT USB4105-GF-A |

Footprints are standard KiCad library names. Check `RF_Module:ESP32-C6-WROOM-1` exists in your KiCad version; older libraries may need Espressif's footprint.

## Off-board BOM (both v1 modules and v2 board)

| Item | Part | Qty |
|---|---|---|
| 5 V supply | Hi-Link HLK-5M05 + 0.5 A slow fuse + 10D561K MOV | 1 |
| Heater SSR | Crydom D2425 or Omron G3NA-225B, bolted to the chassis | 1 |
| Pump / valve SSR | Omron G3MB-202P or Crydom CX240D5, plus 100 Ω / 100 nF X2 snubber each | 2 |
| Thermocouple | K-type, **ungrounded junction**, M4 thread, glass-fibre lead | 1 |
| Pressure transducer | 0–1.6 MPa, 5 V, 0.5–4.5 V, **G1/4 male** | 1 |
| Plumbing | GaggiMate plumbing kit (G1/8 elbow + push tee, 0406 PTFE tube, G1/4 transducer fitting) | 1 |
| Group NTC *(opt)* | 100k B3950 ring-lug NTC | 1 |
| Heater CT *(opt)* | YHDC SCT-013-020 | 1 |
| Scale | Bluetooth (Decent / Bookoo / Acaia) | 1 |
| Wiring | 18 AWG silicone (mains), 24 AWG signal, Faston 6.3 mm, ferrules, fibreglass sleeve | – |
| Enclosure | PETG/ASA print (OpenSCAD, after layout) | 1 |

## Bring-up (board, before the machine)
1. **USB only, no mains.** 3.3 V present; the board enumerates as a USB serial/JTAG device. Flash the bridge build and run `coffee-emu serve --engine-serial …`, so the real board runs against the emulated machine.
2. **Sensors on the bench:**
   - thermocouple ~room temperature, pinched ~35 °C
   - transducer 0 bar, AIN1 ≈ 3.13 V on USB power
   - NTC ~3.0 V at room temperature
   - switch inputs toggle
3. **SSR outputs into LEDs/loads off-mains.** All off during reset and flashing (watch the LEDs while holding RESET).
4. **Zero cross, mains in a closed box:** a scope on ZC shows ~0.9 ms pulses every 10.0 ms (50 Hz) or 8.33 ms (60 Hz). The firmware reports the half-period.
5. Then follow INSTALL.md §3.

## Firmware TODOs this board enables
- AIN1 ratiometric pressure correction
- AIN2 group NTC feeding the portafilter model
- GPIO0 heater-current fault check
- GPIO3 status LED
- ZC midpoint timing
- the Bluetooth scale driver

Each needs a matching emulator sensor model, so the work stays emulation-first.
