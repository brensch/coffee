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

Every item here is checked against the maker's own datasheet or product page. GaggiMate (an open
controller for this machine) confirms the machine side: all switch, thermostat and pump
connections are 6.35 mm quick-connects (gaggimate docs).

| What | Exact part | Qty | Buy (seen 2026-11-04) | Price / stock | Datasheet (verified) |
|---|---|---|---|---|---|
| **14 AWG** silicone, heater-SSR legs M10/M11 (11.4 A) | Tensility **30-03445** (red), cut to length | 1 m | <https://www.digikey.com/en/products/detail/tensility-international-corp/30-03445/25572263> (open by hand) | ~€3.50/m, 121 m in stock (Digi-Key EU site; US price not seen) | Tensility spec sheet rev A 2024: **UL AWM 3530, file E339579, 200 °C, 600 V**, cUL, FT2; 41 × 0.254 mm tinned, OD 3.44 mm (fits the 16-14 FASTONs: TE allows insulation to 6.6 mm) |
| 16 AWG silicone (M1, M4, M9) | CnC Tech **3135-16-1-0500-004-1-TS** (red), by the foot | 10 ft | <https://www.digikey.com/en/products/detail/cnc-tech/3135-16-1-0500-004-1-TS/28739631> (Digi-Key: open by hand, it shows automated browsers a verification page) | ~€1.38/ft, 351 ft in stock (Digi-Key's EU site; US price not seen) | CnC datasheet <https://www.cnctech.us/pdfs/3135-XX-1-0500-0XX-1-TS_.pdf> p.1: **UL AWM 3135, 600 V, 200 °C**, C-UL SEWF-2, UL file **E205197**, VW-1/FT1, OD 3.02 mm. (Digi-Key's listing says 150 °C; the datasheet governs.) |
| 18 AWG silicone (M2, M3, M6, M7, M8) | CnC Tech **3135-18-1-0500-005-1-TS** (blue), by the foot | 15 ft | <https://www.digikey.com/en/products/detail/cnc-tech/3135-18-1-0500-005-1-TS/3766970> (open by hand) | ~€1.33/ft, 3,613 ft in stock (EU site) | same datasheet, OD 2.72 mm |
| 22-18 AWG insulated female 6.35 mm | TE **2-520183-2** | 12 | <https://www.te.com/en/product-2-520183-2.html> (TE store, min 1) | $0.516, 323,686 in stock | TE page: nylon, 600 V, −40…110 °C, UL. 22-18 AWG only |
| 16-14 AWG insulated male 6.35 mm | TE **3-520107-2** | 6 | <https://www.te.com/en/product-3-520107-2.html> | $0.664, 36,922 in stock | TE page: nylon, 600 V, UL; **TE spec 108-2017 p.3: 10 A on 16 AWG, 15 A on 14 AWG, 105 °C max** |
| #8 fork, 16-14 AWG (heater SSR output) | TE **325199** (PIDG) | 3 | <https://www.te.com/en/product-325199.html> | $2.11, 2,558 in stock | TE page: #8 / M4 stud, 300 V, 0…105 °C, UL file E13288 |
| L and N junctions | WAGO **221-415** lever nuts, 25-pack | 1 (uses 2) | <https://www.amazon.com/dp/B0107SYYGU> (third-party seller) | $22.03 | WAGO page <https://www.wago.com/us/wire-splicing-connectors/compact-splicing-connector/p/221-415> + UL certificate: **US rating 600 V / 20 A**, UL 486C, file E69654; 24-12 AWG, 11 mm strip; **max ambient 85 °C**, so mount them away from the boiler |
| Fibreglass sleeve, wire runs | Techflex **SFAG.08** (3.4 mm), 10 ft | 1 | <https://www.wirecare.com/product-lines/fr-silicone-flex-glass-grade-a> (dealer listed on techflex.com) | $16.52 | Techflex catalog <https://assets.techflex.com/assets/products/catalogs/pdf/sfg.pdf>: silicone-coated fibreglass, **200 °C**, Grade A 8 kV, VW-1 |
| Fibreglass sleeve, joints | Techflex **SFAG.02** (6.7 mm), 10 ft | 1 | same dealer | $25.82 | same |

**Why 14 AWG on the heater legs:** TE rates these 6.35 mm FASTONs at **10 A on 16 AWG and 15 A on 14 AWG**
(product specs 108-2017 and 108-2043). The heater draws 11.4 A, so 16 AWG would run the terminals over
their rating. Everything else on the mains side carries under 1 A.

**Crimps used** (matches WIRING.md):
- 22-18 female (2-520183-2): **11**: M2, M3, M6, M7 ×2, J3 ×2, J7 ×4;
- 16-14 male (3-520107-2): **5**: M1, M4, M9 (16 AWG) and M10, M11 (14 AWG);
- #8 fork (325199): **2**: M10, M11.

Buy one or two spares of each for practice crimps.

Notes:
- **Joints near the boiler:** the nylon terminals are rated 105–110 °C and the boiler runs at 93–145 °C, so every nylon joint stays ≥ 50 mm off it, under SFAG.02 sleeve (WIRING.md).
- **Not chosen** (no maker datasheet): the BNTECHGO silicone wire, Nilight spade kit, Baomain forks and uxcell sleeve of earlier versions. Also rejected: TE 63301-1 bare high-temperature tab (made for solid nichrome heater wire on machine-crimp strips, no current rating), and GaggiMate's 18 AWG for the heater path (TE allows 7 A per terminal there).
- **Optional, not in the total:** with a ferrule crimper, 0.25 mm² ferrules on the GH-lead ends at the heater SSR input and the relay coil, and 1.5 mm² on the relay contacts.

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

### 10b. Pressure transducer: TE Connectivity M3200 series, qty 1 (**variant: your choice, see below**)

**Requirement:** 0–~17 bar gauge, 5 V supply, 0.5–4.5 V ratiometric, G1/4 into GaggiMate-style tee
(GaggiMate's pressure tee takes a G1/4 male transducer and sits between the pump outlet and the
OPV, which are G1/8: gaggimate docs), water above 95 °C, load ≤ the board's 147 kΩ.

**Datasheet (verified):** TE M3200, 06/2026 (local copy in the research notes; te.com product pages):
0.5–4.5 V ratiometric on 5 ± 0.25 V (p.8); **load > 100 kΩ** (p.2; the board's divider is 147 kΩ);
±0.25 % FS, total error band ±1.5 % FS (p.2); proof 2×, burst 5×; −40…125 °C (cable versions 105 °C);
17-4PH stainless wetted parts. Every variant below is in this one datasheet.

| Variant | Connection | Port | Buy | Catch |
|---|---|---|---|---|
| **M323M-000004-017BG** | 1 m cable: red +5 V, black GND, white output; vent in the cable (p.4) | 7/16-20 UNF male, O-ring (p.5) | Digi-Key via te.com <https://www.te.com/en/product-20014203-07.html>: **$104.81**, 36 in stock | Needs a 7/16-20 UNF female → G1/4 male adapter. No stainless adapter with its own datasheet found yet (Adaptall 9036-04-04 is carbon steel, unrated) |
| **M3234-000002-250PG** | Packard A connector: A +5 V, B GND, C output; vent through the connector | 1/4-19 BSPP = G1/4 (p.5) | Digi-Key via te.com <https://www.te.com/en/product-20005027-00.html>: **$90.84**, 4 in stock | TE names no mating plug. Sibling TE datasheets (M5200, U5300) name Aptiv **12078090** + 3 × **12103881**, but that plug's seals may block the gauge vent |
| **M323M-000002-017BG** | 1 m cable | G1/4 | factory order only (no stock found) | Lead time |

- **Firmware:** `range_bar` is 17.24 for the 250 psi part and 17.0 for the 017BG parts (set it in the config).
- **Not chosen:**
  - GaggiMate's own transducer, an unbranded AliExpress 0–1.6 MPa part with no datasheet;
  - the XYNLHZQ listing in earlier versions of this list (no datasheet);
  - WIKA A-10: its ratiometric version is limited to 0–80 °C water;
  - Gems 3500: $330, minimum 5;
  - Variohm EPT3100: its current datasheet sits behind a form, and the 2013 sheet doesn't state the load or ratiometric output;
  - Honeywell PX2 (not for water) and PX3 (no cable version).

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

## Cost total (2026-11-04, prices as seen; several Digi-Key prices still to confirm by hand)

| # | Item | Cost |
|---|---|---|
| 1 | Schneider SSP1A125BDT | $24.97 |
| 2 | Auber HS25ET | $13.49 |
| 3 | Crydom EZ240D5 (Enrgtech, £23.05 ≈ $30, FX unverified) | ~$30.00 |
| 4 | Phoenix 1119897, 1 unit (eBay) | $33.07 |
| 5 | RECOM RAC05-05SK/277/W | $13.85 |
| 6 | Bussmann HTB-26M-R holder + S505-1-R fuse | $13.82 + $4.38 |
| 7 | DIN rail DN-R35S-150-4 | $8.00 |
| 8 | Wire: Tensility 14 AWG 1 m + CnC 16 AWG 10 ft + 18 AWG 15 ft (EU prices ×1.1) | ~$4 + ~$15 + ~$22 |
| 8 | TE terminals (12 + 6 + 3) | $6.19 + $3.98 + $6.32 |
| 8 | WAGO 221-415 25-pack | $22.03 |
| 8 | Techflex SFAG.08 + SFAG.02 sleeve | $16.52 + $25.82 |
| 9 | JST AGHGH28K305 ×21 + GHR-03V-S ×7 (Digi-Key, price not seen) | ~$30 (estimate, check) |
| 10a | Molex 218537-3919 | $9.76 |
| 10b | TE M3200 transducer (variant to choose) | $90.84–$104.81 |
| 11 | Taoglas CAB.A02 + GW.15.2113 | $2.97 + $4.76 |
| | **Total** | **≈ $400–415**, plus shipping |

The verified parts cost more than the earlier unverified ones (≈ $268), mostly the transducer
(+$70–80), the sleeve (+$34) and genuine JST leads.

## Couldn't verify

Everything above was checked against the maker's own datasheet or product page, except:
1. **Prices behind human-verification pages** (Digi-Key, Mouser): the 14/16/18 AWG wire, the JST GH leads, the S505-1-R fuse, and the TE transducer's Digi-Key stock. Open those links yourself.
2. **Transducer:** the M323M's G1/4 adapter (no stainless part with a datasheet found yet) or the M3234's mating plug (only named in sibling TE datasheets). See §10b.
3. **The machine itself:** no datasheet exists. Machine facts come from GaggiMate's docs (6.35 mm quick-connects, M4 at the brew-thermostat hole, G1/8 at the pump/OPV, G1/4 tee port). Check the thermostat screws are M4 before fitting the probe.
4. **Phoenix relay:** read from Phoenix's French and Chinese datasheet exports (the English page is blocked). The eBay seller isn't an authorised distributor.
5. **EZ240D5 on the Ulka pump:** the datasheet doesn't cover a half-wave (diode) pump load; bench-test it (INSTALL.md §6).
