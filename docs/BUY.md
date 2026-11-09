# Buy list: off-board parts (board v33, US Gaggia Classic Pro, 120 V)

**v32 puts the small mains loads on the board** (Gaggiuino's split): the board carries its own
certified 5 V supply (Mean Well IRM-10-5) and switches the pump and the 3-way valve with on-board
triacs. The 11 A heater stays on its external SSR, and the stock **steam thermostat (145 °C) and
thermal fuse (184 °C) stay in series with the heater** as the protection that doesn't depend on
software. The low-voltage leads are ready-made (no crimping at the board); the thermocouple clamps into a screw terminal.

Wiring: [WIRING.md](WIRING.md). Every candidate looked at, with datasheet quotes and links, is in
[`docs/evidence/parts.jsonl`](evidence/parts.jsonl): flip a status there to change a choice. The
board itself (all its parts, including the mains section) is the JLC order: `hardware/controller/build/coffee-v33-*`.

AliExpress prices weren't readable from here: the "~" figures are typical, open the links.

## Shopping list

| # | Part | Variant / qty | Where | Price |
|---|---|---|---|---|
| 1 | **Heater SSR**: Schneider **SSP1A125BDT** (genuine, 25 A, zero-cross, hockey-puck size) | 1 | <https://www.standardelectricsupply.com/2341717-Schneider-Electric-SSP1A125BDT> | $24.97 |
| 2 | **18 AWG silicone wire** (mains: L, N, pump, valve to the board's spades; heater SSR) | 2–3 m | <https://s.click.aliexpress.com/e/_EJ5sOfm> | ~$5 |
| 3 | **6.3 mm (0.250") female receptacles, fully insulated**, for 18 AWG: the board's four spades and the machine's tabs | ~20 | any reputable (TE/NSI/Panduit), or a kit | ~$5 |
| 4 | **6.35 mm piggyback** (live split, Gaggia Classic) | 1–2 | <https://s.click.aliexpress.com/e/_EytfYJO> | ~$2 |
| 5 | **Fork connectors** SV1.25-5S (heater SSR output) | 2 | <https://s.click.aliexpress.com/e/_EIAVCxe> | ~$2 |
| 6 | **Thermocouple**: ungrounded **K-type, M4 screw**, 0.5 m PTFE lead (Gaggiuino's: "CG K/PT100 Type M3 M4 Copper Screw Thermocouple", pick **K / M4 / 0.5 m**) | 1 (+1 spare) | <https://www.aliexpress.us/item/3256805310471537.html> | ~$1 |
| 7 | **Pressure transducer 0–1.2 MPa**, 5 V, 0.5–4.5 V, G1/4 (Gaggiuino's, XIDIBEI; pick the **1.2 MPa** variant) | 1 | <https://www.aliexpress.com/item/4000756631924.html> (US-stocked same part: Seeed 114991178) | ~$10–15 |
| 8 | **Plumbing**: GaggiMate kit, or its AliExpress parts for the Classic Pro (PTFE tube 4/6 mm, PL06-01, PLF06-02, G1/8 tee, 1/4" gasket, PTFE tape) | 1 set | kit <https://shop.gaggimate.eu/collections/parts/products/gaggimate-plumbing-kit> (€26.84) | ~$10–30 |
| 9 | **Antenna**: Taoglas **CAB.A02** pigtail + **GW.15.2113** 2 dBi | 1 + 1 | Sager <https://www.sager.com/product/caba02-taoglas/7168042>, <https://www.sager.com/product/gw152113-taoglas/7168348> | $2.97 + $4.76 |
| 10 | **Ready-made JST GH leads**, single-ended, latching, ~300 mm: 3-pin (pressure), 4-pin (heater SSR), 5-pin (scale), 6-pin (front panel). Not PicoBlade/"MX1.25", SH or ZH look-alikes | 1 of each (+ spares) | AliExpress / Amazon "JST GH 1.25 mm single head"; genuine: RS 820-1403 wires + GHR-0nV-S housings | ~$1–2 each |
| 12 | **26–28 AWG silicone wire**, several colours (front panel, scale, SSR input, pressure tail) | ~5 m | any | ~$5 |
| 13 | **Crimper**: a 6.3 mm receptacle crimper (ratchet, red/blue insulated, or IWISS SN-48B for open-barrel), for the new pump lead | 1 | Amazon / AliExpress | ~$25 |
| 14 | Heat-shrink assortment, cable ties | | any | |
| 15 | **Front-panel LEDs**: red 5 mm **pre-wired LEDs with the resistor in the lead, rated 5 V** (150–470 Ω), replacing the three 120 V neon lamps. The board has 330 Ω as well; the LED's own resistor keeps a front-panel lead harmless (SWAP-SAFETY.md) | 3 (+ spares) | any; Gaggiuino's printable "Neon Lamp Clip" holds one in the stock lamp position | ~$1 |
| 16 | **Scale load cells**: 750 g mini bar cells, as Gaggiuino's scale | 2 (buy 3–4) | Gaggiuino's parts list; datasheet-backed alternative HT Sensor TAL220F-1kg | ~$6 |
| 17 | **Scale amplifier**: 2 generic HX711 modules, **powered from 3.3 V only** (link VCC and VDD) | 2 | AliExpress | ~$3 |
| 18 | **Printed scale base**: Gaggiuino scale base + Classic Pro tray adapters | 1 set | Printables model 285370 | filament |
| 19 | **Printed board holder**: frame, cover and two bars (**ASA** or ABS; not PETG or PLA: the case runs 60–70 °C): hardware/holder/out/holder_frame.stl, holder_cover.stl, holder_bars.stl | 1 | print it yourself (hardware/holder/README.md) | filament |
| 20 | **Magnets, 10 × 3 mm neodymium discs**, for the holder's back; **H or SH grade** (N35H, N42SH: 120–150 °C), not plain N52. Check your side panel is magnetic first | 7 (+ spares) | any | ~$5 |

**Total ≈ $110–135** including the receptacle crimper (~$25, reusable), plus shipping.

## Notes per part

- **1 Heater SSR.** Bolted flat on the inside of the rear panel at the ventilation slots (two M4
  bolts, or cable ties), factory thermal pad against the panel. Its input lead is a 4-pin GH to the
  board's J2 (+5V, OUT). Datasheet: <https://www.se.com/us/en/product/download-pdf/SSP1A125BDT>.
- **2–5 Mains wiring.** 18 AWG silicone for every mains run, fully insulated 6.3 mm receptacles on
  the board's four spades (N, L, P, 3) and the machine's tabs. Push receptacles on with the board in
  its holder, before the cover goes on: the channel over the board's top edge takes the push (up to ~80 N).
- **6 Thermocouple.** Its two bare wires clamp into the board's J5 screw terminal (Phoenix MKDS,
  0.14–1.5 mm²); never splice it: any joint to copper away from the board is an unmeasured second
  junction. WIRING check 8 confirms it's ungrounded and finds TC−.
- **7 Transducer.** Gaggiuino's 0–1.2 MPa part (−20…105 °C, 2.4 MPa max): ~33 % more volts per bar
  than the old 1.6 MPa part. No maker datasheet, so INSTALL §3 identifies its wires on the bench
  (red 5 V, black GND, signal green or yellow) and the firmware calibrates zero and span. Its cable
  joins the 3-pin lead's bare end with one soldered, heat-shrunk splice at the transducer end.
- **9 Antenna.** Mount the SMA bulkhead in a plastic part, not through the steel case.
- **10–13 Harness.** One ready-made lead per job, its far end soldered or fitted with a receptacle, labelled. WIRING.md has the table.
- **15 Front panel.** The stock lamps are 120 V neons wired across the thermostats; with the board
  in control they're replaced by LEDs the firmware drives.
- **16–18 Scale.** Gaggiuino's built-in scale under the drip tray; only slow digital lines run to the
  board (J10). Calibrate per CALIBRATION.md.
- **19–20 Holder.** The board has 120 V on its mains half, including the through-hole spade and IRM
  pins on its back: it must never touch the chassis. The holder's back plate is the insulation, its
  solid front wall and roof cover the board, and the magnets hold it to the side panel.

## Dropped since v31

The two SSR boards (Omron G3MB-202P), the external valve snubber and the separate 5 V supply (all on
the board now); GH crimp contacts and the PA-09 crimper (v39: ready-made leads and a thermocouple screw
terminal); the board tray, its M3 standoffs and the velcro (v39: the slide-in holder and magnets); the GaggiMate 0–1.6 MPa transducer
(alternative in the evidence store). The v30 list is in git history (`git show 989b5e3:docs/BUY.md`).
