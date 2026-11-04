# Parts record: v18 low-voltage board

Everything on the board, as matched live against JLCPCB's library (`jlc.py`, 2026-11-04). Each part was checked against its manufacturer's datasheet (pin map, package, every value relied on). "basic" parts carry no
setup fee; each "extended" part type adds about $3 to an assembly order. Off-board modules and cables: [BUY.md](BUY.md).

| Refs | Value | Part | LCSC | JLC library | Stock | Notes |
|---|---|---|---|---|---|---|
| C3 | 10uF 25V X5R | CL21A106KAYNNNE | C15850 | base | 7650831 | Module decoupling at the 3V3 pin |
| C4 C8 C10 C11 | 100nF X7R | CC0603KRX7R9BB104 | C14663 | base | 37539563 | ADC reservoir / ADC sampling reservoir / noise filter / Module decoupling at the 3V3 pin |
| C5 | 1uF X5R | CL10A105KB8NNNC | C15849 | base | 12144479 | EN delay |
| C6 C7 | 22uF 25V X5R | CL21A226MAQNNNE | C45783 | base | 3580380 | Ceramic bulk (X5R 25 V, 85 C: fine at the 60-70 C inside the machine; no electrolytics to age) |
| C9 | 22nF X7R | CL10B223KB8NNNC | C21122 | base | 860568 | ADC reservoir; ~226 Hz with the 32k divider: keeps the pump's 60 Hz pulses |
| D1 | red | KT-0603R | C2286 | base | 6729573 | Heartbeat / ready / fault. Light-pipe it to the front of the machine (optional) |
| D2 | B5819W | B5819W SL | C8598 | base | 812153 | USB bench power |
| D5 | 1N4148W | 1N4148W | C81598 | base | 3686304 | Flyback for the relay coil |
| J1 | 5V in | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33218 | 1=+5V 2=GND 3=GND, from the 5 V AC-DC module |
| J2 | Heater | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33218 | HEATER: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| J3 | Pump | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33218 | PUMP: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| J4 | USB-C | TYPE-C-31-M-12 | C165948 | expand | 156868 | Flashing, logs, JTAG over the C6's USB Serial/JTAG; bench power |
| J5 | Thermistor | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33218 | Molex 218537-3919 50k NTC ring-tab under an M4 screw at the brew-thermostat seat. 1=NTC 2=GND (no polarity), 3 unused |
| J6 | Pressure | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33218 | TE M3234-000002-250PG, 250 psi, 5 V, 0.5-4.5 V ratiometric. 1=+5V 2=SIG 3=GND |
| J7 | Switches | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33218 | 1=BREW 2=STEAM 3=GND. Stock switches rewired as dry contacts |
| J8 | Valve | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33218 | VALVE: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| Q1 Q2 Q3 | AO3400A | AO3400A | C20917 | base | 940575 | HEATER output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) / PUMP output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) / VALVE output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) |
| Q4 | AO3401A | AO3401A | C15127 | base | 1637124 | Reverse-polarity protection |
| R4 R5 R6 | 3.3k | 0603WAF3301T5E | C22978 | base | 4907881 | Gate pull-down: holds the gate < 0.5 V against a weak internal pull-up at reset (sim) |
| R10 R11 R22 R23 R25 R26 | 10k | 0603WAF1002T5E | C25804 | base | 44427549 | EN pull-up (Espressif RC delay) / GPIO8 high: required for download mode from the BOOT pad / P-FET gate to GND / V3_SNS divider bottom / V3_SNS divider top / V5_SNS divider bottom |
| R12 R18 R19 R27 | 1k | 0603WAF1001T5E | C21190 | base | 28493741 | GPIO series protection / GPIO1 series protection: a 5 V plug in the wrong socket pushes ~1 mA into the clamp diode / red LED Vf ~2.0 V: ~1.3 mA from 3.3 V |
| R13 R14 | 5.1k | 0603WAF5101T5E | C23186 | base | 20629861 | USB-C sink identification |
| R15 R20 R21 | 4.7k | 0603WAF4701T5E | C23162 | base | 13897554 | NTC reference: 3.02 V at 25 C, 1.50 V at 93 C, 0.59 V at 140 C (Molex 218537-3919 table) / switch pull-up / wetting current |
| R16 | 47k | 0603WAF4702T5E | C25819 | base | 2623724 | Divider 100/147 (147k load: TE M3200 needs > 100k) |
| R17 | 100k | 0603WAF1003T5E | C25803 | base | 16780669 |  |
| R24 | 20k | 0603WAF2002T5E | C4184 | base | 5916304 | V5_SNS divider top |
| U4 | ESP32-C6-MINI-1U-H4 | ESP32-C6-MINI-1U-H4 | C20627095 | expand | 3855 | W.FL / MHF III antenna (not U.FL): pigtail to a 2.4 GHz antenna outside the steel case |
| U5 | TLV1117LV33DCYR | TLV1117LV33DCYR | C15578 | expand | 2209 | SOT-223 for heat: ~0.17 W average, +11 C |

TP1 (BOOT) is a bare copper pad on the underside: nothing is assembled there.

## Extended parts (4 types)
- **JST GH SM03B-GHS-TB** (×7): one connector type for every cable, which keeps it to one fee.
- **HRO TYPE-C-31-M-12**: USB-C.
- **ESP32-C6-MINI-1U-H4**: the module.
- **TI TLV1117LV33DCYR** (C15578, TI's own part; another maker's 12 V part shares the name): ceramic-stable LDO in SOT-223 for heat (CIRCUIT.md). The basic AMS1117 needs a tantalum output cap and can oscillate on ceramics.

## Removed with the mains section (v14 → v15)
- Triacs: BTA24-800BW, BTA08-800BW, Z0109NN.
- MOC3083 opto-triacs, X2 snubber caps, 1 W snubber resistors.
- RECOM RAC03E on-board supply, T500 mA fuse, Phoenix screw terminals.
- Their jobs now go to modules (BUY.md).
