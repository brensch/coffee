#!/usr/bin/env python3
"""Write docs/PARTS.md: every part on the board, its datasheet, the ratings we rely on, and what the
datasheet audit (hardware/controller/datasheets/audit-*.md) found. Generated from build/bom-120v.csv so
it can't drift from the schematic.

    .venv/bin/python controller.py && python3 parts_record.py
"""
import csv, os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "..", "docs", "PARTS.md")

# datasheet, the ratings the design relies on, audit verdict (keyed by first ref of the BOM line)
INFO = {
    "PS1": ("https://recom-power.com/pdf/Powerline_AC-DC/RAC03E-K_277.pdf",
            "3 W, 4 kVAC reinforced, EN 60335-1 / IEC 61558 / UL 62368, internal fuse, -40..85 C (1.8 W at 85 C)",
            "Footprint was wrong (pins at 0/2.54/25.4 mm); fixed to the datasheet: 0/5.08/30.48 mm, outputs at 25.4/30.48 mm."),
    "Q1": ("https://www.st.com/resource/en/datasheet/bta24.pdf",
           "800 V, 25 A, insulated tab, snubberless; Rth(j-c) 1.7 C/W; ~10.8 W at the heater current",
           "Pinout and thermals OK."),
    "Q2": ("https://www.st.com/resource/en/datasheet/bta08.pdf",
           "800 V, 8 A, insulated tab, snubberless (BW: 50 mA gate)",
           "Was BTA08-800CW, which ST only makes at 600 V in the insulated package (JLC matched a clone). Changed to BTA08-800BWRG."),
    "Q3": ("https://www.st.com/resource/en/datasheet/z01.pdf",
           "800 V, 1 A, IGT 10 mA; (dV/dt)c ~0.37-0.44 A/ms allowed at our 10-16 V/us (Fig. 10)",
           "Kept: the valve's dI/dt is ~0.05-0.08 A/ms, about 5x inside the limit. Snubberless alternatives raise the turn-off spike."),
    "U1": ("https://www.onsemi.com/pdf/datasheet/moc3083m-d.pdf",
           "800 V zero-cross opto-triac, IFT 5 mA max, inhibit 20 V max, 4170 VAC RMS isolation (1 min)",
           "Order the through-hole MOC3083TVM (0.4\" leads); the BOM had matched the SMD SR2M. Isolation figure in the docs corrected from 5.3 kV."),
    "C1": ("https://www.murata.com/en-us/products/productdetail?partno=GA355QR7GB103KW01%23",
           "X2, 250 VAC, 10 nF (~8 nF derated), 2220",
           "OK. A cracked ceramic fails short and burns its 100 R: a fusible resistor would contain that (not done)."),
    "R3": ("", "100 R, 1 W (2512), snubber series resistor", "OK."),
    "R1": ("", "360 R (1206): opto feed to the triac gate", "OK: the gate reaches IGT at ~21 V line (co-simulation)."),
    "R5": ("", "330 R gate-MT1 for the sensitive-gate Z0109", "OK."),
    "F1": ("https://www.littelfuse.com/~/media/electronics/datasheets/fuses/littelfuse_fuse_372_datasheet.pdf.pdf",
           "T500mA slow, 250 VAC, TR5; I2t 1.3 A2s", "OK. The MOV that sat after it was removed (fire risk behind a 500 mA fuse)."),
    "J1": ("https://www.phoenixcontact.com/en-us/products/pcb-terminal-block-mkds-15-2-508-1715721", "MKDS 1,5/2-5.08", "OK. J1 is now 1 = N, 2 = L (silk-labelled), which keeps the live trunk from crossing the neutral."),
    "J2": ("https://www.phoenixcontact.com/en-us/products/pcb-terminal-block-mkds-3-2-508-1711725",
           "MKDS 3/2-5.08: 24 A IEC, UL 15 A; heater 10 A at 120 V",
           "Part number corrected (1711026 is the 5.00 mm pitch part). UL rating covers the 120 V heater with margin; the 230 V build's 11.4 A too."),
    "J3": ("https://www.phoenixcontact.com/en-us/products/pcb-terminal-block-mkds-15-2-508-1715721", "MKDS 1,5/2-5.08", "OK."),
    "U4": ("https://www.espressif.com/sites/default/files/documentation/esp32-c6-mini-1_mini-1u_datasheet_en.pdf",
           "ESP32-C6, 4 MB flash, -40..105 C (H4), external antenna on **W.FL / MHF III** (not U.FL), antenna <= 2.33 dBi",
           "Pads, straps and EN RC match. Antenna connector was mis-named U.FL everywhere: fixed. One reflow only: the module's side must be reflowed second."),
    "U5": ("https://www.ti.com/lit/ds/symlink/tlv1117lv.pdf",
           "3.3 V 1 A LDO, stable with ceramics (> 0.5 uF), dropout 455 mV typ / 700 mV max",
           "Pinout OK. Gets a +3V3 copper area as its heatsink (tab = OUT)."),
    "D2": ("https://www.vishay.com/docs/88746/ss12.pdf", "40 V 1 A Schottky: USB VBUS -> +5V for bench power", "OK."),
    "J4": ("https://gct.co/files/drawings/usb4105.pdf", "USB-C 2.0 receptacle; CC1/CC2 5.1k pull-downs (sink)", "OK."),
    "J5": ("https://www.jst-mfg.com/product/pdf/eng/ePH.pdf", "JST PH 2 mm, 2 A", "OK. PH has no latch: tie the harness down near the pump."),
    "J6": ("https://www.jst-mfg.com/product/pdf/eng/ePH.pdf", "JST PH 3-pin: +5V, SIG, GND", "J6/J7 are both 3-pin; see J7."),
    "J7": ("https://www.jst-mfg.com/product/pdf/eng/ePH.pdf", "JST PH 3-pin: BREW, STEAM, GND",
           "Mis-plug made harmless: the switch inputs have 1k series resistors (a transducer plugged in here can't hurt the GPIOs); a switch harness in J6 can at worst short +5V, which the RECOM survives (hiccup)."),
    "C6": ("https://www.murata.com/en-us/products/productdetail?partno=GRM21BZ71A226ME15%23", "22 uF 10 V X7R 0805 (~7-10 uF at 5 V DC bias)", "OK for the LDO and module."),
    "D1": ("https://www.lcsc.com/product-detail/C12624.html", "green 0603 LED, Vf ~2.9-3.1 V", "R12 changed 1k -> 100 R: at 1k it got only 0.2-0.6 mA."),
    "R15": ("", "8.2k 0.1 %: thermistor reference to 3V3", "OK; now read ratiometrically against the measured 3V3 (R22/R23)."),
    "R16": ("", "0.1 % dividers: pressure (R16/R17) and rail sense (R22-R25)", "New: rail sense lets the firmware cancel rail and ADC gain error."),
    "R18": ("", "1k series on the switch inputs", "New (audit)."),
    "R20": ("", "4.7k switch pull-ups: ~0.7 mA wetting current", "New (audit): the internal 45k gave ~70 uA, too little for old contacts."),
}


def main():
    rows = list(csv.DictReader(open(os.path.join(HERE, "build", "bom-120v.csv"))))
    out = ["# Parts record", "",
           "Generated by `hardware/controller/parts_record.py` from the schematic's BOM. Datasheet checks: "
           "[mains](../hardware/controller/datasheets/audit-mains.md), [power](../hardware/controller/datasheets/audit-power.md), "
           "[MCU and low voltage](../hardware/controller/datasheets/audit-mcu.md).", "",
           "| Refs | Qty | Value | Part / MPN | Footprint | Relied-on ratings | Datasheet | Audit |",
           "|---|---|---|---|---|---|---|---|"]
    for r in rows:
        refs = r["Refs"].split()
        info = next((INFO[k] for k in refs if k in INFO), ("", r["Description"], ""))
        ds = f"[pdf]({info[0]})" if info[0] else ""
        mpn = r["Part / MPN"].replace("|", "/")
        out.append(f"| {r['Refs']} | {r['Qty']} | {r['Value']} | {mpn} | `{r['Footprint'].split(':')[-1]}` | {info[1]} | {ds} | {info[2]} |")
    out += ["", "## Removed", "",
            "- **RV1 MOV (10D271K).** Behind the T500mA fuse a failing MOV can dissipate 60-110 W before the fuse opens; "
            "a real surge would blow the fuse anyway; the RECOM passes its 1 kV surge test unaided and every triac and "
            "opto-triac is an 800 V part. (Its footprint was also wrong: the 10D271K is a 12.5 mm disc on 7.5 mm pitch.)", "",
            "## Assembly notes", "",
            "- **Reflow order.** The ESP32-C6 module tolerates one reflow (datasheet section 12.3). Reflow the side without the "
            "module first; the module's side goes second. Through-hole parts are soldered last.",
            "- **Copper.** 1 oz is enough: the heater current runs on both layers (2-3 mm each) for ~15 mm.",
            ""]
    open(OUT, "w").write("\n".join(out))
    print(f"wrote {OUT}: {len(rows)} BOM lines")


if __name__ == "__main__":
    main()
