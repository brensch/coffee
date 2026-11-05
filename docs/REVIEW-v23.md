# Independent electrical review: controller board v23

Reviewed 2026-11-04 against `hardware/controller/controller.py`, `build/parts-120v.json`,
`build/coffee-controller-120v.kicad_pcb` (the v23 board that produced `coffee-v23-gerbers.zip`),
`build/jlc-120v/{bom,cpl}.csv` (identical to `coffee-v23-{bom,cpl}.csv`), `sim/run.py`, the docs and the firmware.
I worked from manufacturer datasheets downloaded fresh, not from `docs/evidence/parts.jsonl`. Where I used a
local copy from `docs/evidence/pdf/`, the finding says so.

**Verdict:** I found no blocker in the PCB itself. The netlist, every pinout, every LCSC part, the strapping
pins, the ADC ranges and the boot-time output state all check out. Six items should be fixed before ordering
or before first mains power-up. Two are hazards from mis-wiring or the wiring plan, not circuit errors (S1, S3).
One is a noise margin that is too thin and costs one value change to fix (S2). The others are docs, firmware and
simulation fixes.

---

## Findings, by severity

### Blocker
None found.

### Should-fix

**S1. J1 (5 V in) takes the same plug as the three outputs. Swapping the leads switches a load on with no way to turn it off.**
- **What happens:** every 3-pin GH lead fits every 3-pin socket. Swap the supply lead and the heater lead, J1 ↔ J2:
  - the supply's +5 V lands on J2.1, which is the board's +5V rail, so the board boots normally;
  - Q4 is a MOSFET with Vgs = −5 V, so it conducts in reverse and puts +5 V on J1.1;
  - the heater SSR, now plugged into J1, sees J1.1 = +5 V and J1.2 = GND. It is on permanently, whatever the firmware does;
  - the supply's GND lands on J2.2, which shorts the drain of Q1, so Q1 has no effect.
- **Only protection left:** the steam thermostat (145 °C) and the thermal fuse.
- **Same with J3 or J8:** the pump or valve runs continuously.
- **Not caught:** the firmware can't detect this, and neither the docs nor the sims mention it. The sims only consider a 5 V lead in J5.
- **Evidence:** the netlist (`parts-120v.json`): J1 = {+5V_IN, GND, GND}, J2/J3/J8 = {+5V, *_OUT, GND}, and Q4 is bidirectional when enhanced. AO3401A datasheet (AOS, <https://www.aosmd.com/res/datasheets/AO3401A.pdf>, p1): RDS(on) 60 mΩ max at −4.5 V, in either direction once enhanced.
- **Fix (pick one):**
  - give J1 a connector nothing else uses, e.g. JST GH 2-pin SM02B-GHS-TB or 4-pin SM04B-GHS-TB (look up the LCSC part; it adds one extended-part fee);
  - or keep the 3-pin and change J1's pinout so that a supply lead in an output socket and an SSR lead in J1 are both harmless. Check every swap combination; the obvious reorderings fail one of them.
- **At minimum:** label every lead at both ends, and add a WIRING.md check: with the board powered, unplug J1 and confirm the board goes off.

**S2. The front-panel switch inputs have only ~0.2 V of low-level noise margin.**
- **Closed switch (v23 topology, R29/R18/R19 1k in series, R28/R20/R21 4.7k pull-up on the GPIO side):**
  - the GPIO sees 3.3 × 1k / (1k + 4.7k) = **0.58 V**;
  - the firmware also turns on the internal pull-up (`control.rs`: `Pull::Up`, 45 kΩ typ), which makes it 3.3 × 1k / (1k + 4.26k) = **0.63 V**.
- **Limit:** the C6 guarantees a low only below VIL max = 0.25 × VDD = **0.825 V**.
- **Why that's too thin:** there's about 0.2 V of margin before LDO tolerance (±1.5 %), the contact resistance of an aged mains rocker, and noise picked up on leads routed beside 120 V wiring and the pump. The input is debounced but has no filter capacitor.
- **Not simulated:** the v23 change of topology isn't in `sim/run.py`.
- **Evidence:** ESP32-C6-MINI-1/1U datasheet v1.5, Table 6-3 (p26): VIL max 0.25 × VDD, VIH min 0.75 × VDD, RPU 45 kΩ typ. <https://documentation.espressif.com/esp32-c6-mini-1_mini-1u_datasheet_en.pdf>
- **Fix:** change R28/R20/R21 to **10k** (C25744 is already on the BOM; no layout change). Closed then reads 0.36 V (0.39 V with the internal pull-up): 0.45 V of margin, with ~0.33 mA wetting current. Or put the pull-up on the connector side of the 1k: closed then reads ~0 V at the pin.

**S3. The plan puts the SELV front-panel input on a mains switch (system-level, not the PCB).**
- **What the plan does:** WIRING.md step 8 keeps one pole of the two-pole power rocker switching live, and turns the other pole into the J7.1/J7.4 dry contact.
- **Why it matters:** the only thing between 120 V and the whole low-voltage side is then the insulation between the rocker's two poles. That side includes the board, the antenna pigtail, the scale in the drip tray, and the PC whenever USB is on the J4 pads. Pole-to-pole insulation of a mains rocker is functional insulation, not the double or reinforced insulation SELV needs. The board's GND isn't earthed, so if that insulation breaks down, nothing trips.
- **Brew and steam switches:** these are fine as long as step 7 really removes every mains wire from them.
- **Fix (either):**
  - leave the power rocker purely on mains (it switches the loads' live), and either drop the POWER input, or read "mains present" a safe way, e.g. from the 5 V supply being fed from the switched live;
  - or move all the mains switching off that rocker, so that only SELV wires touch it.

**S4. The bare USB pads (J4) have no CC resistor, but BUY.md #20 says any USB-C cable will do.**
- **The problem:** v23 dropped the USB-C socket and its 5.1 kΩ Rd resistors. If you cut a USB-C-to-C cable and plug its C end into a USB-C-only laptop, the host sees no Rd on CC. It won't turn on VBUS and won't enumerate the port, so the first flash fails.
- **What works:** a cable with a USB-A plug at the PC end (the cut-off end's Rp resistor goes with it, which is fine). So does a C-to-A adapter.
- **Fix:**
  - docs: in BUY.md #20 and INSTALL §3, say "a cable with a USB-A plug at the PC end";
  - or hardware: add one 5.1 kΩ 0402 from a fifth bare pad (CC) to GND.

**S5. The scale's DOUT lines float when no scale is fitted. Since v23 they also have no series protection.**
- **Floating inputs:** GPIO4/5 are plain inputs (`control.rs`: `InputConfig::default()`, no pull) and have no external resistor. After reset they are IE only (C6 datasheet Table 2-2). With J10 unplugged, the floating inputs can read low. `scale.rs` then clocks out noise and reports `ok = true` with a garbage weight, because "not ready" is only ever detected as DOUT high.
- **No series resistors:** a generic one-supply HX711 module wired to J10.1 (5 V), which WIRING.md warns against, drives 5 V straight into GPIO4/5, both strapping pins. Nothing limits the clamp current.
- **Evidence:** ESP32-C6 datasheet v1.5, Table 2-2 (pp16–17), the "At Reset / After Reset" columns, <https://documentation.espressif.com/esp32-c6_datasheet_en.pdf>. HX711 datasheet (Avia), p4–5: DOUT high = not ready. <https://cdn.sparkfun.com/datasheets/Sensors/ForceFlex/hx711_english.pdf>
- **Fix:**
  - firmware: `InputConfig::default().with_pull(Pull::Up)` on both DOUT pins. A pull-up applied after boot doesn't affect the MTMS/MTDI strap latch, and those straps only set SDIO edges anyway;
  - optional hardware: put back 1k in series on DOUT1/DOUT2. That limits a 5 V mistake to ~1.4 mA, and its RC with ~2 pF is ≪ the 1 µs SCK period.

**S6. The simulations no longer model the v23 circuit, but CIRCUIT.md still claims "61/61 pass".**
- **What's stale in `sim/run.py`:**
  - `revpol()` still includes the 10 kΩ gate resistor (`Rg g 0 10k`) that v23 removed. The DC result happens to be the same.
  - The "Status LED (red, 1k from GPIO23)" check tests a part that no longer exists, and still counts toward 61/61.
  - The `ssr_ez` (Crydom EZ240D5) case is labelled "pump SSR", but the pump is now a G3MB-202P board.
  - `ssr_board` is modelled as a bare 200 Ω across VCC–OUT. The real board puts its own driver transistor (or opto) and LED between VCC and the G3MB input, so "≥ 4.0 V across the module input" overstates what the G3MB sees. G3MB-202P DC5: 440 Ω ± 20 %, operating range 4–6 VDC (Omron datasheet, Input Rating, local `omron_G3MB-202P.pdf` from <https://people.ece.cornell.edu/land/courses/ece4760/FinalProjects/s2004/daw33/RemoteOutlet/datasheets/D20G3MB0503.pdf>). On USB power (4.6 V) that margin is real but small; it's covered by the bring-up check in CIRCUIT.md.
  - The pressure model loads the ADC pin with 1 MΩ. The C6's IIL/IIH is ≤ 50 nA (≥ 60 MΩ equivalent), so the report shows 2.96 V at full scale where the real value is 3.06 V, or 3.08 V worst case with 1 % resistors. It still passes the ≤ 3.1 V limit, but with less margin than reported.
  - The reset-state comment says "4.7k pull-down"; the value is 3.3k.
- **Not modelled at all:** the v23 switch-input topology (S2), the 330 Ω LED drive, the SCK pull-down against U0RXD's reset pull-up, and the J1/output swap (S1).
- **Fix:** update the models and add those checks. Drop the status-LED check.

### Minor

**M1. The steam-ready LED lights and flickers at every boot.**
- **Cause:** J7.7 is driven from U0TXD (GPIO16). After reset that pin is an output pulled high ("WPU, output enabled", C6 datasheet Table 2-2, note 6), and the ROM prints its boot log on it: EFUSE_UART_PRINT_CONTROL defaults to 0, i.e. it always prints (MINI-1U datasheet §4).
- **Effect:** "steam ready" shows for a fraction of a second at every power-up or watchdog reset.
- **Fix:** swap the two nets, LED_STEAM ↔ STATUS. GPIO23 (pad 29) drives R32, and TXD0 (pad 31) goes to TP2. The pads are next to each other.

**M2. SCK sits on U0RXD (GPIO17).**
- **During reset:** R36 (10k) against the internal pull-up (45k typ) holds SCK at ~0.6 V. The HX711 datasheet gives no VIL figure, so I can't confirm this reads low. If it doesn't, the worst case is a power-down followed by a reset when SCK goes low, which is harmless.
- **In download mode:** the ROM sees UART0 RX held low. That doesn't matter, because flashing goes over USB-Serial/JTAG.
- **OK as is.**

**M3. No bulk capacitor sits near the module's 3V3 pin.**
- **Distances:** pad 3 is at (33.1, 7.3) mm. The nearest capacitor is C4 (100 nF, 9 mm away); C3 (10 µF) is about 21 mm away and C7 (22 µF) about 31 mm.
- **What Espressif shows:** the peripheral schematic (MINI-1U datasheet Fig. 9-1, p40) puts 22 µF + 0.1 µF at the pin. The module already has 10 µF + 2 × 0.1 µF inside (module schematic, §8).
- **Status:** acceptable with 0.4 mm tracks, but it's the first suspect if Wi-Fi TX ever browns the module out.
- **Docs:** the BOM note on C3 says "at the 3V3 pin", which isn't where it is.

**M4. Nothing on the board limits overvoltage on +5V.** This is already recorded as an accepted risk.
- **Limits:** the TLV1117LV allows 5.5 V recommended and 6 V absolute (SBVS160C §6.1/6.3).
- **Why a TVS doesn't help:** a 5 V TVS (e.g. SMF5.0A) clamps around 9 V, too high to protect the LDO.
- **Options:** if this should ever be closed, a 5.6 V zener plus a polyfuse at J1, or a crowbar. Otherwise leave it.

**M5. No ESD parts on any cable pin, and no USB series resistors.**
- **Already protected:** the switch and NTC lines have 1k in series, the LEDs 330 Ω, the pressure line 47k.
- **Wired straight to a pin:** the scale lines (S5) and the output drains, the latter on 30 V FETs.
- **USB:** D± go straight to GPIO12/13. Espressif's Fig. 9-1 leaves room for series resistors (R6/R7 "TBD") and caps on those lines; since USB here is only the first flash, that's acceptable.
- **Option:** a 4-line TVS array at J7/J10 would be the cheapest hardening, if it fits.

**M6. +3V3 goes out on J10.2 with nothing in series.**
- **Effect:** a crushed scale lead shorts the 3.3 V rail. The LDO current-limits (≥ 1.1 A) and the C6 browns out. The outputs then drop to off, which is fail-safe, but the board reset-loops.
- **Option:** 10–47 Ω in series on J10.2 (the HX711 DVDD draws ≈ 0.1 mA).

**M7. The thermistor reads "open" in a very cold room.**
- **Numbers:** from Molex's 218537-3919 table (local `molex_218537-3919.pdf`), the node voltage is 3.153 V at 10 °C and 3.184 V at 5 °C. At 0 °C (168 kΩ) it's 3.21 V, past the 3200 mV fault threshold.
- **Effect:** below about 2–3 °C the machine won't heat at all, and the ADC's ±40 mV total error (ATTEN3) moves that threshold by a few °C either way.
- **Status:** fail-safe and already in CIRCUIT.md. Only worth a line in the troubleshooting docs.

**M8. Stale or wrong docs and comments.**
- **WIRING.md check 7** says "J7.1–J7.3 closes when brew is pressed; J7.2–J7.3 closes when steam is pressed". It should be J7.2–J7.4 (brew) and J7.3–J7.4 (steam), plus J7.1–J7.4 (power).
- **WIRING.md, Low-voltage side** says "Every board connector is a JST GH 3-pin header". J7 and J10 aren't.
- **`controller.py`:**
  - the J4 comment says "4 plain plated holes", but the footprint is SMD pads on B.Cu;
  - the R36 comment says "the GPIO side of R33", but there is no R33. R36 *is* on SCALE_SCK in the netlist, so the v19–v22 bug is fixed;
  - the docstring still says "v7".
- **`firmware/esp32/src/main.rs` pin-map table:** pressure "10k/20k" (it's 47k/100k), valve "relay module" and pump "zero-cross SSR" (both are G3MB boards), and "100R" on SCK (removed).
- **CIRCUIT.md** pin map: "23 STATUS LED" (now a test pad).

**M9. Check the JLC placement preview before paying.**
- **CPL rotations (consistent with JLC's usual offsets):** SOT-23 and SOT-223 are KiCad + 180° (Q1–Q3 0 → 180, Q4 180 → 0, U5 180 → 0). U4 is 270. D2 is 0, and its pad 1 = K = +5V was verified against the netlist.
- **What I couldn't check:** I couldn't load JLC's own footprints, so look at each of these in the DFM preview: U4 (W.FL end toward the right-hand board edge), U5 (tab toward the left edge), the J-connector latches, and D2's band toward C6.
- **Clearance:** the module body ends 0.2 mm inside the right-hand board edge, which is fine.

**M10. Machine temperature margins are adequate but not generous.**
- **Ratings:** the X5R capacitors and the JST GH headers are rated to 85 °C, and the GH figure includes the contacts' own temperature rise (JST GH catalogue, p1). Inside the machine it's 60–70 °C.
- **What could break it:** keep the board away from the boiler, and away from the heater SSR's rear-panel area.

### OK: verified

| # | Item | Evidence |
|---|---|---|
| V1 | **ESP32-C6-MINI-1U pad map.** All 53 pads in `controller.py` match Table 3-1 (pins 1–35, GND 1/2/11/14/36–53). The footprint is Espressif's own, and its 0.8 mm pitch and 13.2 × 12.5 mm body match. The firmware pin map (`control.rs` / `main.rs`) matches the netlist GPIO for GPIO | MINI-1U datasheet v1.5, Table 3-1, pp10–11 |
| V2 | **Analog inputs on ADC1:** GPIO0 = ADC1_CH0, 1 = CH1, 2 = CH2, 3 = CH3. The ATTEN3 (`Attenuation::_11dB`) effective range is 0–3300 mV, ±40 mV total error. Pressure reaches 3.06 V at 4.5 V input (3.08 V worst case at 1 %), NTC 3.02 V at 25 °C, V3_SNS 1.65 V, V5_SNS 1.67–1.75 V: all in range | C6 datasheet v1.5, Table 5-6, p66 |
| V3 | **Strapping pins.** GPIO9: internal pull-up, boots from SPI; TP1 to GND gives download mode. GPIO8: R11 10k pull-up, needed for download mode (GPIO8 = 1, GPIO9 = 0). GPIO15 floats, which is the default and only matters with EFUSE_JTAG_SEL_ENABLE burned. MTMS/MTDI (GPIO4/5) only set the SDIO edges, so the HX711's DOUT level at reset is harmless | MINI-1U datasheet §4, Tables 4-1/4-3/4-4, pp12–14 |
| V4 | **EN delay.** R10 10k + C5 1 µF is exactly Espressif's recommendation; tSTBL ≥ 50 µs is met (τ = 10 ms) | MINI-1U datasheet §9 p40; C6 datasheet Table 2-15 p31 |
| V5 | **Outputs stay off through reset and boot.** GPIO18/19/20 are SDIO pins with the weak pull-up (45 kΩ) on at reset and plain inputs after. Against 3.3k the gate sits at 0.23 V, under the AO3400A's Vth min of 0.65 V even after its ~−0.2 V drift at 70 °C. Even a 20 kΩ pull-up only gives 0.47 V. The firmware drives all three low before anything else | C6 datasheet Table 2-2 pp16–17; AO3400A datasheet p1 |
| V6 | **Output stage.** AO3400A: 30 V, ±12 V gate, Vth 0.65/1.05/1.45 V, 48 mΩ max at 2.5 V gate. At VOH min 2.64 V (0.8 × VDD) it's fully on for 10–25 mA loads; IDSS ≤ 1 µA (≤ 5 µA at 55 °C). The Schneider heater SSR picks up at 3 V and drops out at 1 V, and it gets 4.6–5.0 V on and ~0 V off. The G3MB board needs 4–6 V; see S6 for the margin on USB. Without a flyback diode, the SSR inputs aren't inductive. The 3.3k pull-down loads the GPIO with only 1 mA | AO3400A <https://www.aosmd.com/res/datasheets/AO3400A.pdf> p1; Schneider SSP1A125BDT data sheet (local `schneider_ssp1a125bdt.pdf`, from <https://www.se.com/us/en/product/download-pdf/SSP1A125BDT>), control section p1 |
| V7 | **Reverse-polarity FET Q4.** Netlist: G = GND, S = +5V, D = +5V_IN, so the body diode points from the connector to the rail. Reversed input leaves Vgs = 0 and the FET off. Normal input gives Vgs = −5 V, inside ±12 V, and 60 mΩ max. Dropping R26 is fine: Vgs stays within limits even with a hot-plug overshoot to ~10 V | AO3401A datasheet p1 |
| V8 | **LDO U5 (TLV1117LV33DCYR).** SOT-223 pins 1 = GND, 2 + tab = OUT, 3 = IN match the TabPin2 footprint and the netlist. Needs ≥ 1 µF ceramic at the output (≥ 0.5 µF effective), stable at any ESR: C7 22 µF + C3 10 µF X5R is fine. Input cap: C6 22 µF, 2.8 mm away. Dropout 455 mV typ / 700 mV max at 1 A. Current limit ≥ 1.1 A, RθJA 62.9 °C/W, about 0.2 W average → Tj ≈ 85–95 °C at 70 °C ambient, under 125 °C. VIN ≤ 5.5 V. LCSC C15578 is TI's part, not the JSMSEMI part listed under the same name | TI SBVS160C <https://www.ti.com/lit/ds/symlink/tlv1117lv.pdf>, pin table p3, §6.1/6.3/6.4/6.5, §8.2.2.1 p13 |
| V9 | **USB bench-power diode D2 (B5819W).** Netlist pad 1 = K = +5V, pad 2 = A = VBUS, and SOD-123 pad 1 is the cathode (band). 40 V, 1 A, Vf ≤ 0.6 V at 1 A, RθJA 200 °C/W → ~+24 °C at 0.3 A on USB, only while on the bench | JSCJ B5819W datasheet (local `jscj_b5819w.pdf`, <https://datasheet.lcsc.com/datasheet/pdf/4ac2c059be7c462694ab0715ce987a85.pdf?productCode=C8598>) p1 |
| V10 | **Thermistor input.** 4.7k reference with the Molex 50 kΩ ±1 % probe: 1.49 V at 93 °C, about 0.6 V at 140 °C, 3.02 V at 25 °C. An open probe reads the 3V3 rail and a shorted one reads 0 V; both are fail-safe. R27 1k limits a 5 V plug in J5 to ~1.2 mA into the pin, and the 4.7k to 3V3 back-feeds the rail with only 0.36 mA. Self-heating 0.57 mW, with a dissipation factor of 1 mW/°C | Molex 218537-3919 sales drawing (local), notes 1/7, R–T table |
| V11 | **Pressure input.** A 147 kΩ load is over the 100 kΩ the TE M3200 asks for. Gain is 0.680. The −3 dB corner is 1 / (2π · 32 kΩ · 22 nF) = 226 Hz, so the 60 Hz pump pulses pass through. A 5 V fault on the signal gives 3.4 V through 47k, which is current-limited. C9 is 50 V X7R | TE M3200 datasheet (local `te_m3200.pdf`) p2: "Load Resistance > 100 kΩ, Voltage Output" |
| V12 | **Rail sense.** 10k/10k and 20k/10k plus 100 nF; ~0.33 mA total load; inside the ADC range | arithmetic |
| V13 | **LED drive.** 330 Ω from a 3.3 V GPIO into a red LED (Vf ≈ 1.9–2.0 V) gives ~4 mA. A lead shorted to GND gives 10 mA, 33 mW in the resistor: inside both the 62.5 mW 0402 rating and the GPIO's 20 mA default drive | MINI-1U Table 6-3; C6 datasheet Table 2-2 note 3 |
| V14 | **HX711 interface.** DOUT swings 0–3.3 V because DVDD comes from the same 3V3 rail, so nothing back-powers an unpowered C6. The firmware's 1 µs SCK high/low meets T3/T4 (0.2–50 µs) and T2 ≤ 0.1 µs, and the critical section avoids the 60 µs power-down | HX711 datasheet pp4–5 |
| V15 | **JST GH headers.** C514175, C495552 and C133065 are JST SM03B/SM07B/SM06B-GHS-TB(LF)(SN). Rated 1 A per contact, 50 V, −25 to +85 °C. J1 carries < 0.4 A | JST GH catalogue (local `jst_sm03b-ghs-tb.pdf`, <https://www.jst-mfg.com/product/pdf/eng/eGH.pdf>) p1–2 |
| V16 | **Netlist.** All 47 placed parts appear in both the BOM and the CPL. KiCad DRC: 0 unconnected, and the 50 "violations" are all library-table warnings. Power nets are 0.4 mm tracks and GND is poured. 0402 capacitor voltages: 16 V on ≤ 3.3 V nets, 25 V on EN, 50 V on P_ADC. The 0805s are 25 V on 5 V and 3.3 V | `coffee-controller-120v-drc.rpt`; PCB parse |
| V17 | **Parts removed in v23.** No USB-C socket or CC resistors: fine for function, but see S4. No status LED: fine, the signal is on TP2; see M1. No D5: fine, all three loads are SSRs. No R26: fine (V7). No 100 Ω on the scale lines: they never limited much, and S5 covers what's left. All three removals are reflected consistently in the netlist | netlist |
| V18 | **Module temperature rating.** The -H4 module is rated −40 to 105 °C ambient | MINI-1U datasheet Table 6-2 p26, ordering table p6 |
| V19 | **Antenna connector.** The MINI-1U uses a third-generation connector, compatible with Hirose W.FL, I-PEX MHF III and Amphenol AMMC. The 1U has no keep-out zone | MINI-1U datasheet §10.2, §3.1 |

---

## Part-by-part verification

LCSC/MPN was checked against the live LCSC product-page title (`https://www.lcsc.com/product-detail/<C#>.html`) on 2026-11-04.

| Ref | Part (LCSC) | MPN / maker confirmed | Verified against datasheet | Source |
|---|---|---|---|---|
| U4 | ESP32-C6-MINI-1U-H4 (C20627095) | yes, Espressif | **yes** (pins, straps, IO reset states, VIL/VIH/VOH, ADC, EN, temperature) | MINI-1U datasheet v1.5; C6 datasheet v1.5 |
| U5 | TLV1117LV33DCYR (C15578) | yes, TI | **yes** | TI SBVS160C |
| Q1–Q3 | AO3400A (C20917) | yes, AOS | **yes** (pinout, Vth, RDS(on), VGS, IDSS) | AOS AO3400A |
| Q4 | AO3401A (C15127) | yes, AOS | **yes** | AOS AO3401A |
| D2 | B5819W SL (C8598) | yes, JSCJ | **yes** | JSCJ datasheet (LCSC copy) |
| J1, J2, J3, J5, J6, J8 | SM03B-GHS-TB(LF)(SN) (C514175) | yes, JST | **partly**: ratings yes; KiCad pad 1 matches the netlist; JLC rotation not seen (M9) | JST GH catalogue |
| J7 | SM07B-GHS-TB(LF)(SN) (C495552) | yes, JST | **partly**, as above | JST GH catalogue |
| J10 | SM06B-GHS-TB(LF)(SN) (C133065) | yes, JST | **partly**, as above | JST GH catalogue |
| C6, C7 | CL21A226MAQNNNE (C45783) | yes, Samsung | **partly**: 22 µF / 25 V / X5R / 0805 from the LCSC listing and Samsung part-number decode; Samsung PDF not opened; DC-bias derating not checked (stability needs only ≥ 0.5 µF) | LCSC |
| C3 | CL21A106KAYNNNE (C15850) | yes, Samsung | **partly** (10 µF 25 V X5R; a Samsung PDF exists in `docs/evidence/pdf`, not re-read) | LCSC; evidence copy |
| C5 | CL05A105KA5NQNC (C52923) | yes, Samsung | **partly** (1 µF 25 V X5R 0402) | LCSC |
| C4, C8, C10, C11 | CL05B104KO5NNNC (C1525) | yes, Samsung | **partly** (100 nF 16 V X7R 0402) | LCSC |
| C9 | 0402B223K500NT (C1532) | yes, FH | **partly** (22 nF 50 V X7R) | LCSC |
| R4–R6 3.3k, R10/R11/R22/R23/R25/R36 10k, R15/R20/R21/R28 4.7k, R16 47k, R17 100k, R18/R19/R27/R29 1k, R24 20k, R30–R32 330 | UNI-ROYAL 0402WGFxxxxTCE (C25890, C25744, C25900, C25792, C25741, C11702, C25765, C25104) | yes | **partly** (1 %, 62.5 mW from the LCSC listing; worst dissipation 33 mW, R30–R32 with a shorted LED lead) | LCSC |
| TP1, TP2, J4 | bare pads on B.Cu | n/a | n/a | PCB |
| Off-board: heater SSR | Schneider SSP1A125BDT | n/a | **yes** (control section: 3–32 V, pick-up 3 V, drop-out 1 V, 10 mA at 12 V, −40 to 80 °C) | Schneider data sheet (local copy) |
| Off-board: pump/valve SSR | Omron G3MB-202P DC5 on a generic board | n/a | **partly**: relay yes (4–6 V, 440 Ω ±20 %); board circuit unknown (no datasheet) | Omron datasheet (Cornell mirror) |
| Off-board: NTC | Molex 218537-3919 | n/a | **yes** (R–T table, 1 mW/°C) | Molex sales drawing (local) |
| Off-board: transducer | GaggiMate 0–1.6 MPa, 0.5–4.5 V | n/a | **no** (no datasheet exists); TE M3200's > 100 kΩ load used as the reference | TE M3200 (local) |
| Off-board: scale amplifier | HX711 (dual board) | n/a | **yes** for the chip (timing, supplies); **no** VIL/VIH figure in the datasheet | Avia HX711 |

**Not reached:** AliExpress listings (5 V module, SSR boards) were not opened, and no CAPTCHA pages came up. The Schneider product web page gave only the control-voltage range, so the control specs come from the local PDF copy.
