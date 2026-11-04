# Footprint audit before fabrication (v13, 120 V)

Board: `build/coffee-controller-120v.kicad_pcb` (v13, 56 x 51 mm). Placement file: `build/coffee-v13-cpl.csv` (the same as `jlc-120v/cpl.csv`).
Method:
- Read every pad (number, net, position in the footprint frame, size, drill) and the placement with pcbnew.
- Confirmed that each board footprint is identical to its library source (KiCad 7 stock library, or `lib/*.pretty`).
- Compared pads, drills and body outlines with the manufacturer drawings listed below.
- Compared every pad net on the board with `build/coffee-controller-120v.net`.
- No files were edited.

Datasheets used (all downloaded and read for this audit):

| Tag | Document |
|---|---|
| ESP | Espressif ESP32-C6-MINI-1/1U datasheet v1.5: Table 3-1, Fig. 10-2, Fig. 11-2. https://www.espressif.com/sites/default/files/documentation/esp32-c6-mini-1_mini-1u_datasheet_en.pdf |
| Z01 | ST DS2116 Rev 11 (Z01): pinout drawing, Table 5, Fig. 15. https://datasheet.lcsc.com/datasheet/pdf/44394c07ec3d46848d101379087390aa.pdf |
| BTA24 | ST BTA24/25/26 Rev 10: TO-220AB pinout, Table 10. https://datasheet.lcsc.com/datasheet/pdf/bf1b248f9a55c89cd3aecbebe71293b0.pdf |
| BTA08 | ST DocID7472 Rev 10: Table 8, TO-220AB. https://www.farnell.com/datasheets/2307020.pdf |
| TLV | TI TLV1117LV: pin table and DCY0004A board layout. https://www.ti.com/lit/ds/symlink/tlv1117lv.pdf |
| MOC | onsemi MOC3081M–3083M (13 pp, case 646BZ is the 0.4" "T" option). https://datasheet.lcsc.com/datasheet/pdf/af1d6867f1ab5bd3f40ad0814dab6f15.pdf. Pinout and ordering code from the Fairchild Rev 1.5 copy: https://datasheet.lcsc.com/datasheet/pdf/4827f1f3302a480dbea3a735c97ac4a8.pdf |
| GCT | GCT USB4105 drawing Rev B: recommended PCB layout, pin table. https://gct.co/files/drawings/usb4105.pdf |
| RECOM | RECOM RAC03E-K/277 REV 3/2023, p.6. https://recom-power.com/pdf/Powerline_AC-DC/RAC03E-K_277.pdf |
| LF372 | Littelfuse 372 (TR5) dimensions and PCB holes. https://static.chipdip.ru/lib/869/DOC003869221.pdf |
| MKDS1.5 | Phoenix 1715721 catalogue extract. https://www.farnell.com/datasheets/2138244.pdf |
| MKDS3 | Phoenix 1711725 catalogue extract. https://static.chipdip.ru/lib/145/DOC012145882.pdf |
| JST | JST PH catalogue (through-hole layout). https://www.jst-mfg.com/product/pdf/eng/ePH.pdf |
| SS14 | MDD SS14 (LCSC C2480, the part JLC places): suggested pad layout. https://datasheet.lcsc.com/datasheet/pdf/9977bb85cd7e349115b7bcb7054cfa0d.pdf |
| KT | KENTO KT-0603G (LCSC C12624, the part JLC places), p.2. https://datasheet.lcsc.com/datasheet/pdf/69e84a3abd84e5ab5856c47a2d0334ce.pdf |
| GA355 | Murata GA355QR7GB103KW01, reflow land dimensions (Table 1). https://datasheet.lcsc.com/datasheet/pdf/4d4ad48ae7132cd98d507f9c4a90dfb7.pdf |
| UNIR | UNI-ROYAL thick-film chip resistors, §8 recommended pads (the 2512 that JLC places; also used as the generic 0603/0805/1206 reference). https://datasheet.lcsc.com/datasheet/pdf/0a975aaa49b7c97f38a963127be4a823.pdf |

Sizes are given as pad length x pad width. "Gap" is the inner edge-to-edge distance between the two pads.

## 1. Footprint-by-footprint

### J1, J3: Phoenix MKDS 1,5/2-5,08 (1715721), `TerminalBlock_Phoenix_MKDS-1,5-2-5.08_1x02_P5.08mm_Horizontal`

| Checked | Datasheet (source) | Board | Verdict |
|---|---|---|---|
| Pad count and pitch | 2 positions, pitch 5.08 (MKDS1.5) | 2 pads, 5.08 | OK |
| Drill vs pin | hole 1.3, pin 0.9 x 0.9 (diagonal 1.27) (MKDS1.5) | drill 1.3, pad Ø2.6 | OK. Matches Phoenix, but worst case is tight: JLC finished-hole tolerance of −0.08 gives 1.22, less than the 1.27 pin diagonal. Phoenix specifies 1.3, so it is left as is. |
| Body | depth 9.8 (MKDS1.5) | fab 10.26 x 9.9 | OK. The Phoenix extract also lists "width 12.7", which contradicts 2 x 5.08. UNVERIFIED (the extract has no drawing). |
| Wire-entry direction | horizontal entry, "connection direction 0°" (MKDS1.5) | KiCad puts the entry face on +y (the side with the chamfer). J1 and J3 are at −90°, so the entry faces the left board edge; the body overhangs the edge by 0.53 mm. | OK (by KiCad convention). Confirm in the 3D viewer. |
| **J1 net mapping** | `controller.py`, the netlist, the BOM note and `docs/INSTALL.md` all say **1 = L, 2 = N** | Board: **pad 1 = AC_N, pad 2 = AC_L** (`plan.json` "swap": ["J1"]). `docs/PARTS.md` says the swap is "silk-labelled", but **the board has no L/N silkscreen** (the only board texts are SELV, MAINS and "coffee 120 V"). | **WRONG**: a mains polarity hazard. Wired as documented, the fuse F1 and all three triacs end up in neutral. See Must fix 1. |
| J3 net mapping | 1 = VALVE_L, 2 = PUMP_L | same | OK |

### J2: Phoenix MKDS 3/2-5,08 (1711725), `TerminalBlock_Phoenix_MKDS-3-2-5.08_1x02_P5.08mm_Horizontal`

| Checked | Datasheet (source) | Board | Verdict |
|---|---|---|---|
| Pitch, count | pitch 5.08, 2 positions (MKDS3) | 5.08, 2 pads | OK |
| Drill vs pin | hole 1.3, pin 0.9 x 0.9 (MKDS3) | 1.3 / Ø2.6 | OK (same tolerance note as J1). Each terminal has a single pin carrying 11.4 A; that is the electrical audit's area. |
| Body | depth 11.2 (MKDS3) | fab 10.26 x 11.3 | OK. The pin position within the depth is UNVERIFIED (the extract has no drawing). |
| Wire entry | horizontal | rot 0: the entry faces the bottom board edge, body 0.55 mm over the edge | OK (KiCad convention). Confirm in 3D. |
| Pins | HTR_A / HTR_B, which are interchangeable | as netlist | OK |

### F1: Littelfuse 372 TR5 (37205000411), `Fuse_Littelfuse_372_D8.50mm`

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Lead spacing | 5.08 (LF372) | 5.08 | OK |
| Holes vs leads | leads Ø0.6; recommended PCB holes 1.0 +0.1 (LF372) | drill 1.0, pad Ø2.0 | OK |
| Body | Ø8.5, height 8 max (LF372) | fab Ø8.6, courtyard Ø9.06 | OK |
| Nets | AC_L to AC_L_FUSED | same | OK (but see J1: AC_L is the wrong conductor if J1 is wired as documented) |

### Q1, Q2: ST BTA24-800BWRG / BTA08-800BWRG (TO-220AB insulated), `TO-220-3_Vertical`

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Pinout | TO-220AB front view: 1 = A1, 2 = A2, 3 = G (BTA24 cover drawing; BTA08 the same) | Q1: 1 = HTR_A, 2 = HTR_B (opto pin 6 goes to HTR_B through R1, so HTR_B is MT2), 3 = HEATER_GATE. Q2: 1 = PUMP_L, 2 = AC_L (MT2 via R2), 3 = PUMP_GATE | OK |
| Pitch | e = 2.40–2.70 (BTA24 Table 10 / BTA08 Table 8) | 2.54 | OK |
| Drill vs lead | lead b1 0.61–0.88 x c1 0.49–0.70; worst-case diagonal 1.12 | drill 1.1, pad 1.91 x 2.0 | RISK (low). Nominal leads (about 0.75 x 0.6) fit easily. A 1.2 drill would remove the worst case. |
| Body | B 10.0–10.4 wide, C 4.4–4.6 thick | fab 10.1 x 4.5, courtyard 10.55 x 4.95 | OK |
| Orientation | Q1 tab faces the left board edge, 0.42 mm in from it (bolts to the case); Q2 tab faces +y | | OK |

### Q3: ST Z0109NN5AA4 (SOT-223), `SOT-223-3_TabPin2`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Pinout | SOT-223: 1 = A1, 2 = A2, 3 = G, tab = A2 (Z01 cover drawing) | 1 = VALVE_L (MT1, R5 to gate), 2 and tab = AC_L (MT2, R4 to the opto), 3 = VALVE_GATE | OK |
| Pitch | e = 2.3, e1 = 4.6 (Z01 Table 5) | 2.3 | OK |
| Pads | ST Fig. 15: pins 0.95 x 1.32, tab 3.25 wide, overall 7.80 | KiCad: pins 2.0 x 1.5, tab 2.0 x 3.8, overall 8.3 | OK. The pads are larger than ST's but standard IPC; creepage is in the mains audit. |
| Body | D 6.3–6.7, E 3.3–3.7, H 6.7–7.3 | fab 6.8 x 3.8 | OK |

### U5: TI TLV1117LV33DCYR (SOT-223), `SOT-223-3_TabPin2`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Pinout | 1 = GND, 2 = OUT, 3 = IN, tab = OUT (TLV pin table) | 1 = GND, 2 and tab = +3V3, 3 = +5V | OK |
| Land | DCY0004A: pins 2.15 x 0.95, tab 2.15 x 3.25, pad-centre span 5.8 | pins 2.0 x 1.5, tab 2.0 x 3.8, span 6.3 | OK. Covers the TI land; the toe is 0.25 longer. |

### U1, U2, U3: onsemi MOC3083TVM (6-DIP, 0.4"), `DIP-6_W10.16mm`

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Option / row spacing | "T" = 0.4" lead spacing; lead span 10.16–10.80 (MOC case 646BZ) | rows 10.16 | OK |
| Pinout | 1 anode, 2 cathode, 3 NC, 4 main terminal, 5 substrate (do not connect), 6 main terminal (MOC) | 1 = xx_OPTO_A (from the 150 Ω), 2 = GND, 3 NC, 4 = gate, 5 NC, 6 = to MT2 via 360 Ω | OK |
| Drill vs lead | lead 0.41–0.51 x 0.25–0.36 (diagonal 0.62) | drill 0.8, pad 1.6 | OK |
| **Body / courtyard** | body **8.13–8.89** long x 6.10–6.60 wide (MOC 646BZ) | KiCad fab 7.72 long, courtyard 8.19. Opto centres are **8.695 mm** apart. | **RISK**: at nominal length (8.51) the bodies sit 0.19 mm apart; at maximum (8.89) they interfere by 0.19 mm. DRC cannot see this because the courtyard is undersized. See Should fix 3. |
| Pin 1 | | U1–U3 at 180°: pin 1 is the square pad on the SELV side (x = 35.88) | OK |

### PS1: RECOM RAC03E-05SK/277, `lib/Recom.pretty/RAC03E-K_277`

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Pin positions | Bottom view and recommended footprint (RECOM p.6): pins 1 and 2 at 5.08 spacing, pin 3 at 25.4 from pin 2 (30.48 from pin 1); 5 and 4 at 5.08 spacing on a row 17.78 away, with 4 opposite pin 3 | 1 (0,0), 2 (5.08,0), 3 (30.48,0), 5 (25.4,−17.78), 4 (30.48,−17.78), top view | OK (redrawn correctly) |
| Pin functions | 1 VAC L, 2 VAC N, 3 NC, 4 −Vout, 5 +Vout | 1 = AC_L_FUSED, 2 = AC_N, 3 = none, 4 = GND, 5 = +5V | OK |
| Holes vs pins | pins Ø0.60 ±0.1 | drill 1.0, pad 1.8 | OK |
| Body | 37.0 x 24.0; pin 1 is 3.26 from the side edge and 3.11 from the near long edge | silk/fab rectangle −3.26…33.74 x −20.89…3.11 | OK |
| Documentation | | `lib/README.md` still describes the old, wrong 0 / 2.54 / 25.4 pinout | Doc only: README is stale |

### U4: ESP32-C6-MINI-1U-H4, `lib/Espressif.pretty/ESP32-C6-MINI-1U` (bottom side)

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Pad count | 53 (1–35 perimeter, 36–48 inner GND row, 49 EPAD, 50–53 corners) (ESP Table 3-1) | 53 numbers (pad 49 split into 3 x 3) | OK |
| Pitch / pads | 48 x 0.40 wide x 0.80 long at 0.80 pitch; corners 4 x 0.70; column centres 11.80 / 11.90; row centres 9.80 / 9.90 (ESP Fig. 11-2) | 0.4 x 0.8 at 0.8; columns at ±5.9 (11.8), corners ±5.95 (11.9); rows 9.8 / 9.9 | OK |
| EPAD | 3 x 3 of 1.45, overall 5.40, 0.60 chamfer at pin 1, centre 5.60 from the module's bottom edge (ESP Fig. 11-2) | 1.45 squares at 1.975 pitch (5.40), chamfered custom pad, centre 5.6 from the body edge | OK |
| Body | 13.20 x 12.50 (ESP Fig. 10-2) | fab 13.2 x 12.5; footprint origin 2.05 mm off the body centre | OK (`jlc.py` corrects for the offset; see section 3) |
| Pin 1 / connector | Top view: pin 1 at the top-left; W.FL / MHF III connector 1.70 from the left edge and 1.55 from the top edge, same corner as pin 1 (ESP Fig. 10-2) | Pin 1 is at board (50.70, 10.00). The connector lands at about (49.7, 8.65), facing down (away from the PCB). The nearest bottom-side obstacles are the PS1 pin 4/5 solder joints at y = 3.67, about 5 mm away, and C4 at (53, 11.25). | OK. A pigtail plug can be mated after assembly. Keep PS1 lead clippings short. |
| Pin to function | ESP Table 3-1: 3 = 3V3, 5 = IO2, 6 = IO3, 8 = EN, 12 = IO0, 13 = IO1, 15 = IO6, 16 = IO7, 17 = IO12 (USB D−), 18 = IO13 (USB D+), 22 = IO8, 23 = IO9, 24 = IO18, 25 = IO19, 26 = IO20, 29 = IO23; GND = 1, 2, 11, 14, 36–53 | 3 = +3V3; 5 = V3_SNS; 6 = V5_SNS; 8 = EN; 12 = P_ADC; 13 = NTC; 15 = SW_BREW; 16 = SW_STEAM; 17 = USB_D−; 18 = USB_D+; 22 = IO8_STRAP; 23 = IO9_BOOT; 24 = VALVE_CMD; 25 = PUMP_CMD; 26 = HEATER_CMD; 29 = STATUS; all GND pads on GND; 4, 7, 9, 10, 19–21, 27, 28, 30–35 open | OK. All analog nets are on ADC1 (IO0–IO3); USB is on the native pins. |

### J4: GCT USB4105-GF-A, `USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal` (bottom side)

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Signal pads | 12 x 1.15 long; widths 0.60 (A1/B12, A4/B9, B1/A12, A9/B4) and 0.30 (others); positions ±3.20, ±2.40, ±1.75, ±1.25, ±0.75, ±0.25 (GCT layout) | same | OK |
| Shell slots | 4 x 1.00 pads, 2.10 / 1.80 long; slots 0.60 x 1.70 / 1.40; centres 8.64 apart, rows 4.18 apart | same (±4.32; y −3.105 / +1.075) | OK |
| Locating pegs | 2 x Ø0.65 NPTH at 5.78 (GCT) | Ø0.65 at ±2.89 | OK |
| Pin map | A1/A12/B1/B12 GND, A4/A9/B4/B9 VBUS, A5 CC1, B5 CC2, A6/B6 D+, A7/B7 D−, A8/B8 SBU (GCT pin table) | GND, VBUS, CC1 (R13 5.1k), CC2 (R14 5.1k), D+ and D− paired, SBU open | OK |
| Edge / opening | footprint "PCB Edge" line at y = +3.675, the receptacle face | The face lands at y = 50.975; the board edge is at 51.0. The opening faces the bottom edge. | OK |
| Bottom-side mirroring | | KiCad mirrors the pad pattern exactly as the physical part is mirrored on the underside | OK |

### J5, J6, J7: JST B2B-PH-K / B3B-PH-K, vertical

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Pitch / holes | 2.00 ±0.05; Ø0.7 +0.1/0 (JST p.1) | 2.00; drill 0.75, pad 1.2 x 1.75 | OK |
| Body | 4.5 deep, 1.95 beyond the end pins, (1.7) / (2.8) either side of the pin row (JST) | fab −1.95…(n−1)·2+1.95; across the row −2.16…+2.85 | OK. The connectors sit at x = 53.2; the body reaches x ≈ 56.0, flush with the edge. |
| Pin 1 | No. 1 circuit | square pad 1 | OK. J5: 1 = NTC, 2 = GND. J6: 1 = +5V, 2 = P_SIG, 3 = GND. J7: 1 = BREW, 2 = STEAM, 3 = GND. |
| Plug direction | top entry | vertical, unobstructed | OK |

### D2: SS14 (MDD, LCSC C2480), `D_SMA`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Polarity | band = cathode (SS14) | pad 1 = cathode (template K = 1) = +5V; pad 2 = VBUS. Silk bar on the pad 1 side. | OK |
| Land | pads 1.52 long x 1.68 wide, gap 2.41, overall 5.45 (SS14) | 2.5 x 1.8 at ±2.0: gap 1.5, overall 6.5 | OK. Larger than MDD's (standard KiCad SMA); fine for reflow. |

### D1: KENTO KT-0603G green (LCSC C12624), `LED_0603_1608Metric`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Polarity | KENTO numbers **pin 1 = anode (+), pin 2 = cathode**; the green mark is on the cathode end (KT p.2) | KiCad pad 1 = K = GND; pad 2 = A = STATUS_A (from R12) | OK on the copper. The manufacturer's pin 1 is the opposite of KiCad's pin 1, so a pin-1-based rotation at JLC will flip it. **Check in the JLC preview.** |
| Land | 0.70 x 0.70, gap 0.70 (KT) | 0.875 x 0.95 at ±0.7875 (gap 0.70) | OK |

### C1, C2: Murata GA355QR7GB103KW01L (X2, 2220), `C_2220_5750Metric`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Body | 5.7 ±0.4 x 5.0 ±0.4 (GA355) | fab 5.8 x 5.1 | OK |
| **Land** | Murata reflow Table 1 for GA□55: gap **a = 4.0–4.6**, land length b = 1.4–1.6, land width **c = 3.5–4.8** | KiCad IPC: gap **3.3**, length 1.8, width **5.4** | **WRONG vs Murata.** Pads are 0.7 mm too close and 0.6 mm too wide, giving excess solder and fillet stress on a ceramic safety cap that sits across a mains node (Murata warns that this causes cracking). See Should fix 2. |
| Nets | C1: PUMP_SNUB–PUMP_L; C2: VALVE_SNUB–VALVE_L | | OK |

### R3, R6: 2512 1 W (UNI-ROYAL 25121WF1000T4E), `R_2512_6332Metric`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Land | UNIR §8: gap 5.0, length 1.6, width 3.3, overall 8.2 | gap 4.70, length 1.23, width 3.35, overall 7.16 | RISK (low). Usable, but shorter than recommended. UNI-ROYAL's larger pads also sink heat better from a part that dissipates 1 W. |

### R1, R2, R4, R5: 1206, `R_1206_3216Metric`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Land | UNIR 1206: gap 2.0, length 1.1, width 1.6, overall 4.2 | gap 1.80, 1.12 x 1.75, overall 4.05 | OK |

### 0805 (C3, C6, C7) and 0603 (C4, C5, C8–C11, R7–R25)

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| 0805 land | UNIR 0805: gap 1.0, 1.0 x 1.3, overall 3.0 (generic; Murata's GRM21 sheet not fetched) | gap 0.9, 1.0 x 1.45, overall 2.9 | OK (IPC-7351 nominal) |
| 0603 R land | UNIR 0603: gap 0.9, 0.65 x 0.8, overall 2.1 | gap 0.85, 0.8 x 0.95, overall 2.45 | OK |
| 0603 C land | (generic) | gap 0.65, 0.9 x 0.95 | OK (IPC nominal) |
| Polarity | none | | n/a |

### TP1, TP2: `TestPoint_Pad_D1.5mm`, bottom side

| Checked | Datasheet | Board | Verdict |
|---|---|---|---|
| Pad | bare Ø1.5 pad, no part | TP1 = IO9_BOOT (36.25, 19.0), TP2 = GND (36.0, 9.0) | OK. The pads are 10 mm apart, so shorting them needs a wire or tweezers; that is a usability note, not an error. |

## 2. Net cross-check (board against netlist)

All 175 netlist pins match the board pads, with one exception: **J1 pads 1 and 2 are swapped** (deliberately, via `plan.json` "swap"). See Must fix 1.

## 3. Bottom-side placement and the JLC position file

Correct as laid out:
- KiCad mirrors every flipped footprint, which is exactly what physically happens to a part on the underside.
- The ESP module's antenna connector faces away from the board and is reachable.
- The USB-C opening faces the bottom edge, flush with it.
- No polarized part is mirrored wrongly.

Positions in `coffee-v13-cpl.csv`:
- They are KiCad footprint origins, except U4.
- For U4, `jlc.py` substitutes the courtyard centre (44.80, −13.35). That equals the body centre: body y spans 7.10…19.60 on the board, centre 13.35. **OK.**
- Every other footprint has its origin at the body centre (checked SOT-223, SMA, USB4105, 0603/0805/1206/2512/2220 and LED). **OK.**

Rotations are the raw KiCad orientations from `kicad-cli pcb export pos`.
- KiCad-to-JLC tools disagree on bottom-side rotation. Some pass the value through, as here. The kicad-jlcpcb-tools plugin uses 180 − rot. For a part at 0° or 180° the two answers differ by exactly 180°.
- JLC's own library zero orientations also differ from KiCad for some packages, SOT-223 especially (commonly +180°).
- In this design every polarized bottom part sits at 0° or 180°, so **every polarized bottom part has to be checked visually**. The parts at 90° (C3, C4, C6–C8, C11, R3–R5, R10, R11, R14, R16–R19, R24) are all non-polarized, so a 180° error there is harmless.

## Must fix before fab

1. **J1 mains polarity: the board, the documentation and the silkscreen disagree.**
   - Board: pad 1 = AC_N, pad 2 = AC_L.
   - Schematic, BOM note (`bom-120v.csv`: "1=L 2=N") and `docs/INSTALL.md` §J1 ("1 = L, 2 = N"): the reverse.
   - `docs/PARTS.md` claims the swap is silk-labelled, but there is no L/N text anywhere on the board.
   - Wired per INSTALL.md, the T500mA fuse F1 and the triacs Q1, Q2 and Q3 end up in neutral. The heater, pump and valve would then stay live with respect to earth when "off", and the PSU branch would be unfused against L-to-earth faults.
   - Fix: either add "N" / "L" silkscreen beside J1 pads 1 and 2 and correct the BOM note, INSTALL.md and controller.py, or remove J1 from `plan.json` "swap" and re-route. The silkscreen is mandatory either way, because this is the field-wiring point. (Per the immutable-versions rule, this means a new versions/vN.)

## Should fix (strongly recommended, cheap)

2. **C1, C2 (X2 2220) land pattern.** Replace the IPC 2220 pads with Murata's GA□55 reflow land: about 1.5 long x 4.2 wide, gap 4.3 (pad centres at ±2.9). The current pads are 0.7 mm closer and 0.6 mm wider than Murata allows.
3. **Opto bodies U1–U3.** The KiCad DIP-6 fab/courtyard (7.72 / 8.19 long) is shorter than the onsemi body (8.13–8.89). At 8.695 mm centre spacing, maximum-tolerance bodies collide by about 0.2 mm, and DRC cannot see it. Either increase the spacing to at least 9.2 mm or accept a hand-fit risk. A local footprint with a correct 8.9 x 6.6 body would make DRC catch this.
4. **R3, R6 (2512, 1 W).** Optionally enlarge to UNI-ROYAL's pads (1.6 x 3.3, gap 5.0) for better solder joints and heat spreading.
5. **TO-220 drill.** 1.1 is marginal against the ST worst-case lead diagonal (1.12); 1.2 removes the risk.
6. **Documentation.** `lib/README.md` still describes the old, wrong RAC03E pinout (0 / 2.54 / 25.4).

## Check in JLC preview

For each part, confirm in JLC's placement preview that the part sits on its pads as described below. Coordinates are board top-view, KiCad y down; JLC shows the bottom side mirrored.

| Ref | CPL rot | What must be true | Likely issue |
|---|---|---|---|
| **Q3** Z0109NN (SOT-223) | 180 | Tab (A2) on the large pad at x = 14.6, the mains side. Pins at x = 20.9; pin 1 (A1) at y = 25.7, gate at y = 30.3. | SOT-223 is the classic +180° KiCad-to-JLC offset. Expect it to need flipping. |
| **U5** TLV1117LV33 (SOT-223) | 0 | Tab (OUT) on the large pad at x = 47.15. Pin 1 GND at y = 39.05, pin 3 IN at y = 34.45. | Same +180° SOT-223 offset |
| **D2** SS14 (SMA) | 180 | Cathode band on the x = 41.25 pad (+5V side, toward C6/PS1), anode toward x = 37.25 (VBUS) | Bottom-side 0/180 ambiguity |
| **D1** KT-0603G (LED) | 0 | Green cathode mark on the x = 35.21 pad (GND); anode at x = 36.79 (R12 side) | The manufacturer's pin 1 is the anode, opposite to KiCad. High chance of a 180° flip. |
| **J4** USB4105-GF-A | 180 | Receptacle opening at the board edge (y = 51); signal pads toward the board centre (y = 43.6); centre (42.55, 47.30) | The JLC USB-C model's zero orientation and origin differ by library. A wrong result shows the shell hanging inward. |
| **U4** ESP32-C6-MINI-1U | 180 | Pin 1 / chamfered EPAD corner at (50.7, 10.0); antenna connector in that same corner, toward the top edge (y = 0); pins 12–24 row toward the board centre (y = 18.9). Placement point (44.80, −13.35) is the body centre. | A 180° error swaps the whole module, so check pin 1. |
| C1, C2 (2220) | 0 | Long axis of the pads along x | non-polar. Check only for a 90° error. |
| All R/C | 0/90 | Lying across their pads | non-polar |
