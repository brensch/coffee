# Controller board v6

One 64.5 × 56.5 mm board drives the machine, with 51 parts. It holds:
- an ESP32-C6 module;
- the 5 V supply;
- three zero-cross triacs for the heater, pump and 3-way valve;
- the thermistor and pressure inputs;
- the brew/steam switch inputs.

Mains lives on the left, low voltage (SELV) on the right. They meet only inside certified
packages (three opto-triacs and the AC-DC module), across a 7 mm copper-free band with milled
slots.

The rule for v6: **anything the firmware can do, the firmware does.** The board keeps only what
has to be physics:
- isolation;
- switching mains;
- turning sensors into voltages.

Everything else is Rust (`firmware/esp32`), and the engine above it is tested on the emulator.

Everything is code:

| File | What |
|---|---|
| [`hardware/controller/controller.py`](../hardware/controller/controller.py) | schematic (SKiDL). ERC plus design checks, then netlist, BOM and `parts-<V>v.json` |
| [`hardware/controller/layout.py`](../hardware/controller/layout.py) | PCB (KiCad 7 pcbnew). Placement, outline, slots, keep-outs, heater pours, hand-routed mains traces; Freerouting for the rest; DRC with the mains lint |
| [`hardware/controller/sim/run.py`](../hardware/controller/sim/run.py) | ngspice checks with tolerance corners, written to [`sim/REPORT.md`](../hardware/controller/sim/REPORT.md) |
| `hardware/controller/build/` | `coffee-controller-120v.kicad_pcb` (open in KiCad), `-drc.rpt`, `bom-120v.csv`, gerbers, renders |

```sh
cd hardware/controller
.venv/bin/python controller.py                 # schematic + checks (120 V is the default)
source ~/opt/kienv.sh && python3 layout.py     # place, route, DRC, fab files
python3 sim/run.py                             # simulations
```

Design checks run on every generation, and any failure stops the build:
1. **Isolation:** every net is entirely mains or entirely SELV. Only the opto and PSU packages
   cross.
2. **Firmware pin map:** the GPIOs match `firmware/esp32/src/main.rs`.
3. **Analog on ADC1:** the thermistor and pressure inputs sit on ADC1 pins, which work alongside
   Wi-Fi.
4. **Strapping pins:** no load is driven from a strapping pin (GPIO4/5/8/9/15). GPIO8 has its
   pull-up and GPIO9 goes to the BOOT pad; the rest are left unconnected.

Layout adds two more:
- the KiCad DRC with the mains spacing rules (below);
- a check that no foreign pad sits inside a heater keep-out (v5's routing failure).

## What changed from v5, and why

| Removed | Why it can go |
|---|---|
| Zero-cross detector (H11AA1, 2 × 22k, 100 pF) | The firmware ticks once per mains cycle from the crystal. The opto-triacs only ever switch at a zero crossing anyway, so the board doesn't need to know where the crossing is. One 60 Hz cycle always contains exactly one positive half-wave, so the pump's stroke count stays exact whatever the phase. Grid drift (~0.03 %) doesn't matter. |
| Thermocouple amplifier (MAX31855) | A 100k NTC thermistor plus one 0.1 % resistor, read by the C6's own ADC. Steinhart–Hart and a two-point calibration (room temperature + boiling water) live in firmware. |
| External ADC (ADS1115) and 5 V monitor | The pressure transducer goes straight into ADC1 through a 2/3 divider. Ratiometric drift is handled by calibration and a careful self-zero (an idle boiler holds 2–4 bar, so the firmware can't assume 0 at rest). |
| 100k pull-downs on the drive lines | The opto LED needs 5 mA to fire. A floating or weakly pulled pin can't supply that. |
| Gate resistors on the heater and pump triacs | Only sensitive-gate triacs need one (onsemi AN-3004). The Z0109 on the valve keeps its 330 Ω. |
| Switch RC filters and series resistors | Debounced in firmware; the switches are dry contacts on internal pull-ups. |
| Second 5 V bulk capacitor, OR-ing diode on the PSU | One 22 µF is enough (simulated). |
| ESP32-S3-WROOM-1U | ESP32-C6-MINI-1U-H4: 105 °C rated, half the size, U.FL antenna, in stock at JLCPCB. Single core is plenty for one control loop per mains cycle. |

| Changed | Why |
|---|---|
| 800 V opto-triacs and triacs (MOC3083, BTA24-800BW, BTA08-800CW, Z0109NN) | Simulation showed the valve's triac sees ~2× the line peak when it turns off: up to 410 V at 132 V mains. A coil's current lags its voltage, so the current reaches zero (where the triac lets go) near the voltage peak, and the snubber then rings. 600 V parts left too little margin; the opto-triac sees the same voltage. 800 V parts also ride out surges that the MOV clamps at ~450–700 V. They cost the same. |
| Snubber capacitors: through-hole film → one surface-mount X2 part (Murata GA355QR7GB103, 10 nF, 250 VAC, 2220) for both | They're a third of the size, have no bottom-layer pads, and are fab-assembled. The valve still has margin on 10 nF (≤ 477 V at 132 V even with the ceramic's capacitance derated to 8 nF); without any snubber it reaches ~2 kV. |
| Heater triac BTA16 → BTA24 | 25 A part, lower thermal resistance; runs cooler at 11.4 A. |
| 120 V is the default build | The machine is a US Gaggia Classic Pro. The 230 V variant only changes the MOV. |

## Blocks

```
 MAINS (left)                                         │ band │  SELV (right)
 J1 L ─┬─ F1 T500mA ─┬─ PS1 RECOM RAC03E (AC) ════════╪══════╪═ PS1 (DC) ─ +5V ─ U5 TLV1117LV33 ─ +3V3
       │             └─ RV1 MOV ─ N                   │      │       USB-C VBUS ─ D2 ─┘
       ├─ Q3 Z0109NN ── J3.1 valve ◀ U3 MOC3083 ══════╪══════╪═ R9 150Ω ◀ GPIO18
       └─ Q2 BTA08 ──── J3.2 pump  ◀ U2 MOC3083 ══════╪══════╪═ R8 150Ω ◀ GPIO19
 J2 (brew-thermostat wires) ─ Q1 BTA24 ◀ U1 MOC3083 ══╪══════╪═ R7 150Ω ◀ GPIO20
                                                      │      │  J5 thermistor ─ R15 8.2k to 3V3 ─ GPIO1
                                                      │      │  J6 pressure ─ R16/R17 (⅔) ─ GPIO0
                                                      │      │  J7 brew / steam (dry contacts) ─ GPIO6 / 7
                                                      │      │  D1 status LED ─ GPIO23
```

### Mains side
- **Heater: Q1 BTA24-800BW** (25 A, insulated tab, snubberless; R_th(j-c) 1.7 °C/W).
  - Load: 11.4 A at 120 V (1370 W).
  - Dissipation ≈ V_TO·I_avg + r_d·I_rms² ≈ **11 W**, so ~24 °C above whatever it's bolted to. The tab sits on the board's left edge and bolts to the chassis, which is the heatsink.
  - Why it gets warm at all: every solid-state switch (triac or SSR) drops about 1 V while conducting. A mechanical relay wouldn't, but it can't switch every cycle without wearing out within days.
  - Resistive load, so no snubber.
  - The heater current runs in copper pours on both layers, from the triac to J2 on the bottom edge. They're ~2 mm wide where they leave the triac's middle pin (the TO-220 pin pitch allows no more), then 4.5–6 mm.
  - The board doesn't replace any protection: the **stock steam thermostat and thermal fuse stay in series** with the heater. A triac that fails shorted still can't run the boiler past them.
- **Pump: Q2 BTA08-800CW.** The Ulka pump is inductive, with a diode inside; about 0.4 A at 120 V.
  - Snubber R3 100 Ω + C1 10 nF X2 (surface-mount).
  - Without it, the triac sees ~574 V at turn-off; with it, 170 V.
- **Valve: Q3 Z0109NN** (sensitive gate, SOT-223), with a 330 Ω gate–MT1 resistor R5.
  - Snubber R6 100 Ω + C2 10 nF X2.
- **Triac drive, all three channels:** MOC3083 zero-cross opto-triac (fires only within ±20 V of a crossing), with a 360 Ω resistor from MT2 to the opto (R1, R2, R4) to limit its surge current.
- **Supply branch:**
  - F1 T500mA (TR5), then RV1 MOV 10D271K.
  - Then the **RECOM RAC03E-05SK/277**: 3 W (0.6 A), 4 kVAC reinforced, EN60335-1 household-certified, rated to 85 °C (full load to 75 °C) with a built-in input fuse. It replaced the HLK-PM05, which is only rated to about 60 °C; the case runs at 60–70 °C.
  - F1 protects only this branch. The loads are protected by the machine's own fuse and thermal
    fuses.

### Isolation
- The **band** is 7 mm of no copper on either layer, the full height of the board. That's more
  than the 5 mm reinforced creepage for 250 V, pollution degree 2, material group III
  (IEC 60664-1).
- **Two milled slots** run down the band, 2 mm wide: one under the PSU, one under the optos.
- **Parts that cross the band:** three MOC3083s (5.3 kV RMS isolation, wide 0.4" leads, ~8.6 mm
  pad to pad) and the RECOM RAC03E (4 kV reinforced).
- **Between mains nets** (functional insulation): **2.0 mm minimum, enforced as a design rule**
  (the lint below). IEC 60664-1 asks ~1.6 mm at 120 V, uncoated, pollution degree 2. Exempt:
  - pins inside one part's footprint (the triac and opto pin pitch is the manufacturer's rating);
  - net pairs within a few volts of each other (gate↔MT1, opto↔MT2, either side of the fuse).

  Conformal coating is still recommended.
- **Mounting holes are unplated.** The one on the mains side (H1) has a copper keep-out, because
  the chassis standoff is earthed. The heater triac's tab bolt is the fourth fixing.
- The **mains half gets a printed cover**. Silkscreen marks the band, "MAINS" and "SELV".

### SELV side
- **ESP32-C6-MINI-1U-H4**, rated −40 to 105 °C, so the 60–70 °C inside the case is comfortable.
  - It has a U.FL connector instead of a printed antenna. The Gaggia's case is steel, so run a
    short pigtail to a 2.4 GHz antenna on a plastic part or vent.
  - 4 MB flash: enough for the firmware plus two OTA slots.
- **Programming, logs and JTAG** all go over the one USB-C port, using the C6's built-in USB
  Serial/JTAG (GPIO12/13).
  - `espflash` resets the chip into download mode over USB, so there are no buttons.
  - Recovery, if firmware ever wedges the USB port: short pad **BOOT** (TP1, GPIO9, on the
    underside) to **GND** (TP2) while plugging in. GPIO8 has the 10k pull-up that download mode
    needs.
  - EN has its RC delay.
- **One status LED** (D1, GPIO23) for heartbeat, ready and fault: put it behind a light pipe to the front of the machine. It's optional.
- **Power:**
  - The PSU feeds +5 V directly.
  - USB VBUS joins through D2 for bench work without mains. D2 also stops the PSU back-feeding
    the PC.
  - Bulk capacitors are 22 µF X7R ceramics (125 °C). There are no electrolytics, the part most likely to age at 60–70 °C.
  - TI TLV1117LV33 in SOT-223 on a copper pour. It's stable with ceramic capacitors (the AMS1117 it replaced needs a tantalum output capacitor and can oscillate without one).
- **Triac drive:** GPIO → 150 Ω → opto LED → GND.
  - Simulated 8.2–12.1 mA at every corner, against the MOC3083's 5 mA maximum trigger current.
- **Thermistor (J5):** a 100k B3950 NTC probe to GND, with R15 8.2k 0.1 % to 3.3 V and C8 100 nF, into GPIO1 (ADC1).
  - 3.1 V at 20 °C, 1.68 V at 93 °C (near mid-scale, where it matters), 0.65 V at 150 °C.
  - 24 mV/°C at brew temperature. With 16× oversampling that's about 0.01 °C per count, far finer than the probe's real accuracy.
  - Before calibration, a ±1 % probe is good to about ±1.2 °C. The two-point calibration (room temperature, then boiling water) removes that.
  - Self-heating is 0.33 mW (~0.2 °C in free air, less bolted into brass). It's constant, so the calibration cancels it.
  - The probe body is metal, screwed into the earthed boiler. Use a probe whose sensor is insulated from its body, which is normal for these M4 probes.
- **Pressure (J6):** 0.5–4.5 V ratiometric transducer on +5 V. R16/R17 10k/20k 0.1 % divide it by ⅔ (4.5 V → 3.0 V), with C9 100 nF, into GPIO0 (ADC1).
  - The filter corner is ~230 Hz, so the pump's 60 Hz pulses stay visible to the firmware.
- **Switches (J7):** brew and steam as dry contacts to GND, on internal pull-ups, debounced in
  firmware.

### Pin map (ESP32-C6)
| Signal | GPIO | | Signal | GPIO |
|---|---|---|---|---|
| Valve / pump / heater | 18 / 19 / 20 | | Pressure (ADC1_CH0) | 0 |
| Brew / steam switch | 6 / 7 | | Thermistor (ADC1_CH1) | 1 |
| Status LED | 23 | | USB D− / D+ | 12 / 13 |
| BOOT pad | 9 | | GPIO8 strap (pull-up) | 8 |

The drive pins are ordered so that, along the module's edge, they line up with the three optos
(valve, pump, heater from top to bottom). The traces don't cross.

## Layout
- **Board:** 64.5 × 56.5 mm, 2 layers, 2 oz copper.
- **Mains side, left: horizontal lanes**, top to bottom:
  - supply: mains-in terminal → fuse → MOV → PSU;
  - valve;
  - pump;
  - heater.

  Each lane runs from its opto on the band, through its resistors and snubber, to its triac and
  its terminal on the left edge, so every mains connection is short and local.
  - The heater triac sits on the left edge, ready to bolt to the chassis. Its pours run to J2 on
    the bottom edge.
- **SELV side, right:**
  - the C6 module sits under the PSU, turned so its drive pins face the optos;
  - USB-C on the right edge (only for flashing);
  - sensor dividers next to the module's analog pins;
  - LDO below the module;
  - JST connectors along the bottom edge;
  - GND pours on both layers.
- **All mains copper is hand-routed in code** (`layout.py`): the heater pours, gate and opto feed, and every pump, valve and supply trace. Freerouting throws exceptions inside its maze search on the mains nets (their pins sit 2.3–2.5 mm apart, which it treats as a broken starting board under the 2 mm rule) and leaves the same few connections unrouted, so it only routes the low-voltage side. Every hand trace is checked by the same DRC rules.
- **Routing:** Freerouting routes the SELV side at 45° (4 attempts in parallel, 2-minute cap; the best attempt is chosen by unrouted count, then rule violations).

  | Nets | Track width |
  |---|---|
  | Mains | 0.8 mm, at 2.0 mm clearance |
  | SELV power | 0.6 mm |
  | SELV signals | 0.25 mm |

  No track or pour may enter the band.

## Design rules (the lint)
`layout.py` writes `build/*.kicad_dru` next to the board, built from the schematic's own list of
mains nets, so the two can't drift apart. Every build runs KiCad's DRC with it, and the autorouter
gets the same 2.0 mm clearance for mains nets.

| Rule | Value |
|---|---|
| Mains net to mains net | ≥ 2.0 mm |
| Mains to SELV | the 7 mm copper-free band + slots; no net can cross it (schematic isolation check) |
| Pins inside one mains part | manufacturer's pitch (≥ 0.5 mm), only when *both* items are inside that part's courtyard |
| Same-potential mains pairs | ≥ 0.5 mm |
| Pads inside a heater keep-out | none (except the heater's own) |

## Simulation
`python3 hardware/controller/sim/run.py` runs ngspice on the circuits most likely to be wrong,
across tolerance corners. It writes [`sim/REPORT.md`](../hardware/controller/sim/REPORT.md) and
exits non-zero if any check fails. **43 / 43 pass.** Waveforms are on the viewer's
simulation page.

| Block | What's checked | Result |
|---|---|---|
| Triac drive | opto LED current from the GPIO, VDD 3.0–3.3 V, LED Vf corners | 8.2–12.1 mA (needs ≥ 5 mA) |
| Pump snubber | peak voltage and dv/dt when the triac drops the pump, 120–132 V | 574 V without it; 170–186 V with it |
| Valve snubber | the same for the valve, holding current 5–10 mA | ~2 kV without it; 363–477 V with it (< 533 V, ⅔ of 800 V) |
| Pressure input | full scale into the ADC, filter corner, 60 Hz pulses, ADC sampling kick-back | 2.97 V, 233 Hz, −0.36 dB, 0.3 mV |
| Thermistor | voltage range 20–150 °C, sensitivity, self-heating, tolerance before calibration | 3.10–0.65 V, 24 mV/°C at 93 °C, 0.2 °C, ±1.2 °C |
| Rails | 3.3 V and 5 V under Wi-Fi bursts; PSU −5 %; on USB (TLV1117LV dropout) | 3.3 V ≥ 3.29 V, 5 V ≥ 4.52 V |

The models are behavioural:
- diodes fitted to datasheet Vf;
- triacs as latching switches with a holding current;
- the regulator as a dropout curve;
- coil inductances estimated.

The results are margins, not exact waveforms. What simulation doesn't replace:
- a thermal check of Q1 on the chassis;
- EMC testing;
- the staged bring-up in INSTALL.md.

## Status and what needs a human
- Generated, autorouted and DRC-checked by code. **Not yet reviewed by a person and not yet
  built.** The audit pack (`build/coffee-controller-audit-120v.zip`) is for that review.
- **Before ordering:**
  - Open it in KiCad and walk the mains side.
  - Check the J2/J3 terminals and the Q1 tab position against the machine's side panel.
  - Confirm footprints for the parts you actually buy: the MOC3083 is the 0.4" "TVM" lead option.
- **Assembly split:** JLCPCB places the SMD parts, including the C6 module and the USB connector.
  You hand-solder the through-hole parts: terminals, triacs, optos, PSU, fuse, MOV, JST
  connectors. See `build/jlc-120v/`.
