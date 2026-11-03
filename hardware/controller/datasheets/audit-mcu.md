# Audit: MCU and low-voltage section (controller v6 board, layout v11)

Scope:
- U4 ESP32-C6-MINI-1U-H4, with its strapping, EN, decoupling, ADC, GPIO drive and USB;
- J4 USB-C (GCT USB4105-GF-A), with its CC resistors and VBUS path;
- J5–J7 (JST PH);
- the NTC divider (R15/C8), the pressure divider (R16/R17/C9), and the status LED (D1/R12);
- how the firmware configures each of these pins.

Audited 2026-11-03 against `hardware/controller/controller.py`, the netlist and PCB in `build/`,
`build/jlc-120v/*.csv`, `docs/CIRCUIT.md`, `docs/HARDWARE.md`, `docs/INSTALL.md`, and
`firmware/esp32/src/*.rs` (esp-hal 1.2.2) plus `crates/coffee-core` (sensor decoding). Nothing
was edited.

## Sources

| Ref | Document | URL |
|---|---|---|
| [MOD] | ESP32-C6-MINI-1 & MINI-1U Datasheet **v1.5** | https://www.espressif.com/sites/default/files/documentation/esp32-c6-mini-1_mini-1u_datasheet_en.pdf |
| [CHIP] | ESP32-C6 Series Datasheet **v1.5** | https://www.espressif.com/sites/default/files/documentation/esp32-c6_datasheet_en.pdf |
| [HDG] | ESP32-C6 Hardware Design Guidelines, schematic checklist | https://docs.espressif.com/projects/esp-hardware-design-guidelines/en/latest/esp32c6/schematic-checklist.html |
| [GCT] | GCT USB4105 drawing | https://gct.co/files/drawings/usb4105.pdf |
| [JST] | JST PH series (ePH) | https://www.jst-mfg.com/product/pdf/eng/ePH.pdf (the JST site returned HTML to the script; values taken from distributor mirrors of the same sheet, e.g. https://www.jst.co.uk/downloads/series/ePH-H_(19-04-02).pdf) |
| [LCSC-LED] | LCSC C12624, KENTO KT-0603G | https://www.lcsc.com/product-detail/C12624.html |
| [LCSC-MOD] | LCSC C20627095, ESP32-C6-MINI-1U-H4 | https://www.lcsc.com/product-detail/C20627095.html |
| [ESPHAL] | esp-hal 1.2.2 source (`gpio/mod.rs`, `analog/adc/mod.rs`) | ~/.cargo/registry/src/*/esp-hal-1.2.2 |

---

## 1. ESP32-C6-MINI-1U-H4: pads, footprint, grade

| Claim | Where made | Datasheet value + section | Verdict |
|---|---|---|---|
| Pad map `_c6`: 1,2 GND · 3 3V3 · 4 NC · 5 IO2 · 6 IO3 · 7 NC · 8 EN · 9 IO4 · 10 IO5 · 11 GND · 12 IO0 · 13 IO1 · 14 GND · 15 IO6 · 16 IO7 · 17 IO12 · 18 IO13 · 19 IO14 · 20 IO15 · 21 NC · 22 IO8 · 23 IO9 · 24 IO18 · 25 IO19 · 26 IO20 · 27 IO21 · 28 IO22 · 29 IO23 · 30 RXD0 · 31 TXD0 · 32–35 NC · 36–53 GND | controller.py `_c6` | [MOD] Table 3-1: identical for every pad. GND = 1, 2, 11, 14, 36–53; NC = 4, 7, 21, 32–35. | OK |
| All GND pads (including the 9 sub-pads of EPAD 49 and corners 50–53) go to GND; NC pads are left unconnected | controller.py (loop over `u_mcu.pins`) | [MOD] Table 3-1; §9 notes that soldering the EPAD is optional but improves thermal performance | OK |
| Vendored footprint: 53 pads, pins 1–11 down the left, 12–24 along the bottom, 25–35 up the right, 36–48 the inner row (48 at left, 36 at right), corners 50 TR / 51 BR / 52 BL / 53 TL, 3×3 EPAD | lib/Espressif.pretty/ESP32-C6-MINI-1U.kicad_mod, lib/README.md | [MOD] Fig. 3-1 (pin layout) and Fig. 11-2: 48 × 0.40 × 0.80 pads, 0.8 pitch, 4 × 0.70 corners, outer spans 11.80 / 11.90 and 9.80 / 9.90 mm, EPAD 5.40 mm with 1.45 sub-pads, body 13.2 × 12.5. Footprint: pads at ±5.9 (span 11.8), corners at ±5.95 (11.9), rows at y −2.2 / 7.6 (9.8), corners at −2.25 / 7.65 (9.9), sub-pads 1.45 on a 1.975 pitch (3·1.45 + 2·0.525 = 5.40). | OK |
| Net on every used pad, checked in the routed PCB | build/coffee-controller-120v.kicad_pcb | pad 3 +3V3, 8 EN, 12 P_ADC, 13 NTC, 15 SW_BREW, 16 SW_STEAM, 17 USB_D−, 18 USB_D+, 22 IO8_STRAP, 23 IO9_BOOT, 24 VALVE_CMD, 25 PUMP_CMD, 26 HEATER_CMD, 29 STATUS. All match Table 3-1 and FIRMWARE_PINS. | OK |
| "-H4": 4 MB flash, rated −40 to 105 °C | controller.py, CIRCUIT.md, HARDWARE.md, main.rs | [MOD] Table 1-2: ESP32-C6-MINI-1U-H4 = 4 MB quad SPI (in-package), −40 to 105 °C ambient. Table 6-2: TA up to 105 °C for the 105 °C version. [LCSC-MOD] lists the same part. | OK |
| Flash endurance "centuries" | HARDWARE.md | [MOD] Table 6-9: 100,000 P/E cycles, 20-year retention. 640 KB × 100k ÷ 60 KB/day ≈ 2,900 years with ideal levelling. Retention (20 years), not wear, is the real limit. | OK |
| **"U.FL antenna connector"; "U.FL (IPEX MHF1) pigtail"** | controller.py (ESP32C6 description, U4 notes), README.md, CIRCUIT.md, HARDWARE.md off-board BOM, INSTALL.md ("U.FL (IPEX MHF1)"), lib/README.md, audit.py, bom-120v.csv | [MOD] §10.2: the MINI-1U uses the **third-generation** connector, compatible with **Hirose W.FL, I-PEX MHF III and Amphenol AMMC**. MHF III is smaller than U.FL / MHF I, and the two don't mate. §10.2 also says the certified antenna gain is ≤ 2.33 dBi; a different antenna type or higher gain may need more EMC testing. | **WRONG** |
| Supply from the TLV1117LV33 (1 A) | controller.py | [MOD] Table 6-2: VDD33 3.0–3.6 V, external supply must deliver ≥ 0.5 A. Table 6-4: 382 mA peak TX (802.11b at 20.5 dBm). The LDO is fine. The 5 V budget (PSU 0.6 A) is ≈ 0.38 A + 3 × 12 mA optos + ~10 mA transducer + LED ≈ 0.43 A peak: OK but not generous (power-supply audit). | OK |
| Reflow | (assembly; v10+ is two-sided) | [MOD] §12.3: **"Solder the module in a single reflow."** U4 is on top, and C3–C9, R10/R11, R13–R17, D2 and U5 are on the bottom (`cpl.csv`). If JLC reflows the top side first, the module sees two reflows. | RISK |

## 2. Strapping, EN, boot, JTAG

| Claim | Where made | Datasheet value + section | Verdict |
|---|---|---|---|
| Strapping pins are GPIO4/5/8/9/15 | controller.py `STRAPS`, CIRCUIT.md | [MOD] §4 / [CHIP] §3: boot mode GPIO8+GPIO9; SDIO edge MTMS (GPIO4) + MTDI (GPIO5); ROM print GPIO8; JTAG source GPIO15 | OK |
| GPIO9 floats on its internal pull-up → SPI boot; shorting TP1 to GND at power-up → download | controller.py, CIRCUIT.md | [MOD] Table 4-1: GPIO9 has a weak pull-up (1) by default. Table 4-3: SPI boot = GPIO9 1 (GPIO8 any value); joint download (USB-Serial-JTAG / UART / SDIO) = GPIO8 1, GPIO9 0. [HDG]: "It is recommended to place a pull-up resistor at the GPIO9 pin"; "do not add high-value capacitors at GPIO9". | OK (RISK: only the 45 kΩ internal pull-up on a net with a bare pad and a trace, near mains) |
| GPIO8 needs its 10k pull-up for download mode | controller.py R11, CIRCUIT.md | [MOD] Table 4-1: GPIO8 floats by default. Table 4-3: download needs GPIO8 = 1. Espressif's own peripheral schematic ([MOD] Fig. 9-1, R8 10K) does the same. | OK |
| GPIO15 left unconnected | controller.py check 4 | [MOD] §4.4: GPIO15 has no internal pull and "must be controlled by the external circuit". Table 4-7: with the default eFuses (EFUSE_JTAG_SEL_ENABLE = 0, DIS_PAD_JTAG = 0, DIS_USB_JTAG = 0), GPIO15 is **ignored** and JTAG comes from USB Serial/JTAG. | OK while the eFuses are untouched |
| MTMS/MTDI (GPIO4/5) left unconnected | controller.py | [MOD] Table 4-4: they only pick the SDIO-slave clock edges, and SDIO is unused | OK |
| GPIO6/7 (JTAG MTCK/MTDO) used as switch inputs; called "plain GPIOs" | controller.py comment, firmware | [CHIP] Table 2-9 and §2.3.4: GPIO6 = MTCK (also ADC1_CH6), GPIO7 = MTDO. They are the JTAG pads, but with the default eFuses JTAG is routed to USB ([MOD] Table 4-7), so the pads are free. [CHIP] Table 2-1: MTCK has IE + WPU after reset (DIS_PAD_JTAG = 0); MTDO has IE. A closed switch at boot does nothing harmful. Pad JTAG debugging is no longer possible, but USB JTAG still works. | OK (the "plain GPIOs" wording is inaccurate) |
| EN RC: R10 10k / C5 1 µF X7R (τ = 10 ms) | controller.py | [MOD] §9: "recommended … R = 10 kΩ and C = 1 µF". [MOD] Table 4-8 / [CHIP] Table 2-15: t_STBL ≥ 50 µs. [HDG]: CHIP_PU must not float. | OK (R10/C5 are 10–13 mm from pad 8, on the bottom side; acceptable) |
| espflash resets into download mode over USB, so no buttons are needed | CIRCUIT.md | USB Serial/JTAG supports joint download boot ([MOD] Table 4-3, note 2); the software reset path goes through the USB Serial/JTAG controller. TP1/TP2 is the fallback. | OK |
| Loads are off at boot "because a floating or weakly-pulled pin can't push 5 mA" | controller.py, CIRCUIT.md | [CHIP] Table 2-1: GPIO18/19/20 (SDIO_CMD/CLK/DATA0) and GPIO23 (SDIO_DATA3) have **WPU at reset** and only IE after reset. Through a 45 kΩ pull-up (Table 5-4) the opto LED gets ≈ (3.3 − 1.0)/45k ≈ 50 µA, about 1 % of IFT. The reasoning holds, but the pins are pulled up during reset, not floating. | OK |

## 3. GPIO drive (opto LEDs, status LED)

| Claim | Where made | Datasheet value + section | Verdict |
|---|---|---|---|
| GPIO → 150 Ω → MOC3083 LED gives 8.2–12.1 mA; needs ≥ 5 mA | controller.py, CIRCUIT.md, sim/REPORT.md | [CHIP] Table 5-4 / [MOD] Table 6-3: IOH = 40 mA at VOH ≥ 2.64 V (VDD 3.3 V) **at PAD_DRIVER = 3**. Default drive is 20 mA (PAD_DRIVER = 2; [CHIP] Table 2-1 note 3). | OK, with a mismatch: see next row |
| Firmware drive strength | control.rs `OutputConfig::default()` | [ESPHAL] `OutputConfig::default()` = PushPull, **DriveStrength::_20mA**, no pull. sim/run.py models "~25 Ω at drive strength 3". At drive strength 2 the output resistance is about 33 Ω (VOH spec scaled; not in the datasheet: UNVERIFIED). Worst case (3.0 V rail, Vf 1.5 V): (3.0 − 1.5)/(150 + 33) ≈ **7.9 mA**, against 8.2 mA simulated. Still above the 5 mA IFT max. | RISK (sim and firmware disagree) |
| Total IO current | — | [CHIP] Table 5-1: cumulative IO output ≤ 1000 mA. The board uses ~40 mA. | OK |
| D1 green LED + R12 1k from GPIO23 | controller.py; jlc.py picks C12624 | [LCSC-LED]: KENTO KT-0603G is **emerald green InGaN, Vf 3.1 V at 5 mA**, rated −40 to +85 °C. From 3.3 V through 1 kΩ that leaves ≈ 0.2–0.6 mA, and less on a 3.0 V rail: dim, and very dim behind a light pipe. | RISK |

## 4. ADC: NTC and pressure

| Claim | Where made | Datasheet value + section | Verdict |
|---|---|---|---|
| ADC1 = GPIO0–6; NTC on GPIO1 (ADC1_CH1), pressure on GPIO0 (ADC1_CH0) | controller.py `ADC1`, check 3, docs | [MOD] Table 3-1 / [CHIP] Table 2-9: ADC1_CH0…CH6 = GPIO0…GPIO6. The C6 has a single 12-bit SAR ADC, 7 channels ([CHIP] §4.2.2.1). GPIO0/1 double as XTAL_32K_P/N, and the module fits no 32 kHz crystal. | OK |
| Firmware: `Attenuation::_11dB` = "12 dB, 0–3.3 V", curve calibration from eFuse | control.rs | [ESPHAL] `_11dB = 0b11` = ATTEN3. [CHIP] Table 5-6: ATTEN3 effective range **0–3300 mV, total error ±40 mV** (after hardware + software calibration, 25 °C, Wi-Fi off, 100 nF at the pin). Applies to chips from date code 2321 / modules from PW-2023-06 on ([MOD] §5.1). DNL −8/+12 LSB, INL ±10 LSB (Table 5-5). | OK |
| 100 nF at each ADC pin | controller.py C8, C9 | [CHIP] §5.5 measures with a 100 nF capacitor; [HDG] recommends 0.1 µF at ADC pins. C8/C9 are ~7 mm from pads 12/13 (bottom side). | OK |
| NTC: 100k B3950 to GND, R15 8.2k 0.1 % to 3V3: "3.05 V at 25 °C, ~1.65 V at 95 °C, 0.7 V at 145 °C" / "3.1 V at 20 °C, 1.68 V at 93 °C, 0.65 V at 150 °C, 24 mV/°C at brew" | controller.py R15 note, CIRCUIT.md | Recomputed (β model, 3.300 V): 0 °C 3.221 V; 5 °C 3.199 V; 20 °C 3.097 V; 25 °C 3.050 V; 93 °C 1.683 V (24.2 mV/°C); 95 °C 1.635 V; 145 °C 0.706 V (12.4 mV/°C); 160 °C 0.542 V (9.4 mV/°C). The whole 20–160 °C span sits inside ATTEN3's 0–3300 mV. The ±40 mV total error alone equals ±1.7 °C at 93 °C, ±3.2 °C at 145 °C and ±4.5 °C at 20 °C before the two-point calibration. | OK |
| Which NTC | controller.py J5, HARDWARE.md | The Gaggia Classic Pro has no stock NTC; its brew and steam controls are bimetal thermostats. The design assumes a generic aftermarket "100k B3950 ±1 % M4" probe, with no MPN. B3950 is normally specified as B25/50; using β up to 150 °C adds model error of a few °C, which the firmware's optional Steinhart–Hart coefficients can remove. | UNVERIFIED (no part chosen) |
| NTC self-heating 0.33 mW | CIRCUIT.md | Maximum at R_ntc = R15: (1.65 V)² / 8.2 kΩ = 0.33 mW | OK |
| NTC conversion assumes `v_ref_mv = 3300` | crates/coffee-core config.rs | The divider follows the 3V3 rail, but the ADC uses an internal reference, so the reading is **not ratiometric**. A 1.5 % rail error moves the 93 °C reading by ≈ 1 °C (3.25 V → reads 94.1 °C; 3.35 V → reads 92.0 °C). The two-point calibration only removes the static part. | RISK |
| Fault thresholds: open ≥ 3250 mV, short ≤ 100 mV | config.rs `NtcConfig` | Nominal node voltage is 3.199 V at 5 °C and 3.221 V at 0 °C, which leaves 29–51 mV before the "open" threshold, against ±40 mV ADC error ([CHIP] Table 5-6) plus the rail tolerance. Result: a **false "open" in a cold room (≲ 5 °C)**. The other direction is worse: with the rail at 3.25 V and the ADC reading −40 mV, an **open probe reads ≈ 3.21 V, i.e. "≈ 3 °C"**, so the open isn't detected and the probe looks cold. | **WRONG (firmware margin)** |
| Pressure: 0.5–4.5 V on +5 V, ÷ 2/3 → 0.333–3.00 V; ≤ 3.1 V is inside the 12 dB range | controller.py, CIRCUIT.md, REPORT.md | 3.00 V < 3.3 V effective range. Overrange and faults: a transducer saturated at its supply gives 5.0 × 2/3 = 3.33 V, or 3.5 V at a 5.25 V rail. That stays below VDD + 0.3 = 3.6 V ([MOD] Table 6-3 V_IH max; [CHIP] Table 5-1), and the 6.7 kΩ source limits any injection to ≪ 1 mA. No damage. | OK |
| Filter corner ~230–240 Hz | controller.py C9, CIRCUIT.md | 10k ∥ 20k = 6.67 kΩ with 100 nF → f_c = 239 Hz | OK |
| Pressure accuracy | CIRCUIT.md ("calibration and a careful self-zero") | ATTEN3 ±40 mV = ±0.24 bar uncalibrated (166.7 mV/bar at the pin). The transducer output is ratiometric to +5 V, but the ADC is absolute and +5 V isn't measured, so any +5 V error becomes a **gain** error: 2 % = 0.18 bar at 9 bar. On USB, +5 V is 4.52 V, which is 10 % low and below most 5 V transducers' supply spec (UNVERIFIED, no part chosen). | RISK |
| Pressure faults | config.rs `fault_low_mv = 200` | No high-side fault. A lost transducer ground pulls SIG towards +5 V, which reads ≈ 3.3 V = **≈ 19.8 bar**, shown as a real pressure. | RISK |

## 5. USB-C J4

| Claim | Where made | Datasheet value + section | Verdict |
|---|---|---|---|
| GCT USB4105-GF-A on KiCad `USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal` | controller.py, jlc bom (C3020560) | The footprint's tags list USB4105-GF-A, and its pads are A1, A4–A9, A12, B1, B4–B9, B12, S1 (×4). [GCT] pinout: A1/B12/A12/B1 GND; A4/A9/B4/B9 VBUS; A5 CC1; B5 CC2; A6 Dp1; A7 Dn1; B6 Dp2; B7 Dn2; A8/B8 SBU (left open). The template's pin names match. | OK |
| Rating | — | [GCT]: 5 A on VBUS, 48 V DC, −40 to +85 °C | OK |
| CC1 and CC2 each with their own 5.1k to GND (sink) | controller.py R13, R14 | USB Type-C spec: Rd = 5.1 kΩ ±20 %, one per CC pin. They must not be shared, and they aren't. | OK |
| D+ to both A6/B6, D− to both A7/B7, to GPIO13/GPIO12 | controller.py | [MOD] Table 3-1: IO12 = USB_D−, IO13 = USB_D+. Correct polarity, so the swap bit isn't needed ([CHIP] Table 2-10 note 1). | OK |
| Series parts on D± | (none) | [HDG]: "reserve series resistors (initial value can be 22/33 Ω) and capacitors to ground", with the capacitors unpopulated, near the chip. None on the board. | RISK (minor) |
| ESD | (none) | [MOD] §12.2: module HBM ±2 kV only. There's no TVS on VBUS/D±. The port is service-only and lives inside the machine. | RISK (minor) |
| VBUS → SS14 → +5 V (bench power, no back-feed) | controller.py, CIRCUIT.md | Topology is correct. USB 2.0 limits a device's VBUS bulk capacitance to 10 µF; behind the diode sit C6 (22 µF 0805 X7R, ≈ 10 µF effective at 5 V) plus the PSU's output capacitance. A mild inrush overrun, bench-only. | RISK (minor) |
| Shield tied straight to GND | controller.py | Common practice; Espressif gives no guidance | OK |

## 6. JST PH J5–J7, switches

| Claim | Where made | Datasheet value + section | Verdict |
|---|---|---|---|
| B2B-PH-K / B3B-PH-K, rated 85 °C | controller.py, HARDWARE.md | [JST]: 2 A (AWG 24), 100 V, −25 to +85 °C *including* temperature rise. Our current is mA-level, so 85 °C against a 60–70 °C case is fine. | OK |
| Retention | — | PH is a friction-fit "disconnectable crimp" connector with no positive latch ([JST]), inside a machine with a vibratory pump | RISK |
| J6 (1 = +5V, 2 = SIG, 3 = GND) and J7 (1 = BREW, 2 = STEAM, 3 = GND) are the same 3-pin PH | controller.py | Plug the switch harness into J6 by mistake and the brew switch (pin 1 – pin 3) **shorts +5 V to GND** whenever it closes. The reverse mistake is harmless. | RISK |
| Switches: dry contact to GND on internal pull-ups | controller.py, firmware `Pull::Up` | [CHIP] Table 5-4: R_PU ≈ 45 kΩ, so ~70 µA of wetting current. The stock GCP switches are mains-rated (silver) contacts, and at ~70 µA dry-circuit reliability isn't guaranteed (switch spec UNVERIFIED). The harness also runs beside mains wiring with no series R / RC / TVS. Firmware debounce (3 cycles) handles bounce, not ESD. | RISK |
| Unused IOs left floating, no pull enabled | controller.py (IO2/3/4/5/14/15/21/22/RXD0/TXD0 NC); firmware never touches them | [HDG]: "it is recommended to add a pull-up or pull-down resistor or enable the internal pull during software initialization" | RISK (minor) |

## 7. Decoupling and placement

| Claim | Where made | Datasheet value + section | Verdict |
|---|---|---|---|
| C3 10 µF + C4 100 nF "at the 3V3 pin" | controller.py notes | [MOD] Fig. 9-1 shows 22 µF + 0.1 µF at 3V3; [HDG] asks for ≥ 10 µF at the entrance plus 0.1 µF close to the pins. The values are fine (C7 22 µF sits at the LDO). **Placement:** pad 3 (3V3) is at (59.4, 33.0) mm on top; C3 is at (57.0, 15.4) and C4 at (58.8, 15.6) on the **bottom**, about 17.5 mm away. That isn't "at the pin". | RISK |

## 8. Firmware vs controller.py

| Claim | Where made | Value | Verdict |
|---|---|---|---|
| Pins: GPIO0 pressure, GPIO1 NTC, GPIO18 valve, GPIO19 pump, GPIO20 heater, GPIO6 brew, GPIO7 steam, GPIO23 LED | main.rs `control::Pins`, control.rs | Matches FIRMWARE_PINS and the routed PCB | OK |
| main.rs module-doc pin table: "Brew switch 21, Steam switch 22" | firmware/esp32/src/main.rs header | The code uses GPIO6/GPIO7, as do controller.py and the docs. The doc table is stale. | **WRONG (doc)** |
| Outputs driven low first, push-pull | control.rs, bridge.rs | `Output::new(.., Level::Low, ..)` before anything else | OK |
| Switches with internal pull-up, active low | control.rs | `InputConfig::default().with_pull(Pull::Up)`, `is_low()` = closed | OK |
| USB Serial/JTAG on GPIO12/13 | usb.rs | `UsbSerialJtag::new(USB_DEVICE)`. The pins are never reconfigured as GPIO. | OK |
| ADC conversion speed "~50 µs" | control.rs comment | [CHIP] Table 5-5: up to 100 kSPS, i.e. ≥ 10 µs. The 50 µs per one-shot read including esp-hal overhead is plausible. | UNVERIFIED |
| Oversample clamp at 3300 mV | coffee-board `Oversample::take` | Consistent with ATTEN3's 0–3300 mV | OK |

---

## Must fix

1. **The antenna connector isn't U.FL.** [MOD] §10.2: ESP32-C6-MINI-1U uses the third-generation
   connector, compatible with W.FL / **I-PEX MHF III** / AMMC. A U.FL / MHF1 pigtail (what
   INSTALL.md tells people to buy) won't mate.
   - Change "U.FL" to "MHF III (W.FL)" in controller.py (ESP32C6 description and U4 notes),
     README.md, docs/CIRCUIT.md, docs/HARDWARE.md (off-board BOM), docs/INSTALL.md (§0 table and
     §5 "U4 antenna"), lib/README.md and audit.py.
   - Specify an MHF III pigtail of 100–150 mm, or an MHF III → U.FL/SMA adapter cable, with
     antenna gain ≤ 2.33 dBi to stay inside the module's certification ([MOD] §10.2).
2. **The NTC fault thresholds are inside the ADC's error band.** Fix in
   `crates/coffee-core/src/config.rs` `NtcConfig`. Nominal node voltage is 3.199 V at 5 °C and
   3.221 V at 0 °C, against `open_mv = 3250` and ATTEN3 ±40 mV ([CHIP] Table 5-6) plus the 3V3
   tolerance. A cold kitchen can read "open", and an open probe on a slightly low rail reads
   ≈ 3 °C ("cold, heat harder"). Pick one:
   - **(a) Hardware (preferred):** route +3V3 through a 2 × 10k divider (0.1 %) to spare ADC1 pin
     **GPIO2 (ADC1_CH2, pad 5)** and +5V through 10k/20k to **GPIO3 (ADC1_CH3, pad 6)**, each
     with 100 nF. The firmware then works in ratios: the NTC against the measured 3V3, the
     pressure against the measured 5V. That fixes this item and the two ratiometric RISKs in
     §4.
   - **(b) Firmware only:** set the open threshold at ≥ 3290 mV, i.e. near the ADC ceiling; lower
     the operating floor (e.g. accept down to −10 °C ≈ 3.25 V); and add a plausibility check
     that the temperature has to rise while the heater is on.

   Either way, also add a pressure **high** fault (e.g. > 3150 mV at the pin = transducer above
   4.72 V or its ground open).
3. **Single-reflow rule for the module.** [MOD] §12.3 says "Solder the module in a single
   reflow". Since v10 the board is two-sided, with U4 on top and passives plus U5/D2 on the
   bottom. Either:
   - add an order note to the JLC instructions / `jlc.py` report ("reflow the bottom side first;
     U4 / top side last"); or
   - confirm JLC's two-sided process puts the module through one reflow; or
   - move the bottom-side parts to the top.
4. **Doc error in firmware/esp32/src/main.rs.** The pin-map table says brew = GPIO21 and
   steam = GPIO22; the code and the board use GPIO6 and GPIO7. Correct the table. Doc only, but
   it's the firmware's pin reference.

## Should consider

1. **Decoupling placement.** Move C4 (100 nF), and ideally C3, to within ~2 mm of U4 pad 3 (+3V3)
   on the top side. They sit ~17.5 mm away on the bottom today ([MOD] Fig. 9-1, [HDG]).
   Consider making C3 22 µF (the same Murata GRM21BZ71A226ME15L as C6/C7: one fewer BOM line,
   and it matches Fig. 9-1).
2. **The firmware and the simulation assume different drive strengths.** The firmware uses
   esp-hal's default `DriveStrength::_20mA`; `sim/run.py` models PAD_DRIVER = 3 (~25 Ω). Either
   set `OutputConfig::default().with_drive_strength(DriveStrength::_40mA)` on GPIO18/19/20
   (this matches the datasheet's IOH test condition, [CHIP] Table 5-4) or re-simulate at ~33 Ω.
   The margin is fine either way (≥ 7.9 mA against IFT 5 mA). (`sim/drive.cir` is a stale
   MOC3063/AO3400 netlist.)
3. **Status LED is too dim.** D1 (KT-0603G, emerald InGaN, Vf 3.1 V) with R12 1k from 3.3 V gives
   only ~0.2–0.6 mA. Either drop R12 to 100–120 Ω (~1.5–2 mA), or pick a ~2.0 V yellow-green /
   AlGaInP 0603 LED and keep ~680 Ω.
4. **GPIO9 pull-up.** Add an external 10k pull-up on IO9_BOOT, as [HDG] recommends. Today only
   the internal 45 kΩ holds SPI boot on a net with a bare test pad, near mains wiring. Keep any
   capacitance off it.
5. **USB D± series resistors.** Add footprints for 22–33 Ω on D± ([HDG]), and optionally a
   low-capacitance ESD array (e.g. USBLC6-2SC6) at J4.
6. **USB inrush.** C6 (22 µF) plus the PSU output capacitance behind D2 exceeds USB 2.0's 10 µF.
   Acceptable for bench use, but don't rely on picky hosts.
7. **Off-board inputs (J5–J7).**
   - Add 1 kΩ series resistors plus 10 nF at GPIO6/GPIO7 (or a TVS array). The harness runs
     beside mains, and the module is HBM ±2 kV only ([MOD] §12.2).
   - Add external 4.7k pull-ups for more wetting current on the reused mains-rated switch
     contacts (internal R_PU is 45 kΩ, ~70 µA).
   - Consider a PTC or 10–22 Ω on J6's +5 V so a harness short near the boiler can't drag down
     the whole rail.
8. **Connector keying.** J6 and J7 are both 3-pin PH, and the switch harness plugged into J6
   shorts +5 V to GND through the brew switch. Use different pin counts (e.g. J7 as 4-pin PH:
   BREW, GND, STEAM, GND), or a different family for J6. Also consider a latching series (JST PA
   / GH / Molex Pico-Lock) because of pump vibration; PH has no positive lock.
9. **Unused IOs.** Enable internal pull-downs on IO2/3/4/5/14/15/21/22 in firmware, or tie them
   off ([HDG]). Leave GPIO15 alone if a pull-up is ever needed for pad JTAG.
10. **NTC model.** Use the probe vendor's R–T table to set Steinhart–Hart coefficients (the
    firmware already supports them). The β = 3950 model from B25/50 drifts by a few °C at steam
    temperatures, and the two-point calibration (room + 100 °C) only pins down the brew range.
    Choose and document a specific probe MPN; today it's "generic 100k B3950 M4".
11. **Pressure transducer spec.** Choose an MPN and check:
    - supply range: the USB bench supply gives 4.52 V;
    - minimum load resistance: the divider is 30 kΩ;
    - output clamp behaviour above full scale.
12. **Comment accuracy.** controller.py calls GPIO6/7 "plain GPIOs"; they are MTCK/MTDO (JTAG),
    free only while JTAG stays routed to USB ([MOD] Table 4-7). Never burn
    EFUSE_JTAG_SEL_ENABLE / DIS_USB_JTAG on this board, or GPIO15 becomes a live strap and the
    switches sit on JTAG pads.

## Unverified

- Output resistance at PAD_DRIVER = 2: the datasheet only specifies PAD_DRIVER = 3.
- The NTC probe and pressure transducer: no MPNs, so their datasheets can't be checked.
- Stock GCP switch contact ratings for dry-circuit (µA) use.
- Whether JLC's two-sided process reflows the module once.
- ADC conversion time per one-shot read in esp-hal.
- TLV1117LV33 output tolerance; the ±1.5 % used above is a typical value. It belongs to the
  power-supply audit.
