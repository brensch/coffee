# Parts record: v15 low-voltage board

Everything on the board, as matched live against JLCPCB's library (`jlc.py`, 2026-11-04). "basic" parts carry no
setup fee; each "extended" part type adds about $3 to an assembly order. Off-board modules and cables: [BUY.md](BUY.md).

| Refs | Value | Part | LCSC | JLC library | Stock | Notes |
|---|---|---|---|---|---|---|
| C3 | 10uF 25V X5R | CL21A106KAYNNNE | C15850 | base | 7645140 | Module decoupling at the 3V3 pin |
| C4 C8 C9 C10 C11 | 100nF X7R | CC0603KRX7R9BB104 | C14663 | base | 37633469 | ADC reservoir |
| C5 | 1uF X5R | CL10A105KB8NNNC | C15849 | base | 12118706 | EN delay |
| C6 C7 | 22uF 25V X5R | CL21A226MAQNNNE | C45783 | base | 3589841 | Ceramic bulk (X5R 25 V, 85 C: fine at the 60-70 C inside the machine; no electrolytics to age) |
| D1 | red | KT-0603R | C2286 | base | 6742764 | Heartbeat / ready / fault. Light-pipe it to the front of the machine (optional) |
| D2 | B5819W | B5819W SL | C8598 | base | 814284 | USB bench power |
| D5 | 1N4148W | 1N4148W | C81598 | base | 3697763 | Flyback for the relay coil |
| J1 | 5V in | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 34206 | 1=+5V 2=GND 3=GND, from the 5 V AC-DC module |
| J2 | Heater | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 34206 | HEATER: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| J3 | Pump | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 34206 | PUMP: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| J4 | USB-C | TYPE-C-31-M-12 | C165948 | expand | 160176 | Flashing, logs, JTAG over the C6's USB Serial/JTAG; bench power |
| J5 | Thermistor | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 34206 | 100k B3950 M4 probe in the brew-thermostat hole. 1=NTC 2=GND (no polarity), 3 unused |
| J6 | Pressure | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 34206 | 0-1.6 MPa, 5 V, 0.5-4.5 V. 1=+5V 2=SIG 3=GND |
| J7 | Switches | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 34206 | 1=BREW 2=STEAM 3=GND. Stock switches rewired as dry contacts |
| J8 | Valve | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 34206 | VALVE: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / - |
| Q1 Q2 Q3 | AO3400A | AO3400A | C20917 | base | 945540 | HEATER output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA) |
| Q4 | AO3401A | AO3401A | C15127 | base | 1639709 | Reverse-polarity protection |
| R4 R5 R6 | 3.3k | 0603WAF3301T5E | C22978 | base | 4912144 | Gate pull-down: holds the gate < 0.5 V against a weak internal pull-up at reset (sim) |
| R10 R11 R16 R22 R23 R25 R26 | 10k | 0603WAF1002T5E | C25804 | base | 44446972 | Divider 2/3 |
| R12 R18 R19 | 1k | 0603WAF1001T5E | C21190 | base | 28492729 | GPIO series protection |
| R13 R14 | 5.1k | 0603WAF5101T5E | C23186 | base | 20635164 | USB-C sink identification |
| R15 | 8.2k | 0603WAF8201T5E | C25981 | base | 730112 | NTC reference: 3.05 V at 25 C, ~1.65 V at 95 C, 0.7 V at 145 C |
| R17 R24 | 20k | 0603WAF2002T5E | C4184 | base | 5917071 | V5_SNS divider top |
| R20 R21 | 4.7k | 0603WAF4701T5E | C23162 | base | 13940042 | switch pull-up / wetting current |
| U4 | ESP32-C6-MINI-1U-H4 | ESP32-C6-MINI-1U-H4 | C20627095 | expand | 3863 | W.FL / MHF III antenna (not U.FL): pigtail to a 2.4 GHz antenna outside the steel case |
| U5 | ME6211C33M5G-N | ME6211C33M5G-N | C82942 | expand | 367387 | EN tied to VIN |

TP1 (BOOT) is a bare copper pad on the underside: nothing is assembled there.

## Extended parts (4 types)
- **JST GH SM03B-GHS-TB** (×7): one connector type for every cable, which keeps it to one fee.
- **HRO TYPE-C-31-M-12**: USB-C.
- **ESP32-C6-MINI-1U-H4**: the module.
- **ME6211C33M5G-N**: ceramic-stable LDO. The basic AMS1117 needs a tantalum output cap and can oscillate on ceramics.

## Removed with the mains section (v14 → v15)
- Triacs: BTA24-800BW, BTA08-800BW, Z0109NN.
- MOC3083 opto-triacs, X2 snubber caps, 1 W snubber resistors.
- RECOM RAC03E on-board supply, T500 mA fuse, Phoenix screw terminals.
- Their jobs now go to modules (BUY.md).
