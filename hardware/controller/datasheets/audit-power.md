# Power-path datasheet audit (PS1, U5, D2, F1, RV1, C3/C6/C7, 5 V / 3.3 V budget)

Audited 2026-11-03 against `controller.py`, `docs/CIRCUIT.md`, `docs/HARDWARE.md`, `sim/run.py` (+ `sim/REPORT.md`),
`lib/Recom.pretty/RAC03E-K_277.kicad_mod`, `lib/README.md`, the KiCad 7 stock footprints, `plan.json` and
`build/jlc-120v/`. The design target is **85 °C ambient** inside the machine. No code or docs were changed.

Datasheets used:

| Part | Datasheet (revision) |
|---|---|
| RECOM RAC03E-05SK/277 | https://recom-power.com/pdf/Powerline_AC-DC/RAC03E-K_277.pdf (REV 3/2023, 6 pp.) |
| TI TLV1117LV33 | https://www.ti.com/lit/ds/symlink/tlv1117lv.pdf (SBVS160C, revised Jan 2023) |
| Espressif ESP32-C6-MINI-1/1U | https://www.espressif.com/sites/default/files/documentation/esp32-c6-mini-1_mini-1u_datasheet_en.pdf (v1.5) |
| Murata GRM21BZ71A226ME15 | https://www.murata.com/en-us/products/productdetail?partno=GRM21BZ71A226ME15%23 (I read Murata's own generated spec sheet dated 2020/10/22, from a distributor mirror, because murata.com served no content to the fetcher) |
| Littelfuse 372 (TR5, time-lag) | https://www.littelfuse.com/~/media/electronics/datasheets/fuses/littelfuse_fuse_372_datasheet.pdf.pdf (the copy I read is rev. 12/19/13, from a mirror; the official link is the one in the KiCad footprint) |
| Bourns MOV-10D series | https://www.bourns.com/docs/Product-Datasheets/mov10d.pdf (REV 07/25) |
| SS14 (no maker given in `controller.py`; JLC places LCSC C2480) | Reference: Vishay https://www.vishay.com/docs/88746/ss12.pdf (rev. 23-Apr-2020). The C2480 maker's sheet was not checked (**UNVERIFIED**) |

---

## 1. PS1: RECOM RAC03E-05SK/277

### 1a. Footprint `lib/Recom.pretty/RAC03E-K_277.kicad_mod` vs the datasheet mechanical drawing (p. 6)

I rendered page 6 at 200 dpi and measured it. The scale is 37.0 mm body = 408 px (11.0 px/mm). The 2.54 mm grid of the "Recommended Footprint Details" was a second check.

| Item | Footprint / lib/README.md | Datasheet p. 6 | Verdict |
|---|---|---|---|
| Pin 1 position | (0, 0) | origin | OK |
| **Pin 2 position** | (2.54, 0): "pins 1–3 at 0 / 2.54 / 25.4 mm" | **5.08 mm from pin 1.** Bottom view: pin 1→2 = 56 px = 5.08 mm. Recommended footprint: two grid squares. The "10x 2.54 = 25.4" dimension runs from **pin 2** to pin 3, not from pin 1 | **WRONG** |
| **Pin 3 position** | (25.4, 0) | **30.48 mm** from pin 1 (5.08 + 25.4) | **WRONG** |
| **Pin 4 position** | (25.4, −17.78) | **(30.48, 17.78)**: directly opposite pin 3 ("7x 2.54 = 17.78" row spacing) | **WRONG** (x off by 5.08) |
| **Pin 5 position** | (20.32, −17.78) | **(25.40, 17.78)**: 5.08 mm from pin 4 toward pin 1 (labelled "5.08") | **WRONG** (x off by 5.08) |
| Row spacing | 17.78 | 7 × 2.54 = 17.78 | OK |
| Side of pin 5 relative to pin 4 (top view) | 5 nearer pin 1 | 5 nearer pin 1 | OK |
| Body 37.0 × 24.0 mm; edge 3.26 mm beyond pin 1, 3.11 mm beyond the 1–3 row | −3.26…33.74, −20.89…3.11 | 37.0 × 24.0, 3.26, 3.11 | OK. With the real pin 3 at 30.48 the body is symmetric (3.26 mm each side). In the current footprint pin 3 sits 8.3 mm from the right edge, so the pins and body don't agree with each other |
| Height / pin length | — | 15.4 mm body; pins 4.0 ± 1.0 mm long | info: up to 3.4 mm of pin sticks out below a 1.6 mm board |
| Pin diameter / drill / pad | drill 1.0, pad 1.8 | Ø0.60 ± 0.1 | OK (0.3–0.5 mm clearance) |
| Pin 3 (NC) treatment | NC, not on a mains net; in `plan.json` it lands on the SELV side | Hatched "Restricted area: pin [3] be considered secondary side referenced" | OK, keep it that way: no primary copper near pin 3, and it counts as SELV for creepage |
| L–N pad gap (pins 1–2) | 2.54 − 1.8 = **0.74 mm**, exempted by the "inside one part's courtyard" lint rule | real gap 5.08 − 1.8 = 3.28 mm | WRONG footprint hides this. The lint exemption let a 0.74 mm L–N gap through |
| Pin functions 1 L, 2 N, 3 NC, 4 −Vout, 5 +Vout | `controller.py` PSU template and the nets (AC_L_FUSED, AC_N, NC, GND, +5V) | Pinning table p. 6: 1 VAC in (L), 2 VAC in (N), 3 NC, 4 −Vout, 5 +Vout | OK |

**Effect on the layout.** `plan.json` puts PS1's origin at x = 16.6 with the band at x 20.3–27.3. With the real pinout, pin 2 sits at x = 21.68 and its pad reaches x = 20.78, **inside the copper-free band**. PS1 has to move left by at least 1.4 mm (pin 1 at x ≤ 14.3), or the band has to move. Pins 5 and 4 then land at x ≈ 40 / 45 (y ≈ 3.7). Their 1.8 mm pads come within about 1 mm of the bottom-side ESP32 module U4's outline, so re-check that clearance and hand-solder access.

### 1b. Electrical, thermal and safety claims

| Claim | Where | Datasheet value (page) | Verdict |
|---|---|---|---|
| 3 W, 5 V, 0.6 A | controller.py, HARDWARE | 5 V / 600 mA, 73 % eff. (p. 1, Selection Guide) | OK |
| −40…+85 °C, full load to 75 °C | controller.py, CIRCUIT, HARDWARE | −40…+85 °C at natural convection 0.1 m/s; 100 % to 75 °C (115–240 VAC nom.), derated to **1.8 W (60 %) at 85 °C** (p. 1 description, p. 4 derating graph). 100 VAC nom.: 100 % only to 65 °C | OK, but **at the 85 °C target only 1.8 W (0.36 A) is available continuously** |
| Max case temperature | not mentioned | **+95 °C** (p. 4) | RISK: at 85 °C ambient the case may rise only 10 °C. A closed printed enclosure with no airflow makes the module's local ambient higher than the machine's interior |
| 4 kVAC reinforced | all | I/P–O/P 4 kVAC 1 min; insulation grade reinforced (p. 3/4) | OK |
| EN60335-1, IEC61558, UL62368 | controller.py, HARDWARE | EN60335-1:2012+A2:2019; IEC/EN61558-1, -2-16; UL/CSA/IEC62368-1 (p. 5) | OK |
| Internal input fuse | all | "Input Fuse: internal, fusible resistor" (p. 3) | OK |
| EMC class B with no external parts | controller.py, HARDWARE | EN55032 class B "without external components", floating output (p. 1, p. 5) | OK |
| Minimum load | — | 0 % (p. 2); below 10 % load the specs may not be met (note 5) | OK |
| Max capacitive load | — | **Not stated** in REV 3/2023 | **UNVERIFIED**. The board puts ≈ 10 µF effective on +5 V, which is tiny, so low risk |
| Input current / inrush | — | 70 mA at 115 VAC; **inrush 10 A at 115 VAC**, cold start 25 °C; duration not given (p. 1) | used in F1 below |
| Load-step response | sim models the PSU as 5 V behind **0.15 Ω** | **10 % load step: 6.0 % max deviation, 350 µs recovery** (p. 3) | **RISK / sim invalid**. A Wi-Fi TX burst is a ≈ 0.4 A step, ≈ 65 % of rated load. The datasheet allows ≥ 0.3 V dip for only a 60 mA step. The resistive model cannot show a 350 µs control-loop recovery, so "5 V ≥ 4.52 V" is not supported |
| Output ripple | — | 120 mVp-p, measured with **0.1 µF MLCC + 10 µF low-ESR e-cap** (p. 2, note 4) | OK. C6 gives ≈ 10 µF effective at 5 V (see §6) |
| Surge immunity | — | IEC 61000-4-5 AC port ±0.5 / 1 kV criterion A, with no external parts (p. 5); OVC III (p. 3) | OK. The module doesn't need RV1 for itself |
| Leakage | — | 0.25 mA max at 277 VAC (p. 4) | OK |
| "Design life >30,000 h at 50 °C full load; we run at a quarter load" | HARDWARE | > 30 × 10³ h at 230 VAC / 60 Hz, full load, +50 °C (p. 4). Nothing given at 70–85 °C | **UNVERIFIED** at 70–85 °C. A ×2 per 10 °C rule gives roughly 2.7 kh at 85 °C full load; lighter load helps an unknown amount. Ask RECOM for lifetime at Ta 85 °C / 25 % load, or treat the module as a wear part |
| "Rated to 85 °C: the case runs at 60–70 °C" | controller.py notes | — | OK at 70 °C. At the 85 °C target it holds only with the 1.8 W limit and the 95 °C case limit |

---

## 2. U5: TI TLV1117LV33 (TLV1117LV33DCYR, SOT-223 "DCY")

| Claim | Where | Datasheet value (page/section) | Verdict |
|---|---|---|---|
| Pinout 1 GND, 2 OUT, 3 IN ("AMS1117 pinout") | controller.py LDO template | Table 5-1 (p. 3): IN = 3, **OUT = 2 and tab**, GND = 1 | OK |
| Footprint `SOT-223-3_TabPin2` | controller.py | KiCad: pads 1, 2, 3 at 2.3 mm pitch, tab pad numbered 2 → tab = OUT | OK |
| "Tab pour is the heatsink" | controller.py notes, CIRCUIT ("on a copper pour") | Tab = OUT, so the heatsink copper has to be **+3V3**. The current `build/coffee-controller-120v.kicad_pcb` has **no +3V3 zone** (zones: GND ×3, HTR_A, HTR_B, keep-outs). The tab gets only routed track | **WRONG** (the claim isn't implemented) |
| Stable with ceramic caps, 0 Ω ESR | controller.py, CIRCUIT, HARDWARE | §8.2.2.1 (p. 13): ≥ 1.0 µF ceramic required; **effective C_OUT > 0.5 µF** after bias/temp/ageing; X5R/X7R recommended; no maximum stated; "do not use an electrolytic output capacitor" (§8.4) | OK. Effective C_OUT ≈ 20 µF (§6) |
| Input cap | — | not needed for stability; 0.1–1 µF close to IN recommended; needed if source impedance > 2 Ω (p. 13–14) | OK: C6 22 µF about 5 mm from U5 |
| 455 mV dropout at 1 A | controller.py comment | **typ.** 455 mV at 1 A for V_OUT ≥ 3.3 V, **max 700 mV** (§6.5 p. 5); 230 mV typ. at 500 mV; Fig. 6-5: ≈ 250 mV at 0.5 A, 125 °C | OK as a typ. figure. The max is 1.5× higher |
| Sim dropout model 0.15 V + 0.3 Ω·I (0.30 V at 0.5 A) | sim/run.py | typ. 0.23–0.25 V at 0.5 A (25–125 °C); max at 0.5 A not specified (scaling the 1 A max gives ≈ 0.35–0.39 V) | OK-ish (a fair typ. model, a bit optimistic against max) |
| V_IN range | — | Recommended **2–5.5 V**; **abs. max 6 V** (§6.1, §6.3) | OK in normal use (PSU ±1 %; USB through D2 < 5.3 V). RISK: RECOM OVP is 120–260 % (6–13 V, p. 3), so a PSU regulation fault would destroy U5, U4 and the transducer |
| 125 °C junction | HARDWARE | operating T_J −40…125 °C (§6.5); TSD 165 °C | OK |
| "0.2 W average (junction ~80 °C)" | HARDWARE | R_θJA **62.9 °C/W** (JEDEC high-K board, §6.4) | OK at 70 °C ambient (≈ 83 °C). **At 85 °C ambient: ≈ 98 °C** with JEDEC copper. With only track on the tab (no pour), R_θJA is probably ≈ 100–150 °C/W (**UNVERIFIED**, not in the datasheet), giving ≈ 105–115 °C average. Continuous TX (OTA download, ≈ 0.3 A avg → 0.5 W) at 85 °C would exceed 125 °C without a pour. **RISK** |
| Current limit ≥ 0.5 A for the ESP32-C6 | — | I_CL ≥ 1.1 A (§6.5) | OK |
| LCSC C15578 is a genuine TI part | jlc.py FIXED | — | **UNVERIFIED** (LCSC listing not checked) |

---

## 3. D2: SS14 (VBUS → +5V)

| Claim | Where | Datasheet value (Vishay 88746) | Verdict |
|---|---|---|---|
| Schottky 1 A 40 V | controller.py | V_RRM 40 V, I_F(AV) 1.0 A, I_FSM 40 A (p. 1) | OK |
| Pin 1 = K, pin 2 = A on KiCad D_SMA | controller.py | KiCad D_SMA pad 1 = cathode | OK |
| USB "5.0 V through D2 (~4.6 V)" | sim/run.py | V_F max **0.50 V at 1 A**, 25 °C (p. 2). About 0.4 V at 0.4 A typ. (Fig. 3) | OK-ish. Worst-case USB 2.0 VBUS is 4.75 V at the host and less after the cable, which gives ≈ 4.3 V on +5V. The LDO still holds 3.3 V (4.3 − 0.39 > 3.6 V), but the case isn't simulated |
| Reverse leakage | — | I_R 0.2 mA at 25 °C, **6 mA at 100 °C** at 40 V (p. 2) | OK (at 5 V reverse it is much less) |
| T_J | — | −65…+150 °C; R_θJA 88 °C/W | OK |
| Maker | controller.py MPN "SS14" (generic), JLC C2480 | — | **UNVERIFIED**. Pin down a maker (e.g. Vishay SS14-E3/61T, onsemi SS14) in the MPN |
| Back-feeding the RECOM output when on USB only | — | RECOM datasheet says nothing on reverse voltage at the output with no AC | **UNVERIFIED** (probably a few mA into the feedback divider). Check on the bench |

---

## 4. F1: T500mA TR5 (Littelfuse 37205000411)

| Claim | Where | Datasheet value (Littelfuse 372) | Verdict |
|---|---|---|---|
| Part number 37205000411 is a real TR5 T500mA | controller.py | 372 + amp code **0500** (500 mA) + packaging **0411** ("short leads, bulk 1,000 pcs") (p. 3, part numbering) | OK (valid MPN) |
| 250 VAC | controller.py, CIRCUIT | 250 V rated (p. 2) | OK for 120 V |
| Time-lag | — | IEC 60127-3 time-lag; 210 % → ≤ 2 min; **150 % → ≥ 1 h**; 1000 % 20–150 ms (p. 1) | see the MOV-failure risk in §5 |
| Melting I²t | — | **1.3 A²s** at 10 × I_N (p. 2) | — |
| I²t vs PSU inrush | — | inrush 10 A at 115 VAC (RECOM p. 1), duration not stated | OK by estimate only (**UNVERIFIED**). A fusible resistor of ≈ 16 Ω feeding ≈ 5–10 µF of bulk gives τ ≈ 0.1 ms and I²t ≈ 10² × τ/2 ≈ 0.005 A²s, far below 1.3 A²s even with a pulse derating of ×0.2. Confirm with a scope at switch-on |
| Breaking capacity | — | **35 A at 250 VAC** (UL 50 A) (p. 2) | **RISK**. When RV1 fails short, the prospective current at a US 15/20 A outlet through the machine's cord can be hundreds of amps. A 35 A-IR glass/plastic TR5 is not rated to clear that. Whether the machine has its own upstream fuse is unknown (**UNVERIFIED**) |
| 85 °C operation | HARDWARE | −40…+85 °C "(consider de-rating)"; rerating curve ≈ **85 % at 85 °C** (≈ 425 mA) (p. 2–3) | OK (≈ 30–70 mA rms load) |
| "~80 mA" stress | HARDWARE | RECOM input current 70 mA at 115 VAC full load (p. 1) | OK |
| Footprint `Fuse_Littelfuse_372_D8.50mm` | controller.py | Ø 8.5, pitch 5.08, leads Ø 0.6; footprint drill 1.0 | OK |
| Surges pass through F1 | (topology) | RV1 sits after F1, so the surge current RV1 diverts flows through F1 | **RISK**. IEC 61000-4-5 1 kV / 2 Ω L–N: (1000 − ≈450 V)/2 Ω ≈ 275 A peak, 8/20 µs, so I²t ≈ 275² × 14 µs ≈ 1 A²s, about the same as the 1.3 A²s melt I²t. A 1–2 kV mains surge will probably open or age F1. That fails safe (no 3.3 V means no opto drive), but the board goes dead, and from then on the triacs are unprotected because RV1 is cut off with F1 |

---

## 5. RV1: Bourns MOV-10D271K

| Claim | Where | Datasheet value (Bourns 10D, REV 07/25) | Verdict |
|---|---|---|---|
| Voltage class for 120 VAC | controller.py | 10D271K: **175 V rms / 225 V DC** max continuous; V₁mA 243–270–297 V; clamp **455 V at 25 A** (p. 1 table) | OK. 120 V + 10 % = 132 V rms, a 43 V margin. It is more conservative than the common 201K/241K (130/150 V rms) choice, which helps life at 85 °C. The clamp stays under 800 V for the triacs and optos |
| "MOV clamps surges at ~450–700 V" | controller.py, CIRCUIT | 455 V at 25 A (class current); higher at kA | OK |
| Energy / surge | — | 2500 A peak (8/20 µs, once); **40 J**; 0.40 W rated | OK for an appliance board |
| Rated 85 °C | HARDWARE | operating **−40…+105 °C** (p. 1) | OK (understated) |
| **Footprint** `Varistor:RV_Disc_D7mm_W4.2mm_P5mm` | controller.py MOV template | 10D: body Ø **12.5 mm max**, T 4.2 mm, lead pitch **7.5 ± 0.8 mm**, leads **Ø 0.8 ± 0.1 mm** (p. 2 dimensions). The footprint is a 7 mm disc, **5 mm pitch, 0.6 mm drill** | **WRONG**: the part doesn't fit (pitch, lead diameter and body). The courtyard also understates the body by 5.5 mm, so neighbouring mains parts may collide |
| "F1 sits upstream, so a worn-out MOV that shorts blows the fuse instead of burning" | HARDWARE | F1 holds 150 % (0.75 A) for ≥ 1 h (Littelfuse p. 1) | **RISK**. MOVs usually fail by thermal runaway at partial conduction. 0.5–0.9 A at 120 V is 60–110 W in a 10 mm disc, which a T500mA may not clear for minutes to hours. That is a fire path; this is why UL 1449 requires thermally protected MOVs |
| JLC hand part C2959788 = "10D271K" | jlc report | — | **UNVERIFIED** (maker and dimensions not checked) |

---

## 6. Capacitors C3, C6, C7 (0805) and the effective capacitance

| Claim | Where | Datasheet value | Verdict |
|---|---|---|---|
| C6, C7 = Murata GRM21BZ71A226ME15L, 22 µF 10 V X7R, 125 °C | controller.py, HARDWARE | 22 µF ±20 %, 10 Vdc, 0805, T 1.25 mm, −55…125 °C. Temp. char. **"X7R (MURATA)"**: ±15 % **specified with 50 % of rated voltage applied** (Murata note "Derating 1"). This is not plain EIA X7R, which is specified at 0 V | OK, with the note: it is Murata-X7R, which is still a 125 °C dielectric |
| DC-bias loss | — | Murata DC-bias curve (25 °C, p. 3 of the spec sheet): **≈ −30 % at 3.3 V, ≈ −52 % at 5 V** | — |
| C6 effective on +5 V | sim uses an ideal 22 µF | nominal 22 × 0.48 ≈ **10.5 µF**; worst (−20 % tol., −15 % temp) ≈ **7 µF** | **sim WRONG** (it overstates C by 2–3×) |
| C7 effective on +3V3 | — | 22 × 0.70 ≈ 15.4 µF; worst ≈ 10.5 µF | OK |
| C3 "10uF 10V X7R" (no MPN; JLC picks TCC0805X7R106K160FT, 16 V) | controller.py | DC bias for this CCTC part **UNVERIFIED**; assume ≈ −40 % at 3.3 V → ≈ 6 µF | — |
| Sim 3V3 caps "C3 22u, C4 10u" | sim/run.py | the board has C7 22 µF + C3 10 µF + C4 100 nF | Labels don't match the board and bias is ignored. The total (32 µF ideal vs ≈ 16–21 µF real) is overstated |
| TLV1117LV C_OUT ≥ 0.5 µF effective | TI §8.2.2.1 | ≈ 16–21 µF | OK (no maximum in TI's sheet) |
| ESP32-C6 module 3V3 decoupling | Espressif v1.5, §9 Peripheral Schematics | **22 µF + 0.1 µF** on 3V3 (the module also has 10 µF inside) | OK (≈ 16–21 µF effective plus C4) |
| "X7R, not X5R (85 °C)" | HARDWARE | GRM21BZ7 −55…125 °C | OK |

---

## 7. 5 V / 3.3 V budget for the ESP32-C6 on Wi-Fi

| Item | Value (source) |
|---|---|
| ESP32-C6 TX peak, 802.11b 1 Mbps at 20.5 dBm | **382 mA** (Espressif v1.5, Table 6-4, 100 % duty); 11g 316 mA; 11n 295 mA; RX ≈ 78–82 mA |
| Module supply requirement | V_DD 3.0–3.6 V, **supply must deliver ≥ 0.5 A** (Table 6-2) |
| Other 3V3 loads | 3 opto LEDs ≈ 10 mA each (sim 8–12 mA), status LED ≈ 1–2 mA, NTC divider 0.3 mA → ≈ 35 mA |
| Other 5 V loads | pressure transducer ≈ 10 mA (**UNVERIFIED**, depends on the sensor) |
| **Peak +5V current** | ≈ 0.38 + 0.035 + 0.01 ≈ **0.43 A (2.1 W)**. The LDO passes input current = output current |
| Average +5V current | ≈ 0.1–0.15 A with Wi-Fi connected, no TX saturation (**UNVERIFIED**, firmware-dependent) → 0.5–0.75 W. This matches HARDWARE's "~0.7 W" |
| PSU capability | 3 W / 0.6 A to 75 °C; **1.8 W / 0.36 A at 85 °C**; OCP 120–300 % (≥ 0.72 A), so short bursts above 0.36 A are delivered. The derating is thermal (average) |
| Verdict | OK on average at 85 °C. Peak bursts (0.43 A) exceed the 85 °C continuous rating, which is acceptable only if the duty cycle stays low. A long Wi-Fi TX session (OTA) at 85 °C ambient is the corner to measure: PSU case ≤ 95 °C and U5 junction |
| USB bench power | 5.1 kΩ Rd → default USB power (500 mA for USB 2.0). The 0.43 A peak fits |
| Transient | The real risk is the 5 V dip during a 0.4 A step: RECOM 6 % per 10 % step plus 350 µs recovery, against ≈ 10 µF effective. Brown-out needs +5V < ≈ 3.7 V (3.3 V + ≈ 0.39 V dropout). Not expected, but **not demonstrated**. The sim can't show it (resistive PSU, ideal caps) |

---

## Must fix

1. **PS1 footprint pin positions are wrong** (`lib/Recom.pretty/RAC03E-K_277.kicad_mod`, also `lib/README.md`). Per RECOM REV 3/2023 p. 6, with pin 1 at (0, 0) and the other row at y = −17.78 (KiCad, top view):
   - pin 2 at **(5.08, 0)**;
   - pin 3 at **(30.48, 0)**;
   - pin 4 at **(30.48, −17.78)**;
   - pin 5 at **(25.40, −17.78)**.

   The body outline (−3.26…33.74 × −20.89…3.11), drill 1.0 and pad 1.8 are right. As drawn, the module cannot be inserted: every pin but pin 1 is 2.54 or 5.08 mm off. After the fix, re-run the floorplan. At the current `plan.json` origin (x = 16.6), pin 2's pad would sit in the isolation band (x 20.3–27.3), so move PS1 left to pin 1 at x ≤ 14.3, or move the band. Re-check the clearance of pins 4/5 to bottom-side U4. Also make the mains lint check the L–N pad gap of PS1 instead of exempting it as "inside one part": the wrong footprint passed with 0.74 mm between L and N.
   https://recom-power.com/pdf/Powerline_AC-DC/RAC03E-K_277.pdf

2. **RV1 footprint doesn't match the part.** MOV-10D271K has a 7.5 mm lead pitch, Ø 0.8 mm leads and a body up to Ø 12.5 × 4.2 mm. The footprint `RV_Disc_D7mm_W4.2mm_P5mm` has a 5 mm pitch and a 0.6 mm drill. Do one of the following:
   - Use `Varistor:RV_Disc_D12mm_W4.2mm_P7.5mm`, enlarging its 0.8 mm drill to ≈ 1.0–1.1 mm, because the leads are 0.8 ± 0.1 mm. Re-run placement: the body is 5.5 mm larger.
   - Change the part to a 7 mm, 5 mm-pitch disc of the same voltage (e.g. Bourns MOV-07D271K; its datasheet values are **UNVERIFIED** here). Then fix the BOM/MPN and JLC C2959788 to match.

   https://www.bourns.com/docs/Product-Datasheets/mov10d.pdf

3. **U5 has no heatsink copper, although the docs say "tab pour is the heatsink".** The tab is **OUT (+3V3)** (TI Table 5-1), and the current board has no +3V3 zone. At 85 °C ambient with track-only copper, the junction estimate is ≈ 105–115 °C average and above 125 °C during sustained TX. Add a +3V3 pour of several cm², ideally on both layers with vias under the tab, and make sure it is +3V3, not GND. Otherwise correct CIRCUIT.md and controller.py.
   https://www.ti.com/lit/ds/symlink/tlv1117lv.pdf

## Should consider

1. **MOV failure and fuse breaking capacity (safety, high priority).** The T500mA holds 150 % for ≥ 1 h, so a MOV in thermal runaway at about 0.5–0.9 A can burn before F1 opens. F1's 35 A breaking capacity is low for a hard short on a US branch circuit. Options, best first:
   - (a) A thermally protected MOV (e.g. Littelfuse TMOV / iTMOV or Bourns MOV-TC series; part choice **UNVERIFIED**).
   - (b) Drop RV1. The RAC03E alone passes IEC 61000-4-5 ±1 kV criterion A (p. 5), and the 800 V triacs and optos tolerate breakover. Keep the board's surge protection to the PSU's own.
   - (c) A higher-breaking-capacity fuse (e.g. a ceramic 5×20 or a high-IR TR5 class).

   Separately, find out whether the GCP US has an upstream mains fuse (**UNVERIFIED**). The docs' sentence "a worn-out MOV that shorts blows the fuse instead of burning" should be qualified.
2. **F1 sits in the surge path of RV1.** A 1 kV L–N surge puts about 1 A²s through F1, close to its 1.3 A²s melt I²t, so expect nuisance opening on surges. This is fail-safe but leaves the board dead. If RV1 stays, either move it ahead of F1 (only with a thermally protected MOV, see 1) or accept this and document it.
3. **Add bulk on +5V and fix the rail simulation.** At 5 V, C6 is only ≈ 7–10.5 µF effective (Murata DC-bias curve: −52 % at 5 V). RECOM allows 6 % deviation for a 10 % step, with 350 µs recovery, and a TX burst is a ≈ 65 % step. Make these changes:
   - Add a second 22 µF 0805 or a 47 µF 1206 X7R, or a 100–220 µF 125 °C polymer, at the LDO input.
   - Change `sim/run.py` to bias-derated capacitances (+5V ≈ 10 µF; +3V3 ≈ 15 + 6 µF), with C names matching the board (C7 22 µF, C3 10 µF, C4 100 nF).
   - Replace `Rpsu 0.15` with a source model that has the load-step droop and ~350 µs recovery.
   - Add a USB corner of 4.75 V minus a real D2 V_F (≈ 4.3 V).
   - Confirm the dip on the bench with continuous 802.11b TX at 20.5 dBm.

   CIRCUIT.md's "One 22 µF is enough (simulated)" isn't supported until this is done.
4. **85 °C corner of the PSU.** At 85 °C ambient the module is limited to 1.8 W and a 95 °C case. Keep average +5V current ≤ 0.36 A, which is fine, but vent the enclosure around PS1 and measure case temperature during a long OTA/TX session. Lifetime at 70–85 °C is unspecified (only > 30 kh at 50 °C full load), so ask RECOM or treat the module as a serviceable part. HARDWARE's "wear-out … fine" is **UNVERIFIED**.
5. **Overvoltage on +5V.** RECOM OVP trips only at 120–260 % (6–13 V), while TLV1117LV V_IN is 6 V abs. max (5.5 V recommended). If the PSU fails, a TVS or crowbar on +5V would save U5, U4 and the transducer. It is optional.
6. **Pin D2 to a specific maker in the MPN.** Vishay SS14: V_F ≤ 0.5 V at 1 A, I_R 6 mA at 100 °C. Check that the LCSC C2480 maker's sheet is equivalent. Check bench behaviour when USB back-feeds the unpowered RECOM output (**UNVERIFIED**).
7. **Docs and comments to correct:**
   - HARDWARE: MOV rated 105 °C, not 85 °C.
   - controller.py: the TLV1117LV dropout is "455 mV typ. / 700 mV max at 1 A".
   - controller.py and the docs: GRM21BZ7 is "X7R (Murata, ±15 % at 50 % V_R)".
   - lib/README.md: replace the pin coordinates with the corrected ones.
8. **Verify LCSC C15578** is the genuine TI TLV1117LV33DCYR (jlc.py already pins it; the listing isn't verified here).
