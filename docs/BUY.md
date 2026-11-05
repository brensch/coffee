# Buy list: off-board parts (board v18, US Gaggia Classic Pro, 120 V)

**Safety level: the same as GaggiMate's Lego build** (<https://docs.gaggimate.eu/docs/sourcing-lego>):
the same class of modules, mounted the same way inside the machine (no DIN rail, no extra
enclosure), and the stock **steam thermostat (145 °C) and thermal fuse (184 °C) stay in series
with the heater** as the protection that doesn't depend on software. One upgrade over GaggiMate, by
choice: a genuine heater SSR instead of an AliExpress "SSR-40DA".

Wiring: [WIRING.md](WIRING.md). Every candidate looked at (including the datasheet-verified
alternatives from earlier passes, with quotes and links) is in
[`docs/evidence/parts.jsonl`](evidence/parts.jsonl): flip a status there to change a choice.

AliExpress links are GaggiMate's own (from the Lego list). AliExpress showed this machine a robot
check, so their prices weren't read: the "~" figures are typical, **open the links for the real
price** and pick the variant named.

## Shopping list

| # | Part | Variant / qty | Where | Price |
|---|---|---|---|---|
| 1 | **Heater SSR**: Schneider **SSP1A125BDT** (genuine, 25 A, zero-cross, same hockey-puck size as GaggiMate's 40-DA) | 1 | <https://www.standardelectricsupply.com/2341717-Schneider-Electric-SSP1A125BDT> | $24.97 |
| 2 | **Pump and valve SSRs**: 1-channel 5 V SSR board (Omron G3MB-202P, 2 A, zero-cross) | **2**, "1-Channel high 5V" | GaggiMate's link <https://s.click.aliexpress.com/e/_EviCKOc> | ~$2 each |
| 3 | **AC snubber** for the valve SSR | 1 | <https://s.click.aliexpress.com/e/_EGhn5Gy> | ~$1 |
| 4 | **5 V 2 A power supply** module | 1 | <https://s.click.aliexpress.com/e/_EweWqIc> | ~$4 |
| 5 | **18 AWG silicone wire** (mains) | 2–3 m | <https://s.click.aliexpress.com/e/_EJ5sOfm> | ~$5 |
| 6 | **6.35 mm spade connectors**, male + female | kit | <https://s.click.aliexpress.com/e/_EuLcsLO> | ~$4 |
| 7 | **6.35 mm piggyback** connector (live split, Gaggia Classic) | 1–2 | <https://s.click.aliexpress.com/e/_EytfYJO> | ~$2 |
| 8 | **Fork connectors** SV1.25-5S (heater SSR output) | 2 | <https://s.click.aliexpress.com/e/_EIAVCxe> | ~$2 |
| 9 | **JST GH 1.25 mm 3-pin leads**, single-ended, 300 mm (our board's connectors) | 1 pack of 10 | <https://www.aliexpress.us/item/3256809108366050.html> (variant 3P, single head, 300 mm) | $1.14 + $5.80 ship |
| 10 | **NTC probe**: Molex **218537-3919** 50 kΩ ring tab (the board is matched to it) | 1 | Heilind <https://www.heilind.com/npi/stocking-package/2040> | $9.76 |
| 11 | **Pressure transducer** 0–1.6 MPa, 5 V, 0.5–4.5 V (GaggiMate's) | 1 | <https://s.click.aliexpress.com/e/_Evn1mNS> | ~$18 |
| 12 | **Plumbing**: GaggiMate kit, or its AliExpress parts for the Classic Pro (PTFE tube 4/6 mm, PL06-01, PLF06-02, G1/8 tee, 1/4" gasket, PTFE tape) | 1 set | kit <https://shop.gaggimate.eu/collections/parts/products/gaggimate-plumbing-kit> (€26.84) or the parts list on the Lego page | ~$10–30 |
| 13 | **Antenna**: Taoglas **CAB.A02** pigtail + **GW.15.2113** 2 dBi | 1 + 1 | Sager <https://www.sager.com/product/caba02-taoglas/7168042>, <https://www.sager.com/product/gw152113-taoglas/7168348> | $2.97 + $4.76 |
| 14 | Heat-shrink assortment, electrical tape, cable ties | | any | |

**Total ≈ $95–115** plus shipping, depending on the AliExpress prices and the plumbing route.

## Notes per part

- **1 Heater SSR.** Mounted like GaggiMate's: flat on the inside of the rear panel at the
  ventilation slots, held by two M4 bolts with nuts and washers through the slots (or cable ties),
  factory thermal pad against the panel. No separate heatsink: the panel takes the heat.
  Datasheet: <https://www.se.com/us/en/product/download-pdf/SSP1A125BDT> (24–300 VAC, 0.15–25 A,
  3–32 VDC input, zero-cross, 1 mA leakage, UL 508). If it runs hot in use (INSTALL §6), the
  Auber HS25ET heatsink ($13.49) is the verified add-on (evidence store).
- **2 Pump/valve SSR boards.** The Omron G3MB-202P is 2 A, zero-cross, 100–240 VAC. Each board
  takes 5 V on VCC/GND and a high-level IN; WIRING.md shows how the board's J3/J8 drive it. The
  pump (0.35–0.45 A) and valve are well inside 2 A.
- **3 Snubber.** Across the valve SSR's output terminals: the valve's small inductive load
  otherwise may not let the SSR switch off cleanly (GaggiMate lists it for this).
- **4 Power supply.** GaggiMate's 5 V 2 A module, fed from the switched live and neutral. If the
  variant has PCB pins, solder the wires and heat-shrink every pin.
- **5–8 Wiring.** As GaggiMate: 18 AWG silicone for every mains run, 6.35 mm quick-connects to the
  machine's tabs, a piggyback on the live, forks on the heater SSR's screw terminals.
- **9 GH leads.** Compatible clones; check the latch clicks into the board's SM03B-GHS-TB headers.
- **10 NTC.** Kept because the board's 4.7 kΩ reference and the firmware defaults are matched to
  this probe. It replaces GaggiMate's K-type thermocouple, which needs a MAX31855 board we don't use.
- **11 Transducer.** GaggiMate's; no datasheet, so INSTALL §3 identifies its wires on the bench.
- **13 Antenna.** Mount the SMA bulkhead in a plastic part, not through the steel case.

## Dropped since the previous list (records kept in docs/evidence)

Phoenix DIN-rail relay and DIN rail, Crydom EZ240D5, RECOM supply, external fuse and holder, Auber
heatsink (now optional), Tensility 14 AWG wire, TE FASTONs, WAGO, 3M heat-shrink, genuine JST
leads. The previous, fully datasheet-verified list (≈ $211) is in git history
(`git show 9c08f91:docs/BUY.md`).
