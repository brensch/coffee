# Controller board v2

One board drives the machine. It holds:
- the ESP32-S3;
- the 5 V supply;
- zero-cross sensing;
- three zero-cross triacs for the heater, pump and 3-way valve;
- the thermocouple and pressure front ends;
- switch inputs.

Mains lives on the left half, low voltage (SELV) on the right. They meet only inside certified
packages (three optocouplers and the AC-DC module), across a 7 mm copper-free band with milled
slots.

Everything is code:

| File | What |
|---|---|
| [`hardware/controller/controller.py`](../hardware/controller/controller.py) | schematic (SKiDL). ERC plus design checks, then netlist, BOM and `parts-<V>v.json` |
| [`hardware/controller/layout.py`](../hardware/controller/layout.py) | PCB (KiCad 7 pcbnew). Placement, outline, slots, keep-outs, pours, hand-routed heater path; Freerouting for the rest; DRC |
| `hardware/controller/build/` | `coffee-controller-<V>v.kicad_pcb` (open in KiCad), `-drc.rpt`, `bom-<V>v.csv`, gerbers/, renders |

```sh
cd hardware/controller
.venv/bin/python controller.py            # MAINS_V=120 for the 120 V build
source ~/opt/kienv.sh && python3 layout.py # --mains 120
```

Design checks run on every generation, and any failure stops the build:
1. **Isolation:** every net is entirely mains or entirely SELV. Only the opto and PSU packages
   cross.
2. **Firmware pin map:** the GPIOs match `firmware/esp32/src/main.rs` (esp32s3 feature).
3. **Boot-safe:** every load driver has a gate pull-down, so nothing switches while the ESP32
   boots, resets or flashes.
4. **Strapping pins:** S3 GPIO3/45/46 stay unconnected; GPIO0 has its pull-up and the BOOT button.

## Why switch on board (v1 used off-board SSRs)
The v1 plan kept every mains part in a certified module. v2 brings switching onto the board:
- **No relay modules to buy.**
- **No flying mains leads.**
- **The heater triac takes the brew thermostat's place.** Its two spade wires plug straight into
  J2. The **stock steam thermostat and thermal fuse stay in series**, so a shorted triac still
  can't run the boiler past the steam thermostat. There's also no new heater wiring, and
  software can't remove the hardware cut-outs.
- **The pump and valve get switched live** from J3. That live used to come from the brew switch.

What that costs: mains on a board we designed. Every safety-relevant choice is below, and the
board needs a human review before it is ordered.

## Blocks

```
 MAINS (left)                                          │ band │  SELV (right)
 J1 L ─┬─ F1 T500mA ─┬─ PS1 HLK-5M05 (AC) ═════════════╪══════╪═ PS1 (DC) ─ D1 ─ +5V ─ AMS1117 ─ +3V3
       │             ├─ RV1 MOV ─ N                    │      │            USB-C VBUS ─ D2 ─┘
       │             └─ R1 47k ─ U1 H11AA1 ─ R2 ─ N ═══╪══════╪═ ZC ─ GPIO4
       ├─ Q2 BTA08 ── J3.1 pump    ◀ U3 MOC3063 ═══════╪══════╪═ Q5 ◀ GPIO6
       └─ Q3 Z0109 ── J3.2 valve   ◀ U4 MOC3063 ═══════╪══════╪═ Q6 ◀ GPIO7
 J2 (brew-thermostat wires) ─ Q1 BTA16 ─ J2  ◀ U2 MOC3063 ═╪══╪═ Q4 ◀ GPIO5
                                                       │      │  MAX31855 ◀ J5 thermocouple   SPI 12/13/10
                                                       │      │  ADS1115  ◀ J6 pressure, +5V/⅔, J7 NTC   I²C 8/9
                                                       │      │  J8 brew/steam (dry contacts)  GPIO15/16
```

### Mains side
- **Heater: Q1 BTA16-600BW** (16 A, insulated tab, snubberless).
  - Load: 6 A at 230 V; 11.4 A at 120 V.
  - Dissipation: about 1.1 V × I, so **7 W at 230 V and 12 W at 120 V**. The tab sits at the
    board's left edge and bolts to the chassis, which is the heatsink.
  - Resistive load, so no snubber.
  - The copper is hand-routed **2.5 mm on both layers in parallel, 2 oz**, and kept under 25 mm
    long. That's about 5 mm of 2 oz copper: IPC-2221 gives ~12 A at a 20 °C rise.
- **Pump: Q2 BTA08-600CW.** The Ulka pump is inductive with an internal diode: about 0.2 A at
  230 V, 0.4 A at 120 V. Snubber R8 100 Ω + C21 10 nF X2.
- **Valve: Q3 Z0109MN** (sensitive gate, SOT-223). The valve draws ~35 mA, well above the triac's
  holding current. Snubber R11 + C22.
- **Triac drive, all three channels:** onsemi AN-3004 inductive-load circuit.
  - **MOC3063** zero-cross opto-triac (fires only within ±20 V of a crossing).
  - 360 Ω from MT2 to the opto, 330 Ω gate–MT1.
  - This matches the engine exactly: whole half-cycles, applied ~1.5 ms before each crossing
    (`OUTPUT_LEAD_US`).
- **Supply branch:**
  - F1 T500mA (TR5), then RV1 MOV (10D561K at 230 V, 10D271K at 120 V).
  - Then the **HLK-5M05**: certified, 3 kVAC isolation, 5 W.
  - F1 protects only this branch. The loads are protected by the machine's own fuse and
    thermal fuses.
- **Zero cross:** H11AA1, fed through two 47 kΩ 2512 resistors (22 kΩ at 120 V).
  - LED peak current 3.5 mA; 0.28 W per resistor.
  - With the 47k pull-up on the output, the pulse is ~0.9 ms wide, centred on the crossing.

### Isolation
- The **band** is 7 mm of no copper on either layer, the full height of the board. That's more
  than the 5 mm reinforced creepage for 250 V, pollution degree 2, material group III
  (IEC 60664-1).
- **Two milled slots** run down the band, 2 mm wide: one under the PSU, one under the optos.
- **Every part that crosses the band:**

  | Part | Rated isolation |
  |---|---|
  | MOC3063 (×3) | 7.5 kV |
  | H11AA1 | 7.5 kV |
  | HLK-5M05 | 3 kV |

  The optos are wide-lead (0.4") DIP-6 parts, so pad-to-pad across the slot is about 8.6 mm.
- **Between mains nets** (functional insulation, e.g. L vs a switched output across an off
  triac): 1.0 mm minimum. **Conformal-coat the mains half.** The exceptions are pins inside one
  part, such as TO-220 and DIP pitches, which the part itself is rated for.
- **Mounting holes are unplated.** The one on the mains side (H1) has an 8.5 mm copper keep-out,
  because the chassis standoff is earthed.
- The **mains half gets a printed cover**. Silkscreen marks the band, "MAINS" and "SELV".

### SELV side
- **ESP32-S3-WROOM-1-N8.** The antenna sits at the top edge, with a copper keep-out under it.
  - USB-C is for flashing only, on native USB (GPIO19/20) behind a USBLC6.
  - BOOT is GPIO0 with a pull-up; RESET has an RC delay on EN.
- **Power:**
  - PSU 5 V through D1, or USB VBUS through D2 for bench work without mains (OR-ed, so neither
    can backfeed the other).
  - AMS1117-3.3 in SOT-223 on a copper pour: ~0.17 W average, ~0.5 W at Wi-Fi peaks.
  - 5 V budget: about 0.2 A average, 0.45 A peak with three drivers on. The HLK-5M05 gives 1 A.
- **Triac drivers:** GPIO → 100 Ω → AO3400 gate, with **100 kΩ pull-down** (off at boot).
  - The MOSFET switches the opto LED **in series with a green indicator LED** from 5 V through
    150 Ω: (5 − 1.2 − 2.0) V / 150 Ω ≈ 12 mA, against the MOC3063's 5 mA maximum IFT.
  - Because one current path drives both, a lit LED proves the opto LED is driven.
- **Thermocouple:** MAX31855K at the far right, away from the PSU, LDO and triacs, because its
  cold-junction sensor is the board itself.
  - 10 nF differential filter.
  - Use an **ungrounded** probe only: the boiler is earthed.
- **ADS1115** (0x48, 860 SPS):
  - AIN0: pressure through a 10k/20k 0.1% divider (4.5 V → 3.0 V), 240 Hz RC, PESD5V0 at the
    connector. Resolution ≈ 0.75 mbar/count.
  - AIN1: +5V through the same divider, for ratiometric correction of the transducer.
  - AIN2: optional 100k group NTC against a 10k reference.
  - AIN3: spare, tied low.
- **Switches:** brew and steam as dry contacts. Each has a 10k pull-up, 1k series, 100 nF and a
  PESD5V0.
- **Expansion:** an I²C JST-PH-4 in Qwiic order, for a display or scale bridge.

### Pin map (ESP32-S3)
| Signal | GPIO | | Signal | GPIO |
|---|---|---|---|---|
| Zero cross | 4 | | MAX31855 SCK / SO / CS | 12 / 13 / 10 |
| Heater / pump / valve | 5 / 6 / 7 | | ADS1115 SDA / SCL / RDY | 8 / 9 / 11 |
| Brew / steam switch | 15 / 16 | | USB D− / D+ | 19 / 20 |
| Status LED | 38 | | BOOT | 0 |

## Layout
- **Board:** 99 × 71 mm, 2 layers, 2 oz copper.
- **Mains side, left: horizontal lanes**, top to bottom:
  - supply: mains-in terminal → fuse → MOV → PSU;
  - zero cross;
  - valve;
  - pump;
  - heater.

  Each lane runs from its opto on the band, through its resistors and snubber, to its triac and
  its terminal on the left edge, so every mains connection is short and local.
  - The heater triac sits in the bottom-left corner. Its tab is at the board edge, ready to bolt
    to the chassis.
  - Its 2.5 mm double-layer copper runs to J2 on the bottom edge.
- **SELV side, right:**
  - ESP32 top right, antenna at the edge;
  - triac drivers lined up against their optos;
  - analog front end and JST connectors along the bottom;
  - thermocouple at the far right, away from the heat sources;
  - GND pours on both layers.
- **Heater path:**
  - copper pours, not tracks, on both layers;
  - where it leaves the triac's middle pin it is ~2 mm wide (about the pad width, set by the TO-220's 2.54 mm pin pitch), for about 3 mm, then 4.5–6 mm;
  - the heater gate is hand-routed on the bottom layer, clear of the triac pins and the pours.
- **Routing:**
  - The heater path is hand-routed.
  - Freerouting routes everything else at 45°.
  - Mains nets: 0.8 mm tracks, 1.0 mm clearance.
  - SELV power: 0.6 mm.
  - SELV signals: 0.25 mm.
  - No track or pour may enter the band.

## Status and what needs a human
- Generated, autorouted and DRC-checked by code. **Not yet reviewed by a person and not yet built.**
- **Before ordering:**
  - Open it in KiCad and walk the mains side.
  - Check that the Faston pigtail plan suits your harness.
  - Check the MKDS terminal and TO-220 tab position against the side panel.
  - Confirm footprints for the parts you actually buy: the X2 cap outline is 13 × 4 mm, P10, and
    the MOC3063/H11AA1 "T"/"V" lead options are 0.4".
- **Firmware TODOs this board enables:**
  - AIN1 ratiometric pressure correction
  - AIN2 group NTC
  - status LED (GPIO38)
  - ADC data-ready interrupt (GPIO11)
  - Bluetooth scale driver
