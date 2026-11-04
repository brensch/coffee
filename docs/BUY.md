# Buy list: off-board parts for the low-voltage controller (board v17)

US Gaggia Classic Pro, 120 V / 60 Hz. Everything here sits off the PCB. Wiring is in
[WIRING.md](WIRING.md).

**How this was checked.** Every price and stock figure below was read from the linked page,
opened in a real browser on **2026-11-04**. Every spec quoted was read from the linked datasheet
or, where no datasheet exists, from the product page itself (said so in each case).
- **UNVERIFIED** means I couldn't confirm it from a page I opened.
- Digi-Key, Mouser, Newark, Arrow, RS/Allied, Galco, TME, Radwell, Phoenix Contact and Sensata
  showed bot checks or access-denied pages. They were skipped, not bypassed. Links to them are
  marked **check by hand**.
- eBay and AliExpress listings are third-party sellers. Prices on AliExpress are for the exact
  variant named (selected on the page); AliExpress shows a "Standard" shipping charge per store.
- Prices exclude shipping, tax and tariffs unless stated. Shipping charges seen on the pages are
  listed under the total.

---

## 1. Heater SSR: Schneider Electric **SSP1A125BDT**, qty 1

- **Buy:** <https://www.standardelectricsupply.com/2341717-Schneider-Electric-SSP1A125BDT> (Standard Electric Supply, SKU 2341717).
  **$24.97** each (typical $27.47). **15 in stock**, "Ships Today", seen 2026-11-04.
- **Datasheet:** <https://www.se.com/us/en/product/download-pdf/SSP1A125BDT> (Schneider product data sheet, generated Nov 4, 2026).

**Specs checked in the datasheet:**

| Check | Datasheet says | Verdict |
|---|---|---|
| Switching | SCR output, **zero voltage switching** | OK |
| Output | 24–300 V AC, load current **0.15–25 A**, 600 V transient | OK for 120 V, ~12 A |
| Input | **3–32 V DC**, pick-up **3 V DC**, drop-out 1 V DC, 10 mA at 12 V DC | OK: turns on reliably from the board's 5 V |
| Off-state leakage | **1 mA max** | OK |
| On-state drop | < 1.15 V | So ~12–14 W loss at 12 A: needs the heatsink |
| Thermal | Rjc 0.8 °C/W, **thermal pad factory-fitted** ("Provided accessory: Thermal pad") | No grease needed |
| Isolation | 4 kV AC in/out and to case; 6 kV impulse | OK |
| I²t | 259 A²s (8.33 ms, 60 Hz) | |
| Approvals | UL 508, CSA C22.2 No 14, CE; ambient −40…80 °C | OK |
| Terminals | output 20–10 AWG with a lug (20–8 without); input 24–12 AWG | |

**Derating (datasheet p6):** on Schneider's 2.5 °C/W heatsink it carries about 16 A at 60 °C ambient and 11 A at 80 °C. The HS25ET is 2.0 °C/W, so 11.4 A is fine below ~75 °C inside the case. Mounting holes are 47.6 mm apart (HS25ET: 48 mm, fits).

**Why not the Gefran GQ-25-24-D-1-3** (AutomationDirect, $25.00, 64 available, page opened
2026-11-04):
- its load range is 24–230 VAC and its input is 3–32 VDC, so it would also work;
- but its leakage spec wasn't checked;
- and the Schneider ships with the thermal pad and a full datasheet I could read.

## 2. Heatsink: Auber Instruments **HS25ET**, qty 1

- **Buy:** <https://www.auberins.com/index.php?main_page=product_info&products_id=244>.
  **$13.49**. The page shows no stock count or out-of-stock notice (it only notes that *refurbished* units may be limited).
- **Datasheet:** none published; the specs below are from the product page.

**Specs (product page):**
- **2.0 °C/W** (requirement ≤ 2.5).
- 100 × 90 × 30 mm.
- 6 × M4 threaded holes: two for the SSR at **48 mm** spacing (the standard 25 A puck footprint; the SSP1A125BDT is 59 × 44.5 mm per SES); four corner holes at 90 × 79.5 mm.
- Includes 6 M4 screws and SI-07 thermal grease. Don't use the grease with the SSP: it has its own pad.
- Sold as external-mount (needs a 70 × 55 mm cutout). It can also be mounted fully inside the case: see WIRING.md.

**Thermal estimate**, from the datasheet numbers above, not a measurement:
- 14 W × (2.0 + ~0.2 pad) ≈ 31 °C sink rise, plus 0.8 °C/W × 14 W ≈ 11 °C to the junction.
- At 60 °C inside the case: junction ≈ 102 °C, at continuous full power.
- Normal PID duty is far lower.

## 3. Pump SSR: Sensata Crydom **EZ240D5**, qty 1

- **Buy (opened):** <https://enrgtechglobal.com/product/solid-state-relays/ET16685817/ez240d5> (Enrgtech, UK; ships to the US).
  **£23.05** ex VAT (≈ $30; exchange rate not verified). **15 in stock**, ships in 2–3 business days, est. ship 05 Nov 2026.
- **US source, seen only via aggregator:**
  - Findchips (<https://www.findchips.com/search/EZ240D5>, opened 2026-11-04) lists RS (formerly Allied), part 70131324, with **18 in stock at $15.70**.
  - The RS/Allied pages block automated browsers, so that page itself is **UNVERIFIED**. Buy it by hand at <https://www.alliedelec.com/product/sensata-crydom/ez240d5/70131324/> if you prefer a US shipper.
  - Findchips also shows Sager and Heilind with 0 in stock.
- **Cheaper verified US source: none found** (searched 2026-11-04). Amazon lists it at $55.93 ("Only 15 left") and $69.00. The eBay search page (listing not opened) showed US seller online_components at $30.39 + $8.99 shipping, which is no cheaper than Enrgtech.
- **Datasheet:** Crydom EZ Series, Rev 022912. Sensata's own site is behind Cloudflare, so I read this third-party mirror: <https://static.chipdip.ru/lib/936/DOC017936120.pdf>.

**Specs checked in the datasheet:**

| Check | Datasheet says | Verdict |
|---|---|---|
| Switching | Zero-cross (random turn-on is the **R** suffix) | OK |
| Snubber | Base part is **unsnubbered**: snubbered parts carry the **S** suffix, with 10 mA leakage | OK |
| Off-state leakage | **0.1 mA rms max** at rated voltage | OK (≤ 1 mA) |
| Output | 24–280 Vrms, 5 A, **min load 150 mA**, 600 Vpk, dv/dt 500 V/µs, 38/40 A surge | OK: the 0.35–0.45 A half-wave pump load is above min load |
| Input | **3–15 VDC**, min turn-on 3.0 V, turn-off 1.0 V, 15 mA typ, 300 Ω | OK at 5 V |
| Isolation | 4000 Vrms (240 V DC-control models) | OK |
| Terminals | **0.25" (6.3 mm) Faston**, all four | |
| Mounting | 8-32 screw. "Heat sinking required, see derating curves", but < 1 W here | Chassis panel is enough |

## 4. Valve relay module: Phoenix Contact **PLC-RSC- 5DC/21** (order no. **1119897**), qty 1

- **Buy (single unit):** <https://www.ebay.com/itm/137391576922> (eBay, seller **saleaxis**, 99.6 % positive, 35.3K ratings, located in the US).
  - **$33.07 each**, Buy 1, condition **New**, **50 available**, + **$3.99** standard shipping. 30-day returns. Item specifics: brand Phoenix Contact, model 1119897, country of origin Germany. Seen 2026-11-04.
  - It's a third-party seller, not an authorised distributor. Check the label on arrival (PLC-RSC- 5DC/21, 1119897).
- **Authorised single-unit sources (check by hand, bot-blocked):** Digi-Key <https://www.digikey.com/en/products/detail/phoenix-contact/1119897/17938740>; Mouser <https://www.mouser.com/c/?q=1119897>.
- **Datasheet (verified):** Phoenix-generated catalog exports of the product data, French (9 Jan 2024) <https://res.cloudinary.com/sonepar-fr/raw/upload/s--VsUJ23XL--/v1/documents/126/phoenix-contact-1119897-FR.pdf> and Chinese (24 Feb 2025) <https://docs.rs-online.com/df3e/A700000012977652.pdf>. The two agree on every value. Phoenix's own English page refuses automated browsers: <https://www.phoenixcontact.com/en-us/products/relay-module-plc-rsc-5dc21-1119897> (open by hand if you want the English copy).
- **Specs from that datasheet:**
  - input 5 V DC, **input range 4.2–7 V DC at 20 °C**, **38 mA** typical; polarised input with protection and freewheeling diodes;
  - **ambient −40…60 °C.** The minimum pick-up voltage rises with temperature, to about 4.45 V at 60 °C. In the machine the board passes on the RECOM's 5.0 V (≈ 0.55 V margin). On USB bench power (~4.6 V) the margin is only ~0.15 V when hot. Mount it on the side panel, away from the boiler;
  - reinforced isolation coil-to-contact: 4 kVrms test, 6 kV surge, 250 VAC rated insulation voltage;
  - contacts 6 A; AC15 (solenoid-type loads) 3 A at 120 V; minimum load 10 mA at 12 V. The ~0.1 A valve is well inside;
  - UL/cUL approved;
  - **no touch protection above 25 VAC:** it must sit under the case cover (it does);
  - terminals A1, A2 (input), 11 (common), 12 (NC), 14 (NO), from the connection diagram. A1 is + (the input is polarised, with a protection diode), though the drawing doesn't print a "+".
- **Packaging:** Phoenix itself sells 10 minimum; the eBay single above is the one-unit source.
- **Alternatives looked at (2026-11-04):**
  - **Amazon** <https://www.amazon.com/dp/B0FSJYXPSW>: "1119897, Industrial Relays PLC-RSC- 5DC/21", $54.99 + $9.99 delivery, In Stock, sold by Neutron USA under brand "Generic". Rejected: dearer, and its spec text (SPST-NO, 4.5 V, 1 mA coil) doesn't match the part.
  - **Weidmüller TRS 5VDC 1CO (1122740000):** on the eBay search page only (listing not opened), US seller ddwoodlee, **pack of 5** for $36.54 + $8.95; single units from China sellers at $35–45 + ~$15 shipping. A fallback if the Phoenix sells out. Its coil current and isolation weren't checked.
  - **Finder 38.51.7.005.0050:** LCSC and eibabo.us have no listing for the 5 V version (LCSC offers only the 6 V 38.51.7.006.0050 at $43.76 each). No US single-unit source found.
  - **Omron G2RV-SR500 DC5:** eBay and Amazon showed only 24 V and 110 V versions.
  - **AutomationDirect:** no 5 V DC interface relay (its Klemsan slim relays start at 12 V).

## 5. 5 V supply: RECOM **RAC05-05SK/277/W**, qty 1

- **Buy:** <https://www.sager.com/product/rac05-05sk277w-recom-power/6051147> (Sager, authorised).
  **$13.85** (qty 1). **56 in stock, ready to ship.** Factory lead time 26 weeks.
- **Datasheet:** <https://recom-power.com/pdf/Powerline_AC-DC/RAC05-K_277.pdf> (linked from the Sager page).

**Specs checked in the datasheet:**

| Check | Datasheet says | Verdict |
|---|---|---|
| Input | **85–305 VAC**, 47–63 Hz | OK |
| Output | 5 V, 1000 mA, 80 % efficiency; < 100 mW no-load | OK |
| Safety approvals | **UL 62368-1** (E491408), CAN/CSA 62368-1, IEC/EN 62368-1, **IEC/EN 60335-1** certified, EN 61558-1, EN 62233 | OK |
| Insulation | Class II, **reinforced**; 4.2 kVAC isolation (Sager page) | OK |
| EMC | EN 55032 / FCC Part 15 Class B, no external parts | OK |
| Temperature | Full load −40 to **+75 °C** for the 5 V model (natural convection), so 60 °C ambient is fine; storage to 85 °C | OK |
| Protection | **internal T1A slow-blow fuse**, SCP (hiccup), OVP | Item 6 adds a fuse anyway |
| Wired version (/W) | 100 mm UL1015 **18 AWG** leads: **blue = N, brown = L, red = +Vout, black = −Vout** | Not PCB-pin only: OK |
| Size | 31.7 × 26.7 × 21.8 mm, 37 g; no mounting flange | Tie or clip it down |

**Alternative:** the Mean Well HDR-15-5 (DIN rail) was **not checked**: no page opened.

## 6. Fuse holder and fuse

| Item | Part | Qty | Buy (opened 2026-11-04) | Price / stock | Spec source |
|---|---|---|---|---|---|
| Fuse holder, panel-mount 5×20, 6.3 mm quick-connect terminals | Eaton Bussmann **HTB-26M-R** | 1 | <https://crimpsupply.com/bussmann-htb-26m-r-fuseholder.html> | **$13.82**, "In Stock" (Crimp Supply, seen 2026-11-04; not confirmed as an authorised Eaton distributor) | **Datasheet:** Eaton Technical Data 2119 (Jan 2024): 5×20 mm, **UL Recognized E14853, CSA 47235**, 250 V (UL 300 V), 16 A, −55…85 °C, 1/4" quick-connect terminals, 12.9 mm D-hole in a panel up to 7.6 mm thick (mount it in the printed holder) |
| Fuse, 1 A time-delay 5×20 mm, 250 V, **ceramic, 1500 A breaking** | Eaton Bussmann **S505-1-R** | 1 (buy 2: a spare) | <https://www.digikey.com/en/products/result?keywords=S505-1-R> | **$4.38** each (Digi-Key search result, "FUSE CERAMIC 1A 250VAC 5X20MM", seen 2026-11-04; check at checkout) | **Datasheet:** Eaton Technical Data 2037 (Oct 2020): time-delay ceramic, 1 A, 250 VAC, **1500 A breaking at 250 VAC**, UL Recognized E19180, CSA 53787. A glass fuse (GDC, 35 A breaking) is too weak for a mains branch |

- **Not the GDC-1A** listed here before: it is glass, with only 35 A breaking capacity (Eaton datasheet).
- **Rejected holders:** the MECCANIXITY Amazon holder (no datasheet, no UL/CSA claim); Littelfuse 01500520Z (404 on Littelfuse's site; the 150520 datasheet's only part, 01500520H, is obsolete and sold in 100s); Eaton HHT in-line (neither its datasheet nor Eaton's page states UL/CSA).
- **Rejected:** Littelfuse 0218001.MXP is $8.08 + $7.99 delivery but comes as 25; BOJACK and Delon packs aren't major brands.
- Inline holders on Amazon were all automotive (32 V) or unrated, so they were rejected.

## 7. DIN rail: AutomationDirect **DN-R35S-150-4**, qty 1 pack

- **Buy:** <https://www.automationdirect.com/adc/shopping/catalog/terminal_blocks_-a-_wiring_connectors/din_rail_-a-_accessories/din_rails/dn-r35s-150-4>.
  **$8.00 per 4-pack**, **23 available**, free delivery Fri Nov 6.
- **Specs (product page):** slotted, 35 mm wide, 7.5 mm high, **150 mm** long, plated steel, deburred pre-cut. Only needed for the Phoenix relay.

## 8. Mains wiring and terminals

| What | Exact part | Qty | Buy (opened 2026-11-04) | Price / stock | Specs checked (product page; no datasheets) |
|---|---|---|---|---|---|
| 16 AWG silicone wire (heater path, L/N feeds, valve live) | BNTECHGO 16 AWG silicone, red + black, 10 ft each | 1 | <https://www.amazon.com/dp/B017TFR664> | **$9.98**, In Stock | "200 degree C … Rated Voltage: 600 volts", OD 3.0 mm, tinned stranded. **No UL style number given: UNVERIFIED as a listed appliance wire** |
| 18 AWG silicone wire (pump, valve COM, supply) | BNTECHGO 18 AWG silicone, red + black, 25 ft each | 1 | <https://www.amazon.com/dp/B07HGTKQ89> | **$13.98**, In Stock | "200 degree C … Rated Voltage: 600 volts", OD 2.3 mm. Same caveat |
| Fully insulated nylon 6.3 mm spade kit, male + female, 22-18 and 16-14 AWG | **Nilight 400PCS** nylon quick-disconnect male and female spade terminals, ASIN B0B3T8VNLB | 1 kit | <https://www.amazon.com/dp/B0B3T8VNLB> | **$19.77** (list $21.99), In Stock, shipped and sold by Amazon.com | Listing: "fully insulated male and female crimp terminals", insulation **nylon**, tinned brass. Red = 22-18 AWG, blue = 16-14 AWG, yellow = 12-10 AWG. "200PCS Male and 200PCS female … 80pcs each for 22-18 and 16-14 gauge wires. 40pcs each for 12-10". **The 6.3 mm (0.250") tab width isn't stated in the listing text: check one against an SSR tab before crimping** |
| Insulated fork, #8 (M4), 16-14 AWG (heater SSR output) | Baomain **SNYBS2-4** nylon fork, 100-pack | 1 | <https://www.amazon.com/dp/B09WXLMNZW> | **$7.59**, In Stock | Title: nylon insulated fork, 16-14 AWG, USA #8 |
| L and N junctions | WAGO **221-415** lever nuts (5-conductor), 25-pack | 1 (uses 2) | <https://www.amazon.com/dp/B0107SYYGU> | **$22.03**, add-to-cart available, delivery Nov 10–14 (no stock count shown) | Title: "ENEC & UL certified, all conductor types, 450V, 32A" |
| Fibreglass sleeving, 6 mm | uxcell fibreglass braid sleeving, 6 mm, 16.4 ft | 1 | <https://www.amazon.com/dp/B082YKWPSL> | **$8.54**, In Stock | Title: high-temp fibreglass sleeve. **Temperature rating not stated: UNVERIFIED** |

**What the build uses from the spade kit** (matches WIRING.md):
- **22-18 female (red), 9:** M6 ×1, M7 ×2, J3 ×2 (pump SSR input), J7 ×4 (C.1, C.2, S.4, S.5).
- **16-14 male (blue), 5:** M1, M4, M9, M10, M11.
- The kit has about 40 of each colour and sex, so there are plenty of spares for practice crimps.

Notes:
- **Cheaper kit, contents not itemised:** Nilight 160PCS male/female nylon fully insulated kit (22-18, 16-14, 12-10 AWG), <https://www.amazon.com/dp/B07PM3825J>, **$11.69**, In Stock. The listing doesn't say how many of each size are in it.
- **Rejected:** BAOMAIN 1/4" fully insulated male/female kits (<https://www.amazon.com/dp/B01G8BMZA2>). The listing does state 6.3 mm, but the insulation is **PVC**, not nylon.
- Nylon Faston insulation is typically rated about 105 °C. Keep crimps off the boiler and sleeve the wire runs instead.
- No #6 forks are needed: the heater SSR input takes the GH lead's folded strands (or a ferrule) directly, and the pump SSR has Faston tabs.
- **Optional, not in the total:** if you already own a ferrule crimper, use 0.25 mm² (24 AWG) insulated ferrules on the folded GH-lead ends at the heater SSR input and the relay coil, and 1.5 mm² (16 AWG) ferrules on the relay contacts.

## 9. Low-voltage leads: genuine JST **GH** 1.25 mm, 3-pin

Every board connector is a JST GH 1.25 mm 3-pin side-entry header (SM03B-GHS-TB). The mating cable housing is the **GHR-03V-S**.

- **Datasheet (verified):** JST GH catalogue <https://www.jst-mfg.com/product/pdf/eng/eGH.pdf> (April 2026): housing **GHR-03V-S**, contact **SSHL-002T-P0.2**, header SM03B-GHS-TB; **1.0 A** (26 AWG), **50 V**, **−40 to +105 °C**, wire 30–26 AWG.
- **Part:** JST **AGHGH28K305** pre-crimped jumper (305 mm, 28 AWG UL1571, SSHL-002T-P0.2 at both ends; JST drawing) + **GHR-03V-S** housings. Per lead: push three jumpers' contacts into one housing; the far ends then go to the module (cut off the far contacts and strip). 7 leads = **21 jumpers + 7 housings**.
- **Buy (check by hand: Digi-Key shows a human-verification page to automated browsers):** <https://www.digikey.com/en/products/detail/jst-sales-america-inc/AGHGH28K305/6009450> and <https://www.digikey.com/en/products/result?keywords=GHR-03V-S>. LCSC showed 1 jumper and 0 housings in stock, so it isn't an alternative.
- **Not chosen:** the AliExpress "JST GH compatible" clone leads in earlier versions of this list (no datasheet; latch fit unknown).
- **Wire gauge:** 28 AWG. How to join the leads to each module is in WIRING.md, "Joining the GH leads".

## 10. Sensors

### 10a. NTC thermistor: Molex **218537-3919** ring-tab probe, qty 1

**Requirement:** NTC with a datasheet R–T table, ≥ 150 °C, body insulated from the leads, mountable at the brew-thermostat seat under an M4 screw. (No M4 *screw-in* 100 kΩ probe rated ≥ 150 °C with a manufacturer datasheet was found; the earlier Trianglelab probe has none.)

- **Buy:** Heilind Electronics (authorised Molex distributor) <https://www.heilind.com/npi/stocking-package/2040>: **$9.76** (qty 1), "Immediate Shipment: 50", seen 2026-11-04.
- **Datasheet (verified):** Molex sales drawing 2185373919 rev B4 <https://www.molex.com/content/dam/molex/molex-dot-com/products/automated/en-us/salesdrawingpdf/218/21853/2185373919_sd.pdf>, sheets 1–2; product page <https://www.molex.com/en-us/products/part-detail/2185373919>.
  - **R25 = 50 kΩ ± 1 %**, B0/50 = 4035 K ± 1 %, full R–T table on sheet 2 (25 °C 50.000 k, 95 °C 3.638 k, 140 °C 1.029 k, 200 °C 0.266 k);
  - **−40 to +250 °C**;
  - **body isolation 1500 VAC for 3 s, ≥ 100 MΩ at 500 VDC**;
  - stainless housing with a ring tab, hole **Ø4.2 ± 0.1 mm** (takes an M4 screw);
  - lead 300 mm, 28 AWG PTFE (UL10344) in fibreglass sleeve, ends stripped;
  - time constant 15 s in water; dissipation constant 1 mW/°C.
- **Board and firmware:** R15 is 4.7 kΩ to match (1.50 V at 93 °C). The firmware default is a Steinhart-Hart fit to Molex's table, within 0.2 °C from 25 to 200 °C; the two-point calibration removes the rest (tolerance, mounting).
- **To check on the machine:** that the thermostat's screws are M4 (INSTALL.md §4).

### 10b. Pressure transducer, 0–1.6 MPa, 5 V, 0.5–4.5 V, G1/4, qty 1

- **Part:** "XYNLHZQ" pressure sensor transmitter, variant **0–1.6 MPa**, ASIN B0DLVP141L (generic, no manufacturer part number).
- **Buy:** <https://www.amazon.com/dp/B0DLVP141L>. **$24.34**, **In Stock**.
- **Specs (product page; no datasheet):**
  - G1/4 thread, stainless body, ceramic sensor;
  - supply 5 V DC (or 12 V);
  - output **0.5–4.5 V**;
  - accuracy ±1.0 % FS;
  - operating −40…105 °C, compensation −20…80 °C.
- **UNVERIFIED:** wire colours and pinout, and that the output is truly ratiometric. The firmware self-zeroes; confirm 0.5 V at 0 bar on the bench.
- **Rejected:** Jeanoko 1.6 MPa (<https://www.amazon.com/dp/B0GVHLQL4R>, $31.89, in stock) has an **M10 × 1.5** thread, not G1/4.

## 11. Antenna: Taoglas CAB.A02 pigtail + Taoglas GW.15.2113 2 dBi dipole

- **Module side (verified):** the ESP32-C6-MINI-1U datasheet v1.5 (<https://www.espressif.com/sites/default/files/documentation/esp32-c6-mini-1_mini-1u_datasheet_en.pdf>) §10.2: the -1U's connector is compatible with Hirose W.FL, I-PEX MHF III and Amphenol AMMC; **antenna gain must not exceed 2.33 dBi** (the gain used for certification).

| Item | Part | Buy (opened 2026-11-04) | Price / stock | Datasheet (verified) |
|---|---|---|---|---|
| Pigtail | Taoglas **CAB.A02**: I-PEX MHF III (W.FL compatible) to **SMA female** bulkhead, 0.81 mm coax, 96.4 mm, nut + washer included | <https://www.sager.com/product/caba02-taoglas/7168042> (Sager, authorised) | **$2.97**, 100 in stock | <https://www.taoglas.com/datasheets/CAB.A02.pdf> (SPE-11-8-118-D): connectors p.1/p.7, 50 Ω p.5, ~0.5 dB over its length |
| Antenna | Taoglas **GW.15.2113**: 2.4–2.5 GHz dipole, **SMA male**, 2 dBi peak | <https://www.sager.com/product/gw152113-taoglas/7168348> (Sager, authorised) | **$4.76**, 530 in stock | <https://www.taoglas.com/datasheets/GW.15.2113.pdf> (SPE-13-8-034-H): 2 dBi peak (500 × 500 mm ground plane), VSWR ≤ 1.8, −40 to +85 °C, p.4 |

- **These are plain SMA, not RP-SMA:** buy the two together.
- **Mount the SMA bulkhead in a plastic part, not through the steel case.** Taoglas's own plots show the dipole's gain rising with the ground plane under it: about 1.5 dBi in free space, about 2.7 dBi on a 150 × 90 mm plane and up to about 6 dBi on a 500 × 500 mm one (datasheet §3.5, §5.4). The steel case would be such a plane and could take it past the module's 2.33 dBi limit. In a plastic part, with the cable's ~0.5 dB loss, it stays under.
- **Not chosen:** the generic AliExpress MHF3 pigtail and Bingfu antenna in earlier versions of this list (no datasheets).

---

## Cost total (verified lines only, 2026-11-04)

| # | Item | Cost |
|---|---|---|
| 1 | Schneider SSP1A125BDT | $24.97 |
| 2 | Auber HS25ET | $13.49 |
| 3 | Crydom EZ240D5 (Enrgtech, £23.05 ≈ $30, FX unverified) | ~$30.00 |
| 4 | Phoenix 1119897, **1 unit** (eBay, saleaxis) | $33.07 |
| 5 | RECOM RAC05-05SK/277/W | $13.85 |
| 6 | Bussmann HTB-26M-R holder + S505-1-R ceramic fuse | $13.82 + $4.38 |
| 7 | DIN rail DN-R35S-150-4 | $8.00 |
| 8 | Wire 16 + 18 AWG | $9.98 + $13.98 |
| 8 | Nilight 400-pc nylon spade kit | $19.77 |
| 8 | Baomain #8 forks, WAGO 221-415, fibreglass sleeve | $7.59 + $22.03 + $8.54 |
| 9 | JST GH 3-pin 300 mm leads, 10-pack (compatible) | $1.14 |
| 10a | Trianglelab 104NT-4 M4 thermistor (regular price) | $6.66 |
| 10b | 1.6 MPa transducer | $24.34 |
| 11 | MHF3→RP-SMA 15 cm pigtail + Bingfu 2 dBi antenna 2-pack | $3.32 + $8.89 |
| | **Total** | **≈ $267.82** |

**Shipping seen on the pages, not in the total:**
- eBay relay $3.99;
- AliExpress: GH leads $5.80, pigtail $2.93, thermistor free;
- Amazon: free over $35 for items shipped by Amazon.
- Enrgtech's UK shipping wasn't recorded.

With those it comes to about **$280**, plus Digi-Key and Crimp Supply shipping, plus Enrgtech shipping, tax and tariffs.

**Ways to cut it further:**
- Nilight 160-pc kit instead of the 400-pc kit: −$8.08 (contents not itemised);
- thermistor at the $1.09 first-piece promo: −$5.57;
- EZ240D5 from RS/Allied at $15.70 (check by hand): about −$14.

## Couldn't verify

1. **Thermistor (10a):** M4 thread pitch, thread and lead length, and the insulation between the leads and the screw body (meter-check it). The element's 100 kΩ / B 4267 K / −50…300 °C figures are datasheet-verified; the probe assembly is listing-only.
2. **Antenna (11):** the generic MHF3 plug's fit on the W.FL receptacle (test-fit), and the Bingfu 2 dBi gain (listing only, no datasheet). The Pulse W3921T0150 and Adafruit 5444 + 944 are still out of stock or unlisted.
3. **Phoenix PLC-RSC-5DC/21:** datasheet verified from Phoenix's French and Chinese catalog exports (English page blocked). The eBay seller isn't an authorised distributor: check the label on arrival.
4. **EZ240D5 US single-unit source:** RS/Allied at $15.70 was seen via Findchips only (check by hand). The opened source is UK-based Enrgtech, with a £→$ conversion I didn't verify.
5. **Fuse and holder:** both datasheet-verified (Eaton TD 2037, TD 2119). Prices at Digi-Key and Crimp Supply as seen, not re-checked at checkout.
6. **Silicone wire:** 600 V / 200 °C from the listing text only, no UL style number. Fibreglass sleeve temperature rating not stated.
7. **JST GH leads:** clone housings, not genuine JST. Check the latch fit in SM03B-GHS-TB before wiring. Genuine JST parts (AGHGH28K305 + GHR-03V-S) are on Digi-Key only (check by hand).
8. **Spade kit:** the 6.3 mm tab width isn't printed in the Nilight listing text. Check one on an SSR tab first.
9. **Pressure transducer:** no datasheet; pinout and ratiometric behaviour unconfirmed.
10. **HS25ET:** stock status not shown on Auber's page.
11. **Mean Well HDR-15-5 alternative:** not checked.
