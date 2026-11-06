# Buy list: off-board parts (board v32, US Gaggia Classic Pro, 120 V)

**v32 puts the small mains loads on the board** (Gaggiuino's split): the board carries its own
certified 5 V supply (Mean Well IRM-10-5) and switches the pump and the 3-way valve with on-board
triacs. The 11 A heater stays on its external SSR, and the stock **steam thermostat (145 °C) and
thermal fuse (184 °C) stay in series with the heater** as the protection that doesn't depend on
software. Every lead is crimped by you (no pre-crimped leads, no splices near the board).

Wiring: [WIRING.md](WIRING.md). Every candidate looked at, with datasheet quotes and links, is in
[`docs/evidence/parts.jsonl`](evidence/parts.jsonl): flip a status there to change a choice. The
board itself (all its parts, including the mains section) is the JLC order: `hardware/controller/build/coffee-v32-*`.

AliExpress prices weren't readable from here: the "~" figures are typical, open the links.

## Shopping list

| # | Part | Variant / qty | Where | Price |
|---|---|---|---|---|
| 1 | **Heater SSR**: Schneider **SSP1A125BDT** (genuine, 25 A, zero-cross, hockey-puck size) | 1 | <https://www.standardelectricsupply.com/2341717-Schneider-Electric-SSP1A125BDT> | $24.97 |
| 2 | **18 AWG silicone wire** (mains: L, N, pump, valve to the board's spades; heater SSR) | 2–3 m | <https://s.click.aliexpress.com/e/_EJ5sOfm> | ~$5 |
| 3 | **6.3 mm (0.250") female receptacles, fully insulated**, for 18 AWG: the board's four spades and the machine's tabs | ~20 | any reputable (TE/Molex/Panduit), or a kit | ~$5 |
| 4 | **6.35 mm piggyback** (live split, Gaggia Classic) | 1–2 | <https://s.click.aliexpress.com/e/_EytfYJO> | ~$2 |
| 5 | **Fork connectors** SV1.25-5S (heater SSR output) | 2 | <https://s.click.aliexpress.com/e/_EIAVCxe> | ~$2 |
| 6 | **Thermocouple**: ungrounded **K-type, M4 screw**, 0.5 m PTFE lead (Gaggiuino's: "CG K/PT100 Type M3 M4 Copper Screw Thermocouple", pick **K / M4 / 0.5 m**) | 1 (+1 spare) | <https://www.aliexpress.us/item/3256805310471537.html> | ~$1 |
| 7 | **Pressure transducer 0–1.2 MPa**, 5 V, 0.5–4.5 V, G1/4 (Gaggiuino's, XIDIBEI; pick the **1.2 MPa** variant) | 1 | <https://www.aliexpress.com/item/4000756631924.html> (US-stocked same part: Seeed 114991178) | ~$10–15 |
| 8 | **Plumbing**: GaggiMate kit, or its AliExpress parts for the Classic Pro (PTFE tube 4/6 mm, PL06-01, PLF06-02, G1/8 tee, 1/4" gasket, PTFE tape) | 1 set | kit <https://shop.gaggimate.eu/collections/parts/products/gaggimate-plumbing-kit> (€26.84) | ~$10–30 |
| 9 | **Antenna**: Taoglas **CAB.A02** pigtail + **GW.15.2113** 2 dBi | 1 + 1 | Sager <https://www.sager.com/product/caba02-taoglas/7168042>, <https://www.sager.com/product/gw152113-taoglas/7168348> | $2.97 + $4.76 |
| 10 | **JST GH plug housings**: GHR-02V-S (thermocouple), GHR-03V-S (pressure), GHR-04V-S (heater SSR), GHR-07V-S (front panel, scale) | 1 + 1 + 1 + 2 (+ spares) | Digi-Key / Mouser (LCSC stock is thin for 2- and 3-pin) | ~$0.10 each |
| 11 | **JST GH crimp contacts SSHL-002T-P0.2** (26–30 AWG, insulation OD 0.76–1.0 mm) | ~40 (you'll ruin a few) | LCSC C189897, Digi-Key | ~$0.02 each |
| 12 | **26–28 AWG silicone wire**, several colours (front panel, scale, SSR input, pressure tail) | ~5 m | any | ~$5 |
| 13 | **Crimpers**: Engineer **PA-09** (GH contacts) and a 6.3 mm receptacle crimper (ratchet, red/blue insulated, or IWISS SN-48B for open-barrel) | 1 + 1 | Amazon / AliExpress | ~$40 + ~$25 |
| 14 | Heat-shrink assortment, cable ties | | any | |
| 15 | **Front-panel LEDs**: red 5 mm **pre-wired LEDs with the resistor in the lead, rated 5 V** (150–470 Ω), replacing the three 120 V neon lamps. The board has 330 Ω as well; the LED's own resistor keeps a front-panel lead harmless in the wrong 7-pin socket (SWAP-SAFETY.md) | 3 (+ spares) | any; Gaggiuino's printable "Neon Lamp Clip" holds one in the stock lamp position | ~$1 |
| 16 | **Scale load cells**: 750 g mini bar cells, as Gaggiuino's scale | 2 (buy 3–4) | Gaggiuino's parts list; datasheet-backed alternative HT Sensor TAL220F-1kg | ~$6 |
| 17 | **Scale amplifier**: 2 generic HX711 modules, **powered from 3.3 V only** (link VCC and VDD) | 2 | AliExpress | ~$3 |
| 18 | **Printed scale base**: Gaggiuino scale base + Classic Pro tray adapters | 1 set | Printables model 285370 | filament |
| 19 | **Printed board tray** (PETG or ASA, **not PLA**: the case runs 60–70 °C) with M3 standoffs ≥ 4 mm, a solid floor under the mains zone, cutouts for the plugs; **M3 nylon screws** ×3 | 1 | print it yourself (positions: hardware/controller/plan.json, `H1`-`H3`) | filament + ~$2 |
| 20 | **Heat-resistant velcro** to hold the **tray** (never the bare board) on the side panel | a strip | adhesive rated well above 70 °C | ~$5 |

**Total ≈ $150–175** including the two crimpers (~$65, reusable), plus shipping.

## Notes per part

- **1 Heater SSR.** Bolted flat on the inside of the rear panel at the ventilation slots (two M4
  bolts, or cable ties), factory thermal pad against the panel. Its input lead is a 4-pin GH to the
  board's J2 (+5V, OUT). Datasheet: <https://www.se.com/us/en/product/download-pdf/SSP1A125BDT>.
- **2–5 Mains wiring.** 18 AWG silicone for every mains run, fully insulated 6.3 mm receptacles on
  the board's four spades (N, L, P, 3) and the machine's tabs. Push receptacles on with the board in
  its tray: the tray's solid floor under the spades takes the push force (up to ~80 N).
- **6 Thermocouple.** Crimp GH contacts straight onto its two wires (never splice it: any joint to
  copper away from the board is an unmeasured second junction). Check its wire fits the contact
  (26–30 AWG, insulation OD ≤ 1.0 mm); WIRING check 8 confirms it's ungrounded and finds TC−.
- **7 Transducer.** Gaggiuino's 0–1.2 MPa part (−20…105 °C, 2.4 MPa max): ~33 % more volts per bar
  than the old 1.6 MPa part. No maker datasheet, so INSTALL §3 identifies its wires on the bench
  (red 5 V, black GND, signal green or yellow) and the firmware calibrates zero and span. Its cable
  is thicker than GH takes: one heat-shrunk splice to a 26 AWG tail at the transducer end.
- **9 Antenna.** Mount the SMA bulkhead in a plastic part, not through the steel case.
- **10–13 Harness.** One lead per job, crimped both ends, labelled. WIRING.md has the table.
- **15 Front panel.** The stock lamps are 120 V neons wired across the thermostats; with the board
  in control they're replaced by LEDs the firmware drives.
- **16–18 Scale.** Gaggiuino's built-in scale under the drip tray; only slow digital lines run to the
  board (J10). Calibrate per CALIBRATION.md.
- **19–20 Tray.** The board has 120 V on its mains zone, including the through-hole spade and IRM
  pins on its underside: it must never touch the chassis. The tray is the insulation; the velcro
  holds the tray.

## Dropped since v31

The two SSR boards (Omron G3MB-202P), the external valve snubber and the separate 5 V supply (all on
the board now); pre-crimped GH leads (you crimp every lead); the GaggiMate 0–1.6 MPa transducer
(alternative in the evidence store). The v30 list is in git history (`git show 989b5e3:docs/BUY.md`).
