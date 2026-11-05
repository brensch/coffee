# Parts record: v30 low-voltage board

Everything JLC places on the board, as matched live against JLCPCB's library (`jlc.py`, 2026-11-05). "basic" parts carry no setup fee; each "extended" part type adds about $3 to an assembly order. Not fitted: TP1 (BOOT, bare copper underneath). Independent review: [REVIEW-v23.md](REVIEW-v23.md). Off-board modules and cables: [BUY.md](BUY.md).

| Refs | Value | Part | LCSC | JLC library | Stock | Notes |
|---|---|---|---|---|---|---|
| C3 | 10uF 25V X5R | CL21A106KAYNNNE | C15850 | base | 7569512 | 3V3: C3 at the LDO output (>= 1 uF for stability), C4 at the module's top-left corner |
| C4 C8 C10 C11 | 100nF X7R | CC0603KRX7R9BB104 | C14663 | base | 36417613 | 3V3: C3 at the LDO output (>= 1 uF for stability), C4 at the module's top-left corner / ADC sampling reservoir / noise filter / ADC reservoir |
| C5 | 1uF X5R | CL10A105KB8NNNC | C15849 | base | 11875198 | EN delay |
| C6 C7 | 22uF 25V X5R | CL21A226MAQNNNE | C45783 | base | 3527055 | Ceramic bulk (X5R 25 V, 85 C: fine at the 60-70 C inside the machine; no electrolytics to age) |
| C9 | 22nF X7R | CL10B223KB8NNNC | C21122 | base | 849405 | ADC reservoir; ~226 Hz with the 32k divider: keeps the pump's 60 Hz pulses |
| D1 D3 D4 D5 D6 D7 D8 | red | KT-0603R | C2286 | base | 6673950 | Status: boot log flicker, on while programming, then the firmware heartbeat / indicator: power lamp / indicator: coffee lamp / indicator: steam lamp / indicator: heater on / indicator: pump on / indicator: valve on |
| D2 | B5819W | B5819W SL | C8598 | base | 793742 | USB bench power |
| J1 | 5V in | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33031 | 1=- 2=+5V 3=GND, from the 5 V AC-DC module |
| J2 | SSRs | SM07B-GHS-TB(LF)(SN) | C495552 | expand | 26134 | 2=+5V (to heater SSR 3+, pump and valve board VCC+IN) 3=heater OUT 6=pump OUT 7=valve OUT; 1,4,5 unused |
| J4 | USB-C | TYPE-C-31-M-12 | C165948 | expand | 151649 | Flashing, logs, JTAG over the C6's USB Serial/JTAG; bench power |
| J5 | Thermistor | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33031 | Molex 218537-3919 50k NTC ring-tab under an M4 screw at the brew-thermostat seat. 1=GND 3=NTC (no polarity), 2 unused |
| J6 | Pressure | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 33031 | 0.5-4.5 V ratiometric 5 V transducer (GaggiMate 0-1.6 MPa; BUY.md 10b). 1=SIG 2=+5V 3=GND (v29: +5V in the middle, so the supply lead in this socket just powers the board) |
| J7 | Front panel | SM07B-GHS-TB(LF)(SN) | C495552 | expand | 26134 | 1=POWER 2=BREW 3=STEAM 4=GND 5=LED power 6=LED coffee 7=LED steam (LED cathodes to GND) |
| J10 | Scale | SM07B-GHS-TB(LF)(SN) | C495552 | expand | 26134 | 3=DOUT1 4=SCK 5=GND 6=DOUT2 7=+3V3 (VCC+VDD tied at the HX711 board), 1-2 unused (dual HX711 board in the tray base) |
| Q1 Q2 Q3 | AO3400A | AO3400A | C20917 | base | 937806 | HEATER output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) / PUMP output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) / VALVE output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) |
| Q4 | AO3401A | AO3401A | C15127 | base | 1608883 | Reverse-polarity protection |
| R4 R5 R6 | 3.3k | 0603WAF3301T5E | C22978 | base | 4845966 | Gate pull-down: holds the gate < 0.5 V against a weak internal pull-up at reset (sim) |
| R10 R11 R20 R21 R22 R23 R25 R28 R36 | 10k | 0603WAF1002T5E | C25804 | base | 43670172 | EN pull-up (Espressif RC delay) / GPIO8 high: required for download mode from the BOOT pad / switch pull-up / wetting current / V3_SNS divider top / V3_SNS divider bottom / V5_SNS divider bottom / SCK pull-down: HX711 stays powered while the C6 boots |
| R12 R18 R19 R27 R29 R37 R38 R39 R40 R41 R42 | 1k | 0603WAF1001T5E | C21190 | base | 27934120 | red LED Vf ~2.0 V: ~1.3 mA from 3.3 V / GPIO series protection / GPIO1 series protection: a 5 V plug in the wrong socket pushes ~1 mA into the clamp diode / indicator LED: power lamp / indicator LED: coffee lamp / indicator LED: steam lamp / indicator LED: heater on / indicator LED: pump on / indicator LED: valve on |
| R13 R14 | 5.1k | 0603WAF5101T5E | C23186 | base | 20533055 | USB-C sink identification |
| R15 | 4.7k | 0603WAF4701T5E | C23162 | base | 13589276 | NTC reference: 3.02 V at 25 C, 1.50 V at 93 C, 0.59 V at 140 C (Molex 218537-3919 table) |
| R16 | 47k | 0603WAF4702T5E | C25819 | base | 2564863 | Divider 100/147 (147k load: TE M3200 needs > 100k) |
| R17 | 100k | 0603WAF1003T5E | C25803 | base | 16602151 |  |
| R24 | 20k | 0603WAF2002T5E | C4184 | base | 5851045 | V5_SNS divider top |
| R30 R31 R32 | 330 | 0603WAF3300T5E | C23138 | base | 2924989 | front-panel LED (pwr) current limit, ~4 mA / front-panel LED (coffee) current limit, ~4 mA / front-panel LED (steam) current limit, ~4 mA |
| U4 | ESP32-C6-MINI-1U-H4 | ESP32-C6-MINI-1U-H4 | C20627095 | expand | 3825 | W.FL / MHF III antenna (not U.FL): pigtail to a 2.4 GHz antenna outside the steel case |
| U5 | TLV1117LV33DCYR | TLV1117LV33DCYR | C15578 | expand | 1915 | SOT-223 for heat: ~0.17 W average, +11 C |

## Changed in v30 (mis-plug safety, after the v29 review)
- Re-pinned: J1 –/+5V/GND, J5 GND/–/NTC, J2 heater on pin 3, J10 –/–/DOUT1/SCK/GND/DOUT2/3V3 (SWAP-SAFETY.md).
- The scale runs from 3V3 only: no +5V on J10.

## Changed in v29 (bundled connectors)
- Six GH sockets, three 3-pin (J1, J5, J6) and three 7-pin (J2 SSRs, J7 front, J10 scale), in place of eight.

## Changed in v24 (after the independent review)
- USB-C J4 and its CC resistors R13/R14 are back (left edge); the bare programming pads are gone.
- Status LED D1 + R12 back, now on GPIO16 (U0TXD); the steam lamp moved to GPIO23; TP2 gone.
- J1 is a 2-pin GH (SM02B-GHS-TB, C189893), so it can't swap with a 3-pin output lead.
- Switch pull-ups R28/R20/R21: 4.7k → 10k.

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
