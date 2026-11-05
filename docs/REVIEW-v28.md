# Independent pre-order review: controller board v28

Reviewed 2026-11-04 against:
- `hardware/controller/controller.py` and `build/parts-120v.json`;
- `build/coffee-controller-120v.kicad_pcb`, byte-identical to `versions/v28-thin/board.kicad_pcb`;
- `build/coffee-v28-{bom,cpl}.csv`, identical to `build/jlc-120v/{bom,cpl}.csv`;
- `build/coffee-v28-gerbers.zip`: every file matches `build/coffee-controller-120v-fab/` byte for byte;
- `sim/run.py` and `sim/REPORT.md`;
- the firmware (`main.rs`, `control.rs`, `bridge.rs`, `scale.rs`) and the docs.

**What I re-ran myself, without editing any repo file:**
- **KiCad 7 DRC** on a copy of the board (`pcbnew.WriteDRCReport`, all track errors): 0 unconnected pads, 0 footprint errors, and the only 65 violations are `lib_footprint_issues` warnings.
- **`jlc_cpl.py`**, into a temp directory. It reproduces `coffee-v28-cpl.csv` exactly, and every part's JLC pads land on the board pads.
- **`jlc_pins.py`**: 102 pin functions, 0 mismatches.
- **Board ↔ netlist comparison:** every pad on every footprint carries the net `parts-120v.json` gives it, and no part is missing or extra.

Datasheets: I used local copies in `docs/evidence/pdf/` (Espressif MINI-1/1U v1.5, ESP32-C6 v1.5, Kento KT-0603R, JST GH), and the JLC help pages cited where they're used. No CAPTCHA pages came up.

**Verdict in short:** no blocker. One should-fix concerns how JLC will hold the board during assembly: parts sit right at both long edges. The rest are minor or docs.

---

## Blocker

None found.

## Should-fix

**S1. Parts sit within 2.5 mm of both long edges, and the USB-C overhangs the short edge. JLC will have to add edge rails, so decide how before paying.**
- **What's wrong:**
  - JLC's assembly terms say "the distance between the body of the components and the edge of the board must be equal or greater than 2.5mm", and that boards under 70 × 70 mm get rails at JLC's choice of method.
  - At the top edge, the bodies of J1–J10 are 0.70 mm in and their MP pads 0.50 mm.
  - At the bottom edge:
    - U4's body is 0.50 mm in;
    - U5's pads are 0.60 mm in;
    - the pads of C4, C5 and R10 are 0.575 mm in, so their bodies are about 0.7 mm in.
  - J4's shell sticks out 0.46 mm past the left edge, and its courtyard 0.96 mm.
  - So JLC will panelize the board or add rails itself (extra cost), and the method matters:
    - **V-cut:** JLC needs 0.4 mm from copper to a V-cut line. The GND pour on both layers stops only 0.3 mm from the edge (zone clearance 0.3, 0.3 mm edge keep-outs).
    - **MLCC cracking:** C4 (3V3 decoupling) and C5 (EN delay) are 0603 MLCCs set perpendicular to the bottom edge, 0.6 mm from it. Snapping a V-score there is the classic way to crack an MLCC. A cracked C4 can later fail short and pull the 3V3 rail down.
    - **USB-C overhang:** a rail or tab on the left short edge would collide with it.
- **Evidence:**
  - pad and courtyard distances from `pcbnew` on `build/coffee-controller-120v.kicad_pcb`: J* MP pads at y = 0.50 mm; U4 Fab outline to y = 19.15 on a 19.6 mm board; J4 Fab outline from x = −0.46;
  - JLC assembly terms, §2 item 3 and §5 items 5/14: <https://jlcpcb.com/help/article/terms-and-conditions-of-jlcpcb-assembly-service>;
  - JLC capabilities, trace to V-cut line 0.4 mm: <https://jlcpcb.com/capabilities/Capabilities>;
  - JLC edge rails ≥ 5 mm: <https://jlcpcb.com/help/article/how-to-add-edge-rails-fiducials-for-pcb-assembly-order>.
- **Fix (no re-route needed for either option):**
  - **Option 1:** order with rails added by JLC on the **two long edges only**, joined by **routed tabs / mouse bites, not V-cut**. Say so in the order remark ("USB-C overhangs the left edge; no rail or tab on the short edges; please no V-cut, mouse-bite tabs, components at the long edges"). Then check the panel drawing JLC sends back before confirming.
  - **Option 2:** if JLC insists on V-cut, pull the GND pour back to ≥ 0.5 mm from the long edges, and move C4/C5 ≥ 1 mm from the bottom edge, set parallel to it, using the layout tools rather than hand-routing. That's a new `versions/v29` per the board-version rule.

## Minor

**M1. The 3V3 pin's nearest decoupler is far from it in copper (v23 M3, still open).**
- **Path to C4:** module pad 3 is at (43.48, 14.60). The path to C4 (100 nF, at 49.0/17.8) runs pad 3 → (45.27, 14.6) → (46.0, 15.3) → (45.1, 16.2) → via (45.01, 17.57) → B.Cu → via (49.85, 17.87) → C4: about 11 mm and two vias.
- **Path to bulk:** C3/C7 sit about 30 mm of 0.4 mm track away, by the LDO.
- **Espressif:** Fig. 9-1 (p40) puts 22 µF + 0.1 µF at the pin.
- **Why it's minor:** the module has its own internal decoupling, and the estimated ripple is small (≈ 40 nH against ≥ 10 µF, so Z0 ≈ 0.06 Ω, about 20 mV on a 0.3 A Wi-Fi step).
- **Docs:** the BOM note "C4 at the module's top-left corner" overstates how close it is.
- **Fix:** next layout, put C4 (and ideally a 10–22 µF 0805) right at pad 3, on F.Cu with no via.

**M2. LDO heat sinking and pad connections.**
- **LDO tab:**
  - U5's tab (+3V3, pad 2 at 14.0/11.7) has only its own 3.8 × 2 mm pad plus 0.4 mm tracks; no 3V3 copper area.
  - The datasheet's RθJA of 62.9 °C/W is for a JEDEC high-K board. On this layout expect roughly 120–150 °C/W.
  - At about 0.2 W average (C6, LEDs, dividers) that's +25…30 °C: Tj ≈ 100 °C at a 70 °C ambient, under 125 °C but not generous.
  - There's no thermal-relief question for the tab, because it isn't on a pour.
- **GND pads:**
  - The GND pour connects pads **solid** (zone pad connection = full). Every GND pad of the 0603 parts and LEDs and the USB-C shell legs are tied straight to both pours.
  - This is fine for JLC reflow most of the time. It slightly raises the tombstoning risk on 0603s with one end on the plane, and makes rework and USB-shell hole fill harder.
- **Fix:** next layout, add a +3V3 pour around the tab, and consider thermal spokes for the GND pour on small passives.

**M3. Signal tracks and vias run under the module (tented, DRC clean).**
- **What runs there:** 13 nets have F.Cu track under U4's body. Examples: USB_D− (10 mm), STATUS_A (8.6 mm, with vias at (36.39, 15.46) and (41.23, 11.33)), and +3V3 along y = 18.56 under the antenna end. There are also 13 signal vias.
- **Why it's acceptable:**
  - vias are tented (`viasonmask false`, and the F_Mask gerber has no via openings);
  - KiCad DRC holds ≥ 0.15 mm to every module pad;
  - the module's underside is solder-masked except its pads (MINI-1U datasheet Fig. 10-2, p41).
- **The only risk:** scratched solder mask.
- **Fix:** none for this order.

**M4. Stale comments and docs that now contradict the board.**
- **`firmware/esp32/src/main.rs` pin-map table (lines 34–52):** it says:
  - "Status LED 23" and "Panel LEDs 21/22/16", where the code and netlist have status on 16 and steam on 23;
  - "4.7k pull-up" (now 10k);
  - "10k/20k" pressure (47k/100k);
  - relay / zero-cross SSR (both G3MB boards);
  - "100R" on SCK (removed).

  The code itself is right (see V3). The table would mislead someone probing the board.
- **`controller.py`:**
  - the docstring says "v7" and "the 3-way valve by a relay module";
  - the "One connector type for every cable … JST GH 3-pin" comment is out of date;
  - the R27 comment says "every cable uses the same GH 3-pin plug";
  - the scale comment says "100R series on every line";
  - the R36 comment mentions an "R33" that doesn't exist;
  - "Every module has its own input LED, so the board has none per output" is no longer true, now that D6–D8 exist;
  - the `PROGHDR` and `HOLE` templates are unused.
- **`docs/WIRING.md:110`:** still says "Every board connector is a JST GH 3-pin header" (J1 is 2-pin; J7 and J10 are 7- and 6-pin).
- **`docs/CIRCUIT.md:20`:** says "61/61 pass"; `sim/REPORT.md` and CIRCUIT.md:140 say 54/54.
- **Fix:** edit the text. No hardware change.

**M5. The sims don't include the six indicator LEDs.**
- **What's missing:** `sim/run.py`'s `output()` and its reset check model the 3.3k pull-down alone.
- **Hand check:** at reset, the SDIO pins' WPU (45 kΩ typ, C6 Table 2-2) against 3.3k gives 0.23 V at the gate. A red LED + 1k draws nothing until ~1.5 V, so D6–D8 change neither the boot state nor the margin. When on, a gate pin sources 1.0 mA (pull-down) + ~1.4 mA (LED) = 2.4 mA.
- **Fix:** add the LED branch to the model so the report covers it.

**M6. The silkscreen labels "S P C S H P V" use S and P twice each** (status/steam, power/pump).
- **Fix:** cosmetic. Use e.g. "St Pw Cf Sm Ht Pm Vl", or a legend line, next time.

**M7. VBUS sees about 32 µF behind D2:** C6 22 µF, then the LDO, C7 22 µF and C3 10 µF. USB 2.0 allows 10 µF at attach.
- **Effect:** hosts tolerate this in practice, and it's bench-only.
- **Fix:** none needed.

**M8. Check these in JLC's placement preview, as always.**
- **CPL fits:** each part has a unique angle, with residuals of J4 0.163 mm (pegs weighted) and Q4 0.283 mm (JLC's longer SOT-23 land, tolerance 0.35).
- **Look at:**
  - D1/D3–D8: cathode mark toward the GND pad (see V6);
  - U4: pin-1 chamfer at the bottom-right;
  - U5: tab toward the connector row;
  - J4: shell flush with the left edge;
  - the GH latches facing the top edge.
- **JST headers:** EasyEDA names the package of the JST 3-pin and 6-pin headers "CONN-TH_…". Confirm JLC lists them as SMT in the BOM step, so no THT handling or fee is added.

**M9. v23 items consciously accepted and unchanged:**
- M4 (no OVP on +5V);
- M5 (no ESD arrays, no USB series resistors);
- M6 (+3V3 straight out on J10.2);
- M7 (cold-room NTC fault);
- M10 (85 °C parts);
- the optional half of S5 (no 1k series on DOUT).

All still reasonable.

## Verified OK

| # | Item | Evidence |
|---|---|---|
| V1 | **v23 S1 (supply/output swap) is fixed.** J1 is now a 2-pin GH (SM02B-GHS-TB, C189893). No 3-pin output lead can mate it, so an SSR can no longer be held on through Q4. I also checked the reverse mistake, a 2-pin supply plug forced into a 3-pin socket at either offset. In J2/J3/J8 it either doesn't power the board (+ on OUT, FET off) or powers it through Q1's body diode, with no load connected. In J5 it limits to ~1.4 mA through R27; in J6, ≤ 47k into the ADC. Nothing latches on | netlist; AO3400A/AO3401A datasheets p1 |
| V2 | **v23 S2 (switch noise margin) is fixed.** R28/R20/R21 = 10k on the GPIO side of the 1k. Closed: 3.3 × 1k / (1k + 10k ∥ 45k) = 0.36 V, against VIL max 0.825 V, so 0.46 V margin and 0.36 mA wetting current. The sim agrees (0.359 V) | MINI-1U datasheet v1.5 Table 6-3; `sim/REPORT.md` |
| V3 | **Firmware pins match the schematic exactly.** `control.rs`/`main.rs`: GPIO0 pressure, 1 NTC, 2 V3_SNS, 3 V5_SNS, 18 valve, 19 pump, 20 heater, 6 brew, 7 steam, 14 power, **16 status LED**, 21/22/**23** power/coffee/steam lamps, 17 SCK, 4/5 DOUT. `bridge.rs` drives 18/19/20 low and uses 16 as the status LED. All agree with `FIRMWARE_PINS` and with the board pads: pad 31 = TXD0 = STATUS, pad 29 = IO23 = LED_STEAM_IO, pad 30 = RXD0 = SCALE_SCK. Outputs are created Low before anything else. DOUT pins have `Pull::Up` (v23 S5) | `control.rs:83-99`, `main.rs:142-158`, `bridge.rs:29-32`; pcbnew pad dump |
| V4 | **Status LED on U0TXD (v23 M1 fixed).** The steam lamp is off GPIO16, so it no longer lights at boot. D1 glows while UART0 idles high in ROM/download mode, as documented. U0TXD/U0RXD aren't strapping pins. Before firmware takes the pin, `esp-println`'s `auto` backend may print on UART0 when USB isn't attached, which only flickers D1 (cosmetic). Once `Output::new` re-muxes GPIO16, UART output no longer reaches the pad | C6 datasheet v1.5 Table 2-2 (U0TXD "WPU, output"); esp-println 0.18 `auto` feature |
| V5 | **Indicator LEDs on the gate nets don't affect the boot state.** GPIO18–23 are SDIO pins: WPU only *at* reset, IE after. Against 3.3k the gate sits at 0.23 V (0.47 V even with a 20 kΩ pull-up), under the AO3400A's Vth min of 0.65 V. The LED + 1k branch conducts nothing below ~1.5 V. **GPIO current:** gate pins 2.4 mA, lamp pins 3.9 mA (330 Ω) + 1.3 mA (indicator) = 5.2 mA, or 11.3 mA with a lamp lead shorted. Total ≈ 24 mA with everything on, within the C6's 20 mA per pin at default drive. KT-0603R at 1.3 mA is far below its 25 mA / 40 mW | C6 Table 2-2; AO3400A p1; Kento KT-0603R spec p3 |
| V6 | **LED polarity in the CPL (C2286).** The Kento drawing marks pad ① "+" (anode) and ② cathode, with the green mark at the ② end. JLC's EasyEDA symbol has pin 1 = A, pin 2 = K. KiCad's LED_0603 has pad 1 = K, which goes to GND on all seven LEDs. So `PAD_MAP {"1":"2","2":"1"}` is correct, and `jlc_pins.py` confirms it with 0 mismatches | `docs/evidence/pdf/kento_kt-0603r.pdf` p2; `build/easyeda-cache/C2286.json` |
| V7 | **USB-C (HRO TYPE-C-31-M-12, C165948), v23 S4 fixed.** CC1 = A5 and CC2 = B5, each with its own 5.1k (R13/R14) to GND. DP = A6/B6 → GPIO13, DN = A7/B7 → GPIO12, as the C6 USB pins require. VBUS A4/A9/B4/B9 → D2 → +5V; GND A1/A12/B1/B12 and the shell → GND; SBU A8/B8 left open. Shell slots: 0.6 × 1.7 and 0.6 × 1.2 mm plated; pegs 0.6 mm NPTH, ≥ 1.34 mm from the edge. The shell front sits 0.46 mm past the left edge, normal for this part (but see S1) | netlist; pcbnew pad dump; C6 datasheet pin list (GPIO12 = USB_D−, GPIO13 = USB_D+) |
| V8 | **Connector orientation.** For all eight GH headers, the signal pads are at y = 5.05 (inboard) and the MP reinforcement tabs at y = 1.85, with the body front 0.70 mm from the top edge. That's the side-entry pattern with the mating face toward the edge, so openings face out past the top edge. Pin 1 matches the netlist on every connector. Adjacent bodies sit about 0.8 mm apart, and the GHR plugs are narrower than the headers | JST GH catalogue; pcbnew dump |
| V9 | **Edge copper.** No pad is closer than 0.50 mm to an edge (J1/J5 MP pads, U5 0.60, C4/C5/R10 0.575). The pour sits at 0.3 mm, with 0.3 mm track/via keep-outs on both layers. DRC `copper_edge_clearance` = 0.3 mm passes. This meets JLC's routed-edge requirement; only the V-cut case is a problem (S1) | DRC; zone dump |
| V10 | **Module antenna and W.FL.** The MINI-1U has no keep-out zone. Its connector is at the pin-1 end, 1.70 / 1.55 mm from the corner, which with the 180° rotation puts it at about (42.5, 17.5): 2 mm from the bottom edge, at the end away from the connector row. The nearest parts are R10 (3.5 mm) and C5 (5 mm), both 0603, so the plug presses straight down with the pigtail clear. The module body sits 1.75 mm from J10's body, with the courtyards abutting | MINI-1U datasheet v1.5 §3 (p10, "no keepout"), Fig. 10-2 (p41), §10.2 |
| V11 | **Courtyards, silk, DRC.** No courtyard overlaps; no silk-over-copper or silk-to-edge warnings; minimum text height is met; no starved thermals; no unconnected items; vias are tented | DRC on a copy of the board |
| V12 | **Fab and order files are consistent.** The gerber zip equals the fab directory. The board equals `versions/v28-thin/board.kicad_pcb`. The BOM covers all 64 placed parts; TP1 (bottom bare pad) is correctly left out. Every LCSC part matches its value and package: C14663 100 nF/50 V X7R, C15849 1 µF/50 V X5R, C21122 22 nF/50 V X7R, C45783 22 µF/25 V, C15850 10 µF/25 V, 0603WAF resistors, C2286 KT-0603R, C189893 SM02B-GHS-TB, C514175/C495552/C133065 SM03B/SM07B/SM06B-GHS-TB, C165948 TYPE-C-31-M-12, C20917/C15127 AO3400A/AO3401A, C8598 B5819W, C15578 TLV1117LV33DCYR, C20627095 ESP32-C6-MINI-1U-H4. All are in stock | `build/jlc-120v/report.md`; `cmp` |
| V13 | **Strapping and EN unchanged from v23 (still correct).** GPIO8 has R11 10k up. GPIO9 has its internal pull-up plus TP1. GPIO15 is NC. GPIO4/5 only select SDIO edges, so the DOUT pull-ups after boot are harmless. EN is 10k + 1 µF | MINI-1U datasheet §4; C6 datasheet "Strapping pins" |
| V14 | **Power path unchanged (v23 V7–V9).** Q4: G = GND, S = +5V, D = +5V_IN. U5: 1 GND, 2 + tab OUT, 3 IN. D2 pad 1 = K = +5V. All confirmed again on the routed board | pcbnew pad dump |

---

**Verdict: ready to order, provided S1 is handled at checkout.** Ask JLC for rails on the long edges only, joined with mouse-bite tabs rather than V-cut, and confirm their panel drawing. I found no electrical, netlist, footprint, polarity or firmware-pin error, and every v23 should-fix is fixed or consciously accepted.
