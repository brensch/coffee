# Parts record: v45 board (mains for the pump, valve and supply on board)

Everything JLC places on the board, as matched live against JLCPCB's library (`jlc.py`, 2026-11-06). "base" parts carry no setup fee; each "expand" (extended) part type adds about $3 to an assembly order. The six through-hole parts (PS1, J5, J12-J15) are JLC-soldered in the full-assembly files and listed in `build/coffee-v45-smd-hand-solder.csv` for the SMD-only order (CIRCUIT.md, "Ordering"). Not fitted: TP1 (BOOT pad). No mounting holes: the board slides into its holder (hardware/holder). Datasheet evidence for every part: [evidence/parts.jsonl](evidence/parts.jsonl). Off-board parts: [BUY.md](BUY.md).

| Refs | Value | Part | LCSC | JLC library | Stock | Notes |
|---|---|---|---|---|---|---|
| C3 | 10uF 25V X5R | CL21A106KAYNNNE | C15850 | base | 14143747 | 3V3: C3 at the LDO output (>= 1 uF for stability), C4 at the module's top-left corner |
| C4 C11 C13 C14 | 100nF X7R | CC0603KRX7R9BB104 | C14663 | base | 34736906 | 3V3: C3 at the LDO output (>= 1 uF for stability), C4 at the module's top-left corner / ADC reservoir / MCP3421 VDD bypass / TMP117 V+ bypass |
| C5 C12 | 1uF X5R | CL10A105KB8NNNC | C15849 | base | 12764453 | EN delay / Differential filter across the ADC inputs: ~80 Hz with R43/R44 |
| C6 C7 | 22uF 25V X5R | CL21A226MAQNNNE | C45783 | base | 3444773 | Ceramic bulk (X5R 25 V, 85 C: fine at the 60-70 C inside the machine; no electrolytics to age) |
| C9 | 22nF X7R | CL10B223KB8NNNC | C21122 | base | 817629 | ADC reservoir; ~226 Hz with the 32k divider: keeps the pump's 60 Hz pulses |
| C15 C16 | 22nF X2 | GA355DR7GB223KW01L | C437417 | expand | 7975 | MAINS: snubber C, Class X2 250 VAC (Gaggiuino uses an unrated MLCC here). ~1 mA off-state leakage |
| D1 D3 D4 D5 D6 D7 D8 | red | KT-0603R | C2286 | base | 6665256 | Status: boot log flicker, on while programming, then the firmware heartbeat / indicator: power lamp / indicator: coffee lamp / indicator: steam lamp / indicator: heater on / indicator: pump on / indicator: valve on |
| D2 | B5819W | B5819W SL | C8598 | base | 768943 | USB bench power |
| F1 | 2A T | 0443002.DR | C187538 | expand | 1367 | MAINS. 2 A slow-blow, 250 VAC, 50 A breaking: board live feed (load < 0.7 A; I2t 18 A2s rides the IRM inrush) |
| J2 | Heater SSR | SM04B-GHS-TB(LF)(SN) | C189895 | expand | 17878 | 1=+5V (to SSR 3 +) 2=heater OUT (to SSR 4 -) 3-4=unused (4 pins only for a unique size) |
| J4 | USB-C | TYPE-C-31-M-12 | C165948 | expand | 323731 | Flashing, logs, JTAG over the C6's USB Serial/JTAG; bench power |
| J5 | Thermocouple | MKDS 1,5/ 2-5,08 (Phoenix 1715721) | C5183929 | expand | 4260 | v39: screw terminal. 1=TC+ (chromel: non-magnetic; yellow ANSI, green IEC) 2=TC- (alumel: the magnetic wire; red ANSI, white IEC); the probe's bare wires clamp in. Cold junction measured by U7 beside it |
| J6 | Pressure | SM03B-GHS-TB(LF)(SN) | C514175 | expand | 28841 | 0.5-4.5 V ratiometric 5 V transducer (Gaggiuino 0-1.2 MPa; BUY.md 7). 1=SIG 2=+5V 3=GND (v29: +5V in the middle, so the supply lead in this socket just powers the board) |
| J7 | Front panel | SM06B-GHS-TB(LF)(SN) | C133065 | expand | 15704 | 1=BREW 2=STEAM 3=GND 4=LED power 5=LED coffee 6=LED steam (LED cathodes to GND) |
| J10 | Scale | SM05B-GHS-TB(LF)(SN) | C189896 | expand | 25527 | 1=DOUT1 2=SCK 3=GND 4=DOUT2 5=+3V3 (VCC+VDD tied at the HX711 board; dual HX711 board in the tray base) |
| J12 | N | 63824-1 | C575074 | expand | 12429 | MAINS. N: neutral (feeds the IRM module only) |
| J13 | L | 63824-1 | C575074 | expand | 12429 | MAINS. L: live, unswitched (pump, valve, IRM) |
| J14 | P | 63824-1 | C575074 | expand | 12429 | MAINS. P: switched live to the pump |
| J15 | 3 | 63824-1 | C575074 | expand | 12429 | MAINS. 3: switched live to the 3-way valve |
| PS1 | IRM-10-5 | IRM-10-5 | C2857903 | expand | 3993 | Isolated 5 V supply; bridges the isolation band (AC pins above it, DC pins below). 0.45 A peak load vs 1 A derated at 70 C |
| Q1 Q2 Q3 | AO3400A | AO3400A | C20917 | base | 1079776 | HEATER output: sinks the external heater SSR input (7-15 mA) / PUMP output: sinks the opto-triac LED (~9 mA) / VALVE output: sinks the opto-triac LED (~9 mA) |
| Q5 | T435-600B | T435-600B-TR | C145128 | expand | 1678 | MAINS: PUMP. Pump: IH 35 mA, 400 V/us. ~0.3-0.5 W on its AC_L_F copper (~90-100 C junction at 70 C ambient) |
| Q6 | T410-600B | T410-600B-TR | C2970064 | expand | 1320 | MAINS: VALVE. Valve: IH 15 mA (keeps the coil's turn-off kick to 456 V worst case), 40 V/us; ~0.1 W |
| R4 R5 R6 | 3.3k | 0603WAF3301T5E | C22978 | base | 4821040 | Gate pull-down: holds the gate < 0.5 V against a weak internal pull-up at reset (sim) |
| R10 R20 R21 R25 R28 R36 | 10k | 0603WAF1002T5E | C25804 | base | 42728470 | EN pull-up (Espressif RC delay) / switch pull-up / wetting current / V5_SNS divider bottom / SCK pull-down: HX711 stays powered while the C6 boots |
| R11 R46 | 4.7k | 0603WAF4701T5E | C23162 | base | 16104758 | I2C SDA pull-up; also GPIO8's strap pull-up (high at reset) / I2C SCL pull-up; also GPIO9 high at reset = normal boot |
| R12 R18 R19 R29 R37 R38 R39 R40 R41 R42 R43 R44 | 1k | 0603WAF1001T5E | C21190 | base | 27526943 | red LED Vf ~2.0 V: ~1.3 mA from 3.3 V / GPIO series protection / indicator LED: power lamp / indicator LED: coffee lamp / indicator LED: steam lamp / indicator LED: heater on / indicator LED: pump on / indicator LED: valve on / TC+ input: noise filter with C12 (probe lead runs past the heater wiring) / TC- input series: matches R43 (symmetric filter) |
| R13 R14 | 5.1k | 0603WAF5101T5E | C23186 | base | 20427374 | USB-C sink identification |
| R16 | 47k | 0603WAF4702T5E | C25819 | base | 3029207 | Divider 100/147 (147k load: TE M3200 needs > 100k) |
| R17 | 100k | 0603WAF1003T5E | C25803 | base | 16277306 |  |
| R24 | 20k | 0603WAF2002T5E | C4184 | base | 5724785 | V5_SNS divider top |
| R30 R31 R32 R47 R48 | 330 | 0603WAF3300T5E | C23138 | base | 2893759 | front-panel LED (pwr) current limit (plus the LED's own in its lead): ~2 mA / front-panel LED (coffee) current limit (plus the LED's own in its lead): ~2 mA / front-panel LED (steam) current limit (plus the LED's own in its lead): ~2 mA / PUMP opto LED: (5 - 1.25) / 330 = 11 mA, 2x the 5 mA IFT max, under 50 mA / VALVE opto LED: (5 - 1.25) / 330 = 11 mA, 2x the 5 mA IFT max, under 50 mA |
| R45 | 10M | 0603WAF1005T5E | C7250 | base | 836522 | Open-probe bias on the connector side of R43: open -> ADC over range -> fault; probe in: ~3 uV, calibrated out |
| R49 R50 | 330 | 1206W4F3300T5E | C17930 | expand | 204965 | MAINS: opto pin 6 to MT2: holds the opto's surge under its 1 A peak (onsemi AN-3004); 1206 for the 170 V peak |
| R51 R52 | 100 1W | 25121WF1000T4E | C35218 | expand | 38578 | MAINS: snubber R (inductive load) |
| R53 | 1k | 1206W4F1001T5E | C4410 | base | 1572607 | MAINS: gate-MT1 for the T410's 10 mA logic-level gate: dV/dt immunity (onsemi AN-3004: 330 R - 1k) |
| R54 | 51k 500V | CRCW251251K0JNEG | C2078757 | expand | 880 | MAINS: pump bleeder across the triac: re-arms the zero-cross opto every cycle (review M1) |
| U4 | ESP32-C6-MINI-1U-H4 | ESP32-C6-MINI-1U-H4 | C20627095 | expand | 3782 | W.FL / MHF III antenna (not U.FL): pigtail to a 2.4 GHz antenna outside the steel case |
| U5 | TLV1117LV33DCYR | TLV1117LV33DCYR | C15578 | expand | 1541 | SOT-223 for heat: ~0.17 W average, +11 C |
| U6 | MCP3421A0T-E/CH | MCP3421A0T-E/CH | C29454 | expand | 8953 | Thermocouple ADC, I2C 0x68: PGA 8, 18 bit, 3.75 SPS (15.6 uV/8 = 2 uV per LSB, 0.05 C) |
| U7 | TMP117AIDRVR | TMP117AIDRVR | C699536 | expand | 11609 | Cold junction: at J5's pads, I2C 0x48 (ADD0 to GND), +-0.2 C max to 100 C; thermal pad soldered to GND (TI allows floating or grounded) |
| U8 U9 | MOC3083SR2M | MOC3083SR2M | C111561 | expand | 26297 | PUMP: isolation barrier (pins 1-3 SELV, 4-6 mains) over a slot. Fires only within 20 V of a zero crossing / VALVE: isolation barrier (pins 1-3 SELV, 4-6 mains) over a slot. Fires only within 20 V of a zero crossing |

## Added in v33
- R54, 51k 500 V (Vishay CRCW251251K0JNEG) across the pump triac: re-arms the zero-cross opto every cycle (REVIEW-v32.md, M1).

## Added in v32 (mains on board)
- Mains: spade tabs J12-J15 (N, L, P, 3), fuse F1, Mean Well IRM-10-5 PS1, opto-triacs U8/U9, triacs Q5 (T435, pump) and Q6 (T410, valve), opto resistors R47-R50, gate resistor R53, snubbers R51/C15, R52/C16.
- J2 (heater SSR) became a GH 4-pin, J5 (thermocouple) a GH 2-pin: unique sizes, no wrong lead fits.

## Removed in v32
- J1 (5 V in), Q4 (reverse-polarity P-FET): the supply is on the board.
- R22/R23/C10 (3.3 V sense): the thermocouple ADC doesn't use the 3.3 V rail as a reference.
- v31's WAGO 2060 thermocouple terminal: the probe's own wires are crimped into a GH plug.

## Changed in v31 (thermocouple)
- NTC divider (R15, R27, C8) replaced by the MCP3421 + TMP117 front end (U6, U7, R43-R45, C12-C14).

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
