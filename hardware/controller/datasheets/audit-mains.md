# Datasheet audit: mains switching side

Scope: Q1 BTA24-800BW (heater), Q2 BTA08-800CW (pump), Q3 Z0109NN (valve), U1-U3 MOC3083, R1/R2/R4 360 Ω,
R5 330 Ω, R3/R6 100 Ω 1 W, C1/C2 Murata GA355QR7GB103KW01L, R7-R9 150 Ω, J1-J3 Phoenix MKDS.
Sources checked: `hardware/controller/controller.py`, `docs/CIRCUIT.md`, `docs/HARDWARE.md`, `sim/run.py`,
`sim/REPORT.md`, `build/jlc-120v/report.md`, `firmware/esp32/src/control.rs`, `crates/coffee-board/src/lib.rs`,
and the KiCad 7 footprints.

Audit date: 2026-11-03. Nothing in the design was edited.

## Datasheets used

st.com and onsemi.com refused automated downloads (timeout or Akamai "Access Denied"), so I read the manufacturers' own
PDFs from distributor mirrors. The revision of each is given. Where a newer revision exists and I could not read it,
the table says so.

| Part | Official URL | Copy actually read |
|---|---|---|
| BTA24 (ST) | https://www.st.com/resource/en/datasheet/bta24.pdf | ST "BTA24, BTB24, BTA25, BTA26, BTB26, T25", Rev 10, July 2007, via LCSC: https://datasheet.lcsc.com/datasheet/pdf/bf1b248f9a55c89cd3aecbebe71293b0.pdf |
| BTA08 (ST) | https://www.st.com/resource/en/datasheet/bta08.pdf | ST DocID7472 Rev 10, Jan 2017: https://www.farnell.com/datasheets/2307020.pdf (Rev 9 also checked: https://static.chipdip.ru/lib/642/DOC001642295.pdf) |
| Z01 (ST) | https://www.st.com/resource/en/datasheet/z01.pdf | ST DS2116 Rev 11, May 2019, via LCSC: https://datasheet.lcsc.com/datasheet/pdf/44394c07ec3d46848d101379087390aa.pdf |
| MOC3083M (onsemi) | https://www.onsemi.com/pdf/datasheet/moc3083m-d.pdf | Fairchild/onsemi MOC3081M-3083M Rev 1.5, Nov 2015, via LCSC: https://datasheet.lcsc.com/datasheet/pdf/4827f1f3302a480dbea3a735c97ac4a8.pdf (onsemi Rev 2, Sep 2022, not read) |
| Murata GA355QR7GB103KW01L | https://www.murata.com/en-us/products/productdetail?partno=GA355QR7GB103KW01%23 | Murata reference sheet GA355QR7GB103KW01-01 (Sep 2018) plus cautions JEMCGC-02981E: https://datasheet.lcsc.com/datasheet/pdf/4d4ad48ae7132cd98d507f9c4a90dfb7.pdf |
| Phoenix MKDS 3/2 (1711026) | https://www.phoenixcontact.com/en-us/products/pcb-terminal-block-mkds-3-2-1711026 | Phoenix catalog extract 2015: https://static.chipdip.ru/lib/558/DOC037558696.pdf |
| Phoenix MKDS 3/2-5,08 (1711725) | https://www.phoenixcontact.com/en-us/products/pcb-terminal-block-mkds-3-2-508-1711725 | Phoenix catalog extract (CC-2009): https://static.chipdip.ru/lib/145/DOC012145882.pdf |
| Phoenix MKDS 1,5/2-5,08 (1715721) | https://www.phoenixcontact.com/en-us/products/pcb-terminal-block-mkds-15-2-508-1715721 | https://www.farnell.com/datasheets/2138244.pdf, https://static.chipdip.ru/lib/775/DOC031775831.pdf |
| UNI-ROYAL 25121WF1000T4E (R3, R6) | — | https://datasheet.lcsc.com/datasheet/pdf/0a975aaa49b7c97f38a963127be4a823.pdf |
| FOJAN FRC1206F3600TS (R1, R2, R4) | — | https://datasheet.lcsc.com/datasheet/pdf/106fb934f95f063b695dad1b31b1eab8.pdf |
| ZhongXin BTA08-800CW-A (what jlc.py actually matched for Q2, LCSC C53426907) | — | https://datasheet.lcsc.com/datasheet/pdf/0cb23f8483a595ec673a0ff7bc03e4de.pdf |
| Vishay IL4216 (power-factor and snubber note for opto-triacs) | https://www.vishay.com/docs/83630/il4216.pdf | same |

---

## Q1 BTA24-800BW: heater, 11.4 A resistive

| Claim | Where | Datasheet value (ST bta24 Rev 10) | Verdict |
|---|---|---|---|
| TO-220 pinout 1 = MT1, 2 = MT2, 3 = G | controller.py docstring, `TRIAC_TO220` | Page 1 package drawing, TO-220AB Insulated (BTA24): A1, A2, G. Tab insulated (2500 V RMS). | OK |
| Footprint pad numbering | `Package_TO_SOT_THT:TO-220-3_Vertical` | Pads 1/2/3 at 0, 2.54, 5.08 mm, in line with the pin order | OK |
| 25 A | controller.py, CIRCUIT.md | IT(RMS) = 25 A at **Tc = 75 °C** for TO-220AB Ins. (Table 2) | OK. The 25 A applies only at Tc = 75 °C, so it is not a free margin. |
| 800 V | all docs | VDRM/VRRM 800 V; VDSM/VRSM = VDRM + 100 V (Table 2) | OK |
| Snubberless, IGT 50 mA (BW) | controller.py MPN | BW: IGT ≤ 50 mA (Q I-III), VGT ≤ 1.3 V, IH ≤ 75 mA, IL ≤ 80 mA (I/III) or 100 mA (II), dV/dt ≥ 1000 V/µs, (dI/dt)c ≥ 22 A/ms with no snubber (Table 3) | OK |
| Rth(j-c) 1.7 °C/W | controller.py comment, CIRCUIT.md | Rth(j-c) for TO-220AB Insulated = 1.7 °C/W; Rth(j-a) = 60 °C/W (Table 6) | OK |
| "~11 W at 11.4 A" | CIRCUIT.md | VTO ≤ 0.85 V, Rd ≤ 16 mΩ at 125 °C (Table 5). P = 0.85·(0.9·11.4) + 0.016·11.4² = 8.7 + 2.1 = **10.8 W** | OK |
| "~24 °C over the chassis" | controller.py, CIRCUIT.md | j-c alone is 10.8 × 1.7 = 18.4 °C. A pad or grease interface adds about 0.3-1 °C/W (3-11 °C; the interface is UNVERIFIED). Tj is about chassis + 22-30 °C. At a 70 °C chassis, Tj is about 92-100 °C, under 125 °C, and Tc is about 75-81 °C, at the 25 A rating's Tc but at less than half that current. | OK, but slightly optimistic. Depends on the interface and on how hot the chassis actually is at the bolt. |
| No gate-MT1 resistor needed | CIRCUIT.md ("only sensitive-gate triacs need one") | The MOC3083 datasheet's own hot-line circuit (Fig. 13) shows 330 Ω gate-MT1 with a standard triac. ST BW is rated dV/dt ≥ 1000 V/µs with the gate open. | OK for function. See "Should consider" 3. |
| Gate drive through 360 Ω from a zero-cross opto | controller.py | 50 mA max IGT needs line ≥ 0.05·360 + VTM(opto) 3.0 + VGT 1.3 = 22.3 V. At 108 V low line that is 8.4° (0.39 ms) after the crossing. The opto latches below VINH (≤ 20 V) and keeps conducting as the line rises, because its IH is only 0.5 mA. Load current at 8.4° is 2.4 A, far above IL 80 mA. | OK |
| Junction 125 °C | HARDWARE.md | Tj -40 to +125 °C (Table 2) | OK |
| Lifecycle | — | LCSC C15293 is genuine ST BTA24-800BWRG (1738 in stock). Findchips lists the "BTA24-800BW" family code as obsolete. | UNVERIFIED (st.com status not reachable) |

## Q2 BTA08-800CW: pump, Ulka with internal diode

| Claim | Where | Datasheet value (ST DocID7472 Rev 10) | Verdict |
|---|---|---|---|
| Part exists: "ST BTA08-800CWRG (8 A, 800 V, insulated tab, snubberless)" | controller.py MPN, jlc.py | Table 10 (Product selector): **BTA08-xxxCW is marked for 600 V only.** Table 11 (Ordering) lists BTA08-600CWRG, BTA08-800BWRG, BTA08-800CRG and BTB08-800CWRG. **There is no BTA08-800CWRG.** Rev 9 (Nov 2016) is the same. | **WRONG**. Not an ST orderable part. |
| What gets bought | build/jlc-120v/report.md | jlc.py matched LCSC C53426907 "BTA08-800CW-A", made by ZhongXin Century (ZG), not ST. Its sheet: IGT 35 mA, IH 60 mA, dV/dt 500 V/µs, Rth(j-c) 2.05 °C/W. It gives no (dI/dt)c and no insulation rating that I could find. | **RISK**: a clone with unverified commutation and tab insulation |
| TO-220 pinout 1 = MT1, 2 = MT2, 3 = G | controller.py | ST TO-220AB convention, A1-A2-G, the same as BTA24. The BTA08 pin drawing is a graphic and could not be text-extracted. | OK (same ST package convention; drawing not text-verified) |
| 8 A, 800 V | controller.py | IT(RMS) 8 A at Tc = 100 °C (TO-220AB Ins.). VDRM 800 V for C, BW and BTB08-CW (Table 1, Table 10). | OK for the real 800 V variants |
| CW: IGT 35 mA, snubberless | implied by the MPN | CW: IGT ≤ 35 mA, IH ≤ 35 mA, IL ≤ 50 mA, dV/dt ≥ 400 V/µs, (dI/dt)c ≥ 4.5 A/ms with no snubber. BW: 50 mA, 50 mA, 70 mA, 1000 V/µs, 7 A/ms (Table 2). | OK as numbers, but not for an 800 V BTA part |
| Rth | — | Rth(j-c) for TO-220AB Insulated = 2.5 °C/W; Rth(j-a) = 60 °C/W (Table 5) | — |
| "pump ~0.4 W" | HARDWARE.md | VTO ≤ 0.85 V, RD ≤ 50 mΩ. At 0.4-1 A RMS half-wave, P ≈ 0.2-0.5 W, so Tj is about 12-30 °C over ambient, free-standing (60 °C/W). | OK |
| Snubber needed? | controller.py, CIRCUIT.md | A snubberless triac needs no RC for commutation: (dI/dt)c ≥ 4.5 A/ms, and the pump's dI/dt at its current zero is well under 1 A/ms. The RC limits overshoot instead: 574 V → 170-186 V in the sim. 574 V would leave only 28 % margin to 800 V before line surges. | OK, keep the RC (it is for overvoltage, not commutation) |
| Pump current "about 0.4 A" | CIRCUIT.md | Not a datasheet item (Ulka EX5/EP5 120 V data not checked) | UNVERIFIED |
| Sim pump model | sim/run.py `snubber()` | Uses `SIN(0 vpk 50)`, which is **50 Hz**, for a 60 Hz machine. Coil L = 1.2 H and R = 220 Ω are estimates. | WRONG (minor). The result probably moves little, but the model is not the target machine. |

## Q3 Z0109NN (Z0109NN5AA4): valve, AC solenoid

| Claim | Where | Datasheet value (ST DS2116 Rev 11) | Verdict |
|---|---|---|---|
| SOT-223 pinout 1 = MT1, 2/tab = MT2, 3 = G | controller.py docstring | Page 1 SOT-223 drawing: A1, A2, G, with tab = A2 | OK |
| Footprint | `Package_TO_SOT_SMD:SOT-223-3_TabPin2` | Pad 1 (-3.15, -2.3), pad 2 (-3.15, 0) and tab pad "2", pad 3 (-3.15, +2.3) | OK |
| Order code Z0109NN5AA4 | controller.py | Fig. 20: Z01 09 = 10 mA; N = 800 V; N = SOT-223; 5AA4 = 7" reel. LCSC C145131 is genuine ST. | OK |
| "1 A, 800 V, IGT 10 mA" | controller.py | IT(RMS) 1 A at Ttab = 90 °C; VDRM 800 V; IGT ≤ 10 mA in **all four** quadrants; VGT ≤ 1.3 V; IH ≤ 10 mA; IL ≤ 15 mA (I/III/IV), 25 mA (II) (Tables 1-2) | OK |
| "sensitive gate" | controller.py `TRIAC_SOT223` description, CIRCUIT.md | ST calls Z01 "Standard 1 A Triacs" (4-quadrant, not snubberless) | WRONG label (cosmetic) |
| Needs a 330 Ω gate-MT1 resistor (R5) | controller.py, CIRCUIT.md | Static dV/dt ≥ **50 V/µs** only (gate open, Tj = 110 °C). R5 raises that immunity, and it also shunts the opto's inhibited-state leakage (IDRM2 ≤ 2 mA → 0.66 V < VGT). Cost: the opto must source IGT + VGT/330 ≈ 14 mA, reached at about 9.4 V of line. | OK, keep it |
| Commutation | sim/run.py limit "< 300 V/us (MOC3083 static dv/dt 600 min)" | **(dV/dt)c ≥ 2 V/µs at (dI/dt)c = 0.44 A/ms, Tj = 110 °C** (Table 2). Fig. 10 allows more (dI/dt)c at lower (dV/dt)c, and vice versa; the exact value could not be read off the graphic. The MOC3083 datasheet note 2: "Commutating dv/dt is a function of the load-driving thyristor(s) only." The sim's valve turn-off dv/dt is **7.8-15.7 V/µs**: 4-8 × the ST test-point rating. A low valve current (dI/dt at zero ≈ 0.05-0.15 A/ms for 0.1-0.25 A RMS) earns back some margin through Fig. 10. That is unquantified. | **RISK**. The sim checks the wrong limit. If the triac fails to commutate, the valve does not turn off when commanded. |
| Valve peak ≤ 477 V vs 800 V | CIRCUIT.md, REPORT.md | VDRM 800 V | OK (behavioural sim) |
| Dissipation "~0.03 W" | HARDWARE.md | VTO 0.95 V, Rd 400 mΩ. At ~0.1-0.2 A RMS, P ≈ 0.1-0.2 W, not 0.03 W. Rth(j-a) 60 °C/W with 5 cm² copper → +6-12 °C. | Number WRONG (low by ~4×). Harmless. |
| Tj 125 °C | HARDWARE.md | -40 to +125 °C, but dV/dt and (dV/dt)c are specified only at 110 °C | OK |

## U1-U3 MOC3083 (onsemi MOC3083TVM)

| Claim | Where | Datasheet value (MOC3081M-3083M Rev 1.5) | Verdict |
|---|---|---|---|
| Pinout 1 A, 2 K, 3 NC, 4 MT, 5 NC (substrate, do not connect), 6 MT | controller.py `MOC3083` | Fig. 1: 1 anode, 2 cathode, 3 N/C, 4 main term., 5 NC* "DO NOT CONNECT (TRIAC SUBSTRATE)", 6 main term. Pin 5 is left NC in the netlist. | OK |
| Footprint | `Package_DIP:DIP-6_W10.16mm` | Pads 1-3 at x = 0, 4-6 at x = 10.16 mm, numbered counter-clockwise (pin 4 is opposite pin 3) | OK for the TVM (0.4") option |
| Part ordered | controller.py "MOC3083TVM"; **build/jlc-120v/report.md lists MOC3083SR2M (LCSC C111561)** | Ordering table: **SR2M = "SMT 6-Pin (Lead Bend)", tape and reel**, 0.3" gull-wing. TVM = "DIP 6-Pin, 0.4" Lead Spacing". | **WRONG in the generated BOM**. The SR2M will not fit a 10.16 mm through-hole footprint. |
| 800 V | all docs | VDRM 800 V (abs. max) | OK |
| IFT 5 mA max | controller.py, CIRCUIT.md, sim | IFT ≤ 5 mA (MOC3083M) at main-terminal voltage 3 V, TA = 25 °C. Note 3: recommended IF is between max IFT and 60 mA. | OK |
| 150 Ω from a 3.3 V GPIO gives 8.2-12.1 mA | controller.py, CIRCUIT.md, REPORT.md | VF typ 1.3 V, max 1.5 V **at 30 mA**, so the sim's 1.5 V at 10 mA is conservative. Hand check at VDD 3.0 V, 25-40 Ω GPIO and VF 1.5 V: ≈ 8 mA. That is ≥ 1.6 × IFT max at 25 °C. | OK at 25 °C |
| ...at temperature (60-70 °C case interior) | HARDWARE.md "85 °C ambient" | TOPR -40 to +85 °C, TJ ≤ 100 °C. Fig. 4 (IFT normalised vs TA) is a graphic I could not extract. The 1.6× margin covers a modest temperature coefficient plus some LED ageing. | OK on rating; temperature margin UNVERIFIED |
| "Fires only within ±20 V of a zero crossing" | controller.py, CIRCUIT.md | VINH typ 12 V, max 20 V at IF = rated IFT | OK |
| Opto surge, 360 Ω "1 A peak" | controller.py R1 note | ITSM 1 A (single 60 Hz cycle). 360 Ω limits it to 0.52 A even at 132 V peak, and zero-cross firing keeps the real value far lower. VTM ≤ 3.0 V at 100 mA. IH typ 500 µA. | OK |
| dv/dt | sim limit "MOC3083 static dv/dt 600 min" | Static dv/dt ≥ 600 V/µs (typ 1500), IF = 0. Commutating dv/dt depends on the power triac only (note 2). | OK as an opto number. Wrong as the sole limit (see Q3). |
| "5.3 kV RMS isolation" | CIRCUIT.md (Isolation) | **VISO = 4170 VAC RMS, 1 min** (UL1577). VIORM 850 Vpk, VIOTM 6000 Vpk. | **WRONG** for the onsemi part |
| "~8.6 mm pad to pad" across the band | CIRCUIT.md | Footprint: 10.16 − 1.6 = 8.56 mm. Datasheet: external creepage ≥ 7 mm, clearance ≥ 10 mm for the 0.4" "TV" option. CTI 175 (material group IIIb). | OK |
| Zero-cross with an unsynchronised crystal tick, pump stroke count "exactly one per window, whatever the phase" | CIRCUIT.md, HARDWARE.md rule 3, `crates/coffee-board/src/lib.rs` `grid` model | Firmware (`control.rs`) switches each load on or off for whole crystal-timed 16.67 ms windows (cycle skipping / sigma-delta; never phase angle). That suits zero-cross drivers. **But** the opto fires whenever the LED is lit and \|V\| < VINH (12-20 V), not only at the exact crossing. At 120 V the line is below 12-20 V for ≈ ±0.19-0.31 ms around each crossing. If a window opens inside that band just after a positive crossing, the opto fires at once, and the next positive crossing still falls inside the window, so the pump gets **2 strokes**. The `grid::crossings_in` model treats the crossing as a point and misses this. Crystal-to-grid drift is slow (≈ 5 µs/cycle at 0.03 %), so once the phase lands in the band it stays there for ~40-60 consecutive cycles. | **RISK** (wrong claim, ~1-2 % of phases). Matters only if stroke counting is used for flow. The heater gets an occasional extra half-cycle (a small DC component), which is harmless. |
| Zero-cross driver on an inductive valve held on continuously | not discussed | Datasheet lists "Solenoid/Valve Controls" as an application. Fig. 13: "for highly inductive loads (power factor < 0.5), change [snubber R] to 360 Ω". Mechanism: at each current zero (lagging ~60-80° for the modelled 4.4 H / 300 Ω coil, PF ≈ 0.18), the voltage across the triac jumps towards the instantaneous line voltage. If it passes VINH before the lit opto re-fires, that half-cycle is skipped until the next voltage zero. The result can be asymmetric (DC) solenoid current, a hum and a hot coil. Vishay IL4216 p.3 recommends an RC across the triac for PF < 0.8, as here. sim/run.py gates the triac ideally, so this case is not simulated. | **RISK / UNVERIFIED**. Bench-check with a current probe. |
| Opto at 85 °C | HARDWARE.md | TOPR max 85 °C. Optos sit beside the heater triac, its pours and the PSU; local board temperature is not known. | RISK (thermal layout, UNVERIFIED) |

## Snubbers: C1/C2 Murata GA355QR7GB103KW01L, R3/R6 100 Ω 1 W, R1/R2/R4 360 Ω, R5 330 Ω

| Claim | Where | Datasheet value | Verdict |
|---|---|---|---|
| 10 nF, X2, 250 VAC, 2220 | controller.py | 10000 pF ±10 %, X7R (±15 % from -55 to 125 °C), AC 250 V, safety Class X2 (UL60384-14 E37921, VDE/EN/IEC60384-14), 5.7 × 5.0 × 1.5 mm; voltage proof DC 1075 V for 60 s; 2.5 kV impulse in the life test (Murata ref. sheet §1, §4, test 3, test 13) | OK |
| 125 °C | HARDWARE.md | Operating -55 to +125 °C; 1000 h life at 125 °C | OK |
| "10 nF (8 nF after ceramic derating)" | controller.py, CIRCUIT.md | The reference sheet gives no AC-voltage capacitance curve, only the caution "capacitance changes with AC/DC voltage" | UNVERIFIED (plausible) |
| "Ceramic safety caps don't lose capacitance over the years the way film X2 caps do" | HARDWARE.md | The Murata caution list has a "Capacitance Aging" item: X7R ages logarithmically. MLCC failure mode is crack → short, not self-healing → open like film. A shorted C1/C2 puts 100 Ω straight across the triac: the valve or pump runs permanently, and R3/R6 then see ~120 V (144 W) and burn. | Partly WRONG. RISK on mechanical cracking (board held by its edges; 2220 is crack-prone near board edges and flex). |
| Footprint creepage | `Capacitor_SMD:C_2220_5750Metric` | Pad gap 5.1 − 1.8 = 3.3 mm; the part's own termination gap g ≥ 3.0 mm | OK (> the 2.0 mm functional rule) |
| R3/R6 100 Ω 1 W 2512 (UNI-ROYAL 25121WF1000T4E) | controller.py | 2512: 1 W, max working voltage 200 V, overload 500 V | OK. Steady state: 0.45 mA through Xc = 265 kΩ, so µW. Per turn-off ½CV² ≈ 1.1 mJ at 477 V. Turn-on happens at ≤ 20 V. Oversized; that is acceptable, and it gives creepage (pad gap 4.7 mm). |
| R1/R2/R4 360 Ω 1206 (FOJAN FRC1206F3600TS) | controller.py | 1/4 W, max working voltage 200 V, overload 400 V | OK. ~0 V across it in the off state (the opto blocks); µs gate pulses ≤ ~60 mA when on. |
| R5 330 Ω 1206 | controller.py | VGT 1.3 V → 5 mW | OK |

## J1-J3 Phoenix MKDS terminals

| Claim | Where | Datasheet value | Verdict |
|---|---|---|---|
| J2 "Phoenix Contact MKDS 3/2-5.08 (1711026)" | controller.py MPN, HARDWARE.md | **1711026 is MKDS 3/2, pitch 5.00 mm** (Phoenix 2015 extract). The 5.08 mm part is **1711725 MKDS 3/2-5,08** (Phoenix CC-2009 extract). The footprint is `TerminalBlock_Phoenix_MKDS-3-2-5.08`, pads at 5.08 mm. | **WRONG MPN** (pitch mismatch) |
| J2 "24 A, 4 mm²" | controller.py, HARDWARE.md | IEC: IN 24 A; max load 28 A with 4 mm² conductor; 4 mm² **solid** max, 2.5 mm² flexible or ferruled max; rated 250 V (III/3), 400 V (III/2). **UL/cUL Recognized: 15 A / 10 A, 300 V** for 1711026 (two columns: use groups B and D); **10 A, 300 V** for 1711725 (2009 extract). | Partly WRONG. 24 A is the IEC figure at the derating-curve reference. The UL rating, which matters for a US appliance, is 10-15 A against an 11.4 A heater. |
| J2 "11.4 A at 60-70 °C" OK | HARDWARE.md | The IEC derating curve (current vs ambient) and the operating-temperature row did not survive text extraction. Insulating material PA, UL94 V0. | UNVERIFIED (check the derating curve at 70 °C) |
| J1/J3 MKDS 1,5/2-5,08 (1715721), 17.5 A | controller.py, HARDWARE.md | IEC IN 17.5 A, max load 22 A, 400 V (III/2); UL/cUL 15 A / 10 A, 300 V; pitch 5.08 mm | OK (load < 2 A) |
| Footprint pin numbering | KiCad MKDS footprints | Pad 1 (rect) at 0, pad 2 at 5.08 mm, drill 1.3 mm | OK |

---

## Must fix

1. **Q2: BTA08-800CWRG is not an ST part.** ST DocID7472 Rev 9/10 offers the CW sensitivity in BTA08 at 600 V only, and jlc.py matched a ZhongXin clone (C53426907). Change Q2 to **ST BTA08-800BWRG**: 800 V, insulated, snubberless, IGT 50 mA, dV/dt ≥ 1000 V/µs, (dI/dt)c ≥ 7 A/ms. It is driven by the same 360 Ω path, which the BTA24 analysis shows already fires a 50 mA gate. Update the MPN string in controller.py and the jlc.py mapping (`"BTA08-800CW"` → `"BTA08-800BW"`). CIRCUIT.md and HARDWARE.md name the part too. (BTB08-800CWRG exists, but its tab is live at A2.)
2. **U1-U3: the generated hand-solder BOM orders MOC3083SR2M** (SMD lead-bend, 0.3") for a 0.4" through-hole footprint (`DIP-6_W10.16mm`). Pin the opto to **MOC3083TVM** (onsemi "DIP 6-Pin, 0.4" Lead Spacing") in jlc.py (FIXED map or the mapping key), and check build/jlc-120v/hand.csv.
3. **J2 MPN: 1711026 is the 5.00 mm-pitch MKDS 3/2.** Change it to **1711725 (MKDS 3/2-5,08)** to match the 5.08 mm footprint. Also correct the "24 A" claim. It is the IEC rating; the UL/cUL rating is 10-15 A at 300 V against an 11.4 A heater. Either accept that and document it (IEC 24 A, check the derating curve at 70 °C), or move to a terminal with ≥ 20 A UL. For example a 7.5/7.62 mm-pitch MKDS 5 or a PCB 6.3 mm quick-connect tab, which matches the machine's Faston wiring; the UL rating of any replacement must be checked.
4. **Valve triac commutation is checked against the wrong limit.** sim/run.py compares the valve's turn-off dv/dt (7.8-15.7 V/µs) with the MOC3083's static dv/dt (600 V/µs). The governing limit is the Z0109's **(dV/dt)c ≥ 2 V/µs at (dI/dt)c 0.44 A/ms** (DS2116 Table 2). Do one of the following:
   - (a) Add a sim check against Z01 Fig. 10, using the measured valve current. Then bench-test turn-off at 132 V and hot (Tj ~ 100 °C).
   - (b) Swap Q3 to a 3-quadrant snubberless part with a real commutation rating. A second BTA08-800BWRG (TO-220, 7 A/ms) is one option; a T8/T4-series snubberless SMD part rated 800 V is another, but check the exact order code and pinout.
   - (c) Slow the snubber: bigger C, or the 360 Ω R that the MOC3083 Fig. 13 recommends for PF < 0.5. The peak-voltage sim must then be re-run.
5. **CIRCUIT.md: "MOC3083s (5.3 kV RMS isolation)" is wrong** for onsemi MOC3083M. The datasheet gives VISO 4170 VAC RMS (1 min), VIORM 850 Vpk and VIOTM 6000 Vpk. Correct the number.

## Should consider

1. **Pump stroke count claim.** "Exactly one stroke per window whatever the phase" (CIRCUIT.md, HARDWARE.md, coffee-board `grid` docs and tests) ignores the opto's ±VINH firing band. A window that opens within ≈ 0.2-0.3 ms after a positive crossing gives 2 strokes, and because of the slow crystal/grid drift this repeats for ~1 s at a time. Either:
   - widen the `grid::crossings_in` model to an interval (crossing ± t_inh, where t_inh = asin(VINH/Vpk)/ω) and document a ±1 tolerance; or
   - avoid it in firmware by changing the pump output only at a fixed phase. That would need a crossing reference, which the board removed.
   This matters only if flow is inferred from stroke count.
2. **Zero-cross drivers on the inductive valve (and the pump).** Confirm on the bench, with a current probe on the valve held on for ≥ 10 s at 120 V, that both half-cycles conduct (no DC offset, coil not over-temperature). If one polarity drops out, use a random-phase 800 V opto-triac for the valve channel. Firmware already switches only at its tick, so random phase costs one possibly mid-cycle turn-on per activation.
3. **Gate-MT1 resistors on Q1/Q2.** The MOC3083 datasheet's own application circuit (Fig. 13) shows 330 Ω from gate to MT1 even for a standard triac. CIRCUIT.md cites AN-3004 for leaving it out. It is not strictly required for ST snubberless BW parts (1000 V/µs gate-open), so this is optional. Adding a 1206 footprint (populate 330 Ω-1 kΩ) on Q1 and Q2 costs little, improves noise/EFT immunity on the unsnubbered heater channel, and can be left unpopulated.
4. **Snubber capacitor failure mode.** An X7R 2220 that cracks fails short and runs the load permanently through R3/R6, which then burn at ~144 W. Keep C1/C2 away from board edges and the edge-clamping grooves, and away from the J3 screw-torque area. Consider a fusible snubber resistor (flameproof/fusible 2512 or MELF) for R3/R6 so that a shorted cap opens the RC instead of burning. Reword the HARDWARE.md "don't lose capacitance" line: X7R ages logarithmically (Murata caution "Capacitance Aging"), and the real difference from film is no self-healing.
5. **BTA24 thermal claim.** "~24 °C over the chassis" covers the 18.4 °C j-c drop plus only a few °C of interface. Specify the insulating pad or grease (its Rth(c-s)) in INSTALL.md and re-state it as ≈ 22-30 °C. Note also that Rth(j-a) is 60 °C/W: running the heater with Q1 not bolted down (bench testing) gives >600 °C of rise, so firmware or INSTALL must forbid heater-on without the tab mounted.
6. **Sim pump model runs at 50 Hz** (`SIN(0 vpk 50)` in sim/run.py `snubber()`), and both coils are estimates. Change it to 60 Hz. Once a pump and valve are available, replace L and R with measured values: inductance with the plunger seated and the coil current at 120 V. Those set (dI/dt)c and PF, which items 4 and 2 depend on.
7. **Label fixes.** Z0109 is a "standard" 4-quadrant triac, not "sensitive gate" (controller.py `TRIAC_SOT223` description, CIRCUIT.md). The valve triac loss is ≈ 0.1-0.2 W, not 0.03 W (HARDWARE.md). coffee-board/src/lib.rs still says "MOC3063".
8. **TO-220 pad spacing on Q1.** `TO-220-3_Vertical` pads are 1.905 mm wide at 2.54 mm pitch: a 0.64 mm copper gap between MT1 and MT2, at full line voltage and 11.4 A. The lint exempts it as "manufacturer's pitch". With hand soldering this is the most likely bridge on the board. Consider narrower oval pads (e.g. 1.6 mm) or a staggered-lead TO-220 footprint for Q1, and conformal coat as already recommended.
9. **MOC3083 temperature margin.** IFT ≤ 5 mA is specified at 25 °C. Read Fig. 4 (IFT vs TA) of the current onsemi Rev 2 sheet to confirm that 8 mA minimum drive still leaves margin at 70-85 °C after LED ageing. A 120 Ω R7-R9 would give ≈ 10 mA minimum, still well under the 60 mA maximum, if margin is thin.

## Not verified

- st.com and onsemi.com current revisions (both blocked automated access). The ST values above come from ST's own PDFs mirrored by LCSC and Farnell; the MOC3083 values come from the Fairchild-branded onsemi Rev 1.5.
- The IFT-vs-temperature (MOC3083 Fig. 4), (dI/dt)c-vs-(dV/dt)c (Z01 Fig. 10) and MKDS derating curves are graphics and were not read numerically.
- Ulka pump and Gaggia valve coil currents, inductance and power factor.
- The X7R capacitance at 250 VAC (the 8 nF assumption).

---

## Follow-up: Q3 (valve) replacement study, superseding Must-fix 4

Load: ~9 W AC solenoid, 120 V/60 Hz, coil current 75-150 mA RMS. Driven by MOC3083 through R4 360 Ω from MT2; the
100 Ω + 10 nF snubber stays. Commutation dI/dt at the current zero is √2·I·ω = **0.04-0.08 A/ms**. The simulated
turn-off dv/dt is **8-16 V/µs**.

Sources:
- ST T4 series, DocID7699 Rev 5 (May 2014): https://www.st.com/resource/en/datasheet/t435.pdf. The copy read was
  https://www.farnell.com/datasheets/1837719.pdf. Values below are from Tables 3-6, 11 and 12 and Figs. 8-10.
- ST BTA08, DocID7472 Rev 10, and ST Z01, DS2116 Rev 11 (URLs above).
- I read the graphs (Z01 Fig. 10, T4 Fig. 9) from rendered page images, so those values are about ±10 %.

### Datasheet numbers (Tj = 25 °C unless noted)

| Part | Package | VDRM | Quadrants / type | IGT max | IL max (I-III) | IH max | Static dV/dt min | Commutation spec | Allowed (dI/dt)c at 10-16 V/µs | Margin vs 0.08 A/ms |
|---|---|---|---|---|---|---|---|---|---|---|
| **Z0109NN** (current) | SOT-223 | 800 V | 4Q standard | 10 mA | 15 mA | 10 mA | 50 V/µs (110 °C) | (dV/dt)c ≥ 2 V/µs at (dI/dt)c 0.44 A/ms, 110 °C | Fig. 10: the Z0109 curve stays ≈ 1.0 up to ~10 V/µs, ≈ 0.85 at 16 V/µs, and reaches 0 near ~50-60 V/µs → **≈ 0.37-0.44 A/ms** | **≈ 5×** at 110 °C. More when cooler (Z01 Fig. 11 rises as Tj falls). |
| **T410-800B** | DPAK (T410-800B-TR on reel) | 800 V (Table 11: available) | 3Q, "Logic level" | 10 mA | 25 mA (II: 30) | 15 mA | 40 V/µs (125 °C) | (dI/dt)c ≥ 2.7 A/ms at 0.1 V/µs; **≥ 2.0 A/ms at 10 V/µs**; 125 °C | Fig. 9: T410 ≈ 1.0 at 10-16 V/µs, reaching 0 near ~40 V/µs → **≈ 2.0 A/ms** | **≈ 25×** |
| **T435-800B** | DPAK | 800 V | 3Q Snubberless | 35 mA | 50 mA (II: 60) | 35 mA | 400 V/µs (125 °C) | (dI/dt)c ≥ 2.5 A/ms with no snubber, 125 °C | Fig. 9: T435 ≈ 1.0-1.1 → **≈ 2.5 A/ms** | **≈ 30×** |
| T435-800H / T435-800T | IPAK / TO-220AB (non-insulated: tab = A2) | 800 V | as T435-B | 35 | 50 | 35 | 400 | as T435-B | as T435-B | as T435-B. Through-hole, with no advantage over B. |
| BTA08-800BW | TO-220AB Ins. | 800 V | 3Q Snubberless | 50 mA | 70 mA (II: 80) | 50 mA | 1000 V/µs | (dI/dt)c ≥ 7 A/ms with no snubber | ≥ 7 A/ms | ≈ 90×. Large through-hole part. |

All five meet the commutation requirement by a wide margin. The original concern, that the Z0109 is 4-8× over its
2 V/µs test point, came from reading only the Table 2 point. Z01 Fig. 10 shows the Z0109 holds roughly its specified
0.44 A/ms up to ~10 V/µs, and our dI/dt is ~0.08 A/ms. **Must-fix 4 therefore reduces to: correct the sim criterion.**
Compare the valve turn-off dv/dt with the triac's (dV/dt)c curve (Z01 Fig. 10 at the measured coil current), not with
the MOC3083's static 600 V/µs. The Z0109 itself does not need replacing on commutation grounds.

### Latching with a ~0.1 A inductive load fired at zero cross

Mechanism:
- The opto fires at ≤ 20 V. Until the main triac latches, the **whole load current flows MT2 → R4 → opto → gate → MT1**.
- The coil limits that current. It is not a pulse.
- The triac turns on once the current reaches IGT. It stays on, without the opto, only once the load current exceeds IL.
- Below that, the opto path carries the load, and keeps re-firing the triac.

Pure-inductor approximation: fired at the voltage zero, i(θ) = Î·(1 − cos θ). The worst case is the smallest coil,
75 mA RMS, Î = 0.106 A. IL is the 25 °C maximum; it rises ≈ 1.2-1.3× at 0 °C (T4 Fig. 8, Z01 Fig. 5).

| Part | IL used | Angle / time to latch | Share of each half-cycle on the opto | Peak opto/R4 current | R4 average power (≈ IL²·360·d/5) | Opto detector average (≈ 2 V · IL/3 · d) | Safe? |
|---|---|---|---|---|---|---|---|
| Z0109 (R5 330 Ω takes ~4 mA) | 15 mA | 31° / 1.4 ms | 17 % | ~19 mA | ~3 mW | ~2 mW | yes |
| T410-800B | 25 mA | 40° / 1.9 ms | 22 % | 25 mA | ~10 mW | ~4 mW | yes |
| T435-800B | 50 mA | 58° / 2.7 ms | 32 % | 50 mA | ~60 mW | ~11 mW | yes, with less margin |
| BTA08-800BW | 70 mA | 70° / 3.2 ms | 39 % | 70 mA | ~140 mW | ~18 mW | marginal for R4 |

Limits:
- MOC3083: ITSM 1 A (single cycle). Detector PD 150 mW at 25 °C, derated 1.76 mW/°C, so ≈ 70 mW at 70 °C ambient.
- R4 (FOJAN FRC1206): 1/4 W.

Every candidate latches within the first half-cycle, because the coil current keeps rising to ~2Î, well above every
IL. None overstresses the opto. The high-IL parts (T435, BTA08-BW) put a meaningful repetitive load on R4 and the opto
for a third of every half-cycle while the valve is held. BTA08-BW reaches ~55 % of R4's rating.

### Holding current sets the turn-off spike

At turn-off the triac stops at IH, with the coil still carrying that current. The coil energy ½·L·IH² then rings into
the 10 nF snubber.

The existing sim (sim/REPORT.md, 132 V, 100 Ω + 10 nF) gives:
- **394 V at IH 5 mA;**
- **458 V at IH 10 mA;**
- 477 V at IH 10 mA with the capacitor derated to 8 nF.

That is about +13 V per mA. Extrapolated linearly (UNVERIFIED: re-run `valve(ih=...)`):

| IH max | Peak |
|---|---|
| T410, 15 mA | ≈ 520-540 V, at the 533 V (⅔ VDRM) criterion |
| T435, 35 mA | ≈ 780 V |
| BTA08-BW, 50 mA | > 900 V |

Real IH is usually well below the 25 °C maximum, and it falls further with temperature (×0.6-0.75 at 60-100 °C,
T4 Fig. 8). Even so, **the high-IH snubberless parts are the wrong choice for this small, highly inductive coil** with
the 10 nF snubber unchanged. Their strength is high (dI/dt)c, which this load does not need.

### Footprint / pinout

- **DPAK.** KiCad `Package_TO_SOT_SMD:TO-252-2` has pad 1, tab pad "2" and pad 3. ST DPAK triacs use pin 1 = A1,
  tab/pin 2 = A2, pin 3 = G, which matches the existing `TRIAC_SOT223` pin map (1 MT1, 2 MT2, 3 G). The T4 Rev 5
  page-1 drawing shows only the symbol and no pin labels, so confirm against the ordering-code package drawing before
  layout (UNVERIFIED).
- **Tab.** The DPAK tab is A2 (= AC_L), as on the SOT-223. Same mains-net copper rules apply.
- **Dissipation.** At 0.15 A it is ~0.15 W. Rth(j-a) is 70 °C/W at 0.5 cm² (Table 6), so heating is trivial.

### Recommendation

1. **Keep Z0109NN.** By Fig. 10 it meets commutation with ≈ 5× margin at 110 °C, and more in practice. It has the
   lowest IL (least opto/R4 stress) and the lowest IH (lowest simulated spike, ≤ 477 V). Fix only the sim criterion
   (Must-fix 4 as amended above).
2. **If a 3-quadrant SMD part is still wanted, use ST T410-800B (-TR) in DPAK.** It is 800 V and 3Q, with
   (dI/dt)c ≥ 2.0 A/ms at 10 V/µs (~25× margin), IGT 10 / IL 25 / IH 15 mA. Keep R5 (330 Ω): its gate-open static
   dV/dt is only 40 V/µs at 125 °C. Re-run the valve sim with IH = 15 mA before committing; it extrapolates to about
   the 533 V criterion.
3. **Do not use T435-800B/H/T or BTA08-800BW for the valve** with the 100 Ω + 10 nF snubber. Their IH (35-50 mA)
   extrapolates to turn-off spikes near or above 800 V. Their IL (50-70 mA) also loads R4 and the opto for a third of
   every half-cycle. T435-H/T and BTA08 are through-hole as well.
