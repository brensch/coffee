# Parts record: v23 low-voltage board

Everything JLC places on the board, as matched live against JLCPCB's library (`jlc.py`, 2026-11-05). "basic" parts carry no setup fee; each "extended" part type adds about $3 to an assembly order. Not fitted (bare copper): J4 programming pads, TP1 BOOT, TP2 STATUS. Off-board modules and cables: [BUY.md](BUY.md).

| Refs | Value | Part | LCSC | JLC library | Stock | Notes |
|---|---|---|---|---|---|---|
| C3 | 10uF 25V X5R | CL21A106KAYNNNE | C15850 | base | 7605185 | Module decoupling at the 3V3 pin |
| C4 C8 C10 C11 | 100nF X7R | CL05B104KO5NNNC | C1525 | base | 33208561 | Module decoupling at the 3V3 pin / ADC sampling reservoir / noise filter / ADC reservoir |
| C5 | 1uF X5R | CL05A105KA5NQNC | C52923 | base | 10029800 | EN delay |
| C6 C7 | 22uF 25V X5R | CL21A226MAQNNNE | C45783 | base | 3554899 | Ceramic bulk (X5R 25 V, 85 C: fine at the 60-70 C inside the machine; no electrolytics to age) |
| C9 | 22nF X7R | 0402B223K500NT | C1532 | base | 551162 | ADC reservoir; ~226 Hz with the 32k divider: keeps the pump's 60 Hz pulses |
| D2 | B5819W | B5819W SL | C8598 | base | 802043 | USB bench power |
| J1 | 5V in | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33170 | 1=+5V 2=GND 3=GND, from the 5 V AC-DC module |
| J2 | Heater | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33170 | HEATER: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| J3 | Pump | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33170 | PUMP: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| J5 | Thermistor | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33170 | Molex 218537-3919 50k NTC ring-tab under an M4 screw at the brew-thermostat seat. 1=NTC 2=GND (no polarity), 3 unused |
| J6 | Pressure | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33170 | 0.5-4.5 V ratiometric 5 V transducer (GaggiMate 0-1.6 MPa; BUY.md 10b). 1=+5V 2=SIG 3=GND |
| J7 | Front panel | SM07B-GHS-TB(LF)(SN) | C495552 | expand | 26135 | 1=POWER 2=BREW 3=STEAM 4=GND 5=LED power 6=LED coffee 7=LED steam (LED cathodes to GND) |
| J8 | Valve | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33170 | VALVE: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| J10 | Scale | SM06B-GHS-TB(LF)(SN) | C133065 | expand | 20530 | 1=+5V 2=+3V3 3=GND 4=SCK 5=DOUT1 6=DOUT2 (dual HX711 board in the tray base) |
| Q1 Q2 Q3 | AO3400A | AO3400A | C20917 | base | 947548 | HEATER output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) / PUMP output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) / VALVE output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) |
| Q4 | AO3401A | AO3401A | C15127 | base | 1619820 | Reverse-polarity protection |
| R4 R5 R6 | 3.3k | 0402WGF3301TCE | C25890 | base | 1447459 | Gate pull-down: holds the gate < 0.5 V against a weak internal pull-up at reset (sim) |
| R10 R11 R22 R23 R25 R36 | 10k | 0402WGF1002TCE | C25744 | base | 15166895 | EN pull-up (Espressif RC delay) / GPIO8 high: required for download mode from the BOOT pad / V3_SNS divider top / V3_SNS divider bottom / V5_SNS divider bottom / SCK pull-down: HX711 stays powered while the C6 boots |
| R15 R20 R21 R28 | 4.7k | 0402WGF4701TCE | C25900 | base | 16084963 | NTC reference: 3.02 V at 25 C, 1.50 V at 93 C, 0.59 V at 140 C (Molex 218537-3919 table) / switch pull-up / wetting current |
| R16 | 47k | 0402WGF4702TCE | C25792 | base | 6607124 | Divider 100/147 (147k load: TE M3200 needs > 100k) |
| R17 | 100k | 0402WGF1003TCE | C25741 | base | 7236728 |  |
| R18 R19 R27 R29 | 1k | 0402WGF1001TCE | C11702 | base | 34036609 | GPIO series protection / GPIO1 series protection: a 5 V plug in the wrong socket pushes ~1 mA into the clamp diode |
| R24 | 20k | 0402WGF2002TCE | C25765 | base | 2560963 | V5_SNS divider top |
| R30 R31 R32 | 330 | 0402WGF3300TCE | C25104 | base | 1908585 | front-panel LED (pwr) current limit, ~4 mA / front-panel LED (coffee) current limit, ~4 mA / front-panel LED (steam) current limit, ~4 mA |
| U4 | ESP32-C6-MINI-1U-H4 | ESP32-C6-MINI-1U-H4 | C20627095 | expand | 3845 | W.FL / MHF III antenna (not U.FL): pigtail to a 2.4 GHz antenna outside the steel case |
| U5 | TLV1117LV33DCYR | TLV1117LV33DCYR | C15578 | expand | 2023 | SOT-223 for heat: ~0.17 W average, +11 C |

TP1 (BOOT) is a bare copper pad on the underside: nothing is assembled there.

## Extended parts (4 types)
- **JST GH SM03B-GHS-TB** (×7): one connector type for every cable, which keeps it to one fee.
- **HRO TYPE-C-31-M-12**: USB-C.
- **ESP32-C6-MINI-1U-H4**: the module.
- **TI TLV1117LV33DCYR** (C15578, TI's own part; another maker's 12 V part shares the name): ceramic-stable LDO in SOT-223 for heat (CIRCUIT.md). The basic AMS1117 needs a tantalum output cap and can oscillate on ceramics.

## Removed in v23 (the connector rectangle)
- USB-C socket J4 and its CC resistors R13/R14: replaced by four bare programming pads underneath.
- Status LED D1 and R12: GPIO23 goes to a bare pad (TP2).
- Valve flyback diode D5: all outputs drive SSR inputs, nothing inductive.
- P-FET gate resistor R26: the gate goes straight to GND.
- Scale series resistors R33–R35: as GaggiMate and Gaggiuino.
- 0603 passives became 0402.

## Removed with the mains section (v14 → v15)
- Triacs: BTA24-800BW, BTA08-800BW, Z0109NN.
- MOC3083 opto-triacs, X2 snubber caps, 1 W snubber resistors.
- RECOM RAC03E on-board supply, T500 mA fuse, Phoenix screw terminals.
- Their jobs now go to modules (BUY.md).
