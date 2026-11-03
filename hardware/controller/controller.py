#!/usr/bin/env python3
"""coffee controller board v2: one board that drives the machine (schematic as code, SKiDL).

Everything on one board: mains switching (three zero-cross triacs), the 5 V supply,
zero-cross sensing, thermocouple + pressure inputs, ESP32-S3. The mains half and the
SELV half meet only across optocouplers and the certified AC-DC module, over a
milled isolation slot. Design notes and calculations: docs/CIRCUIT.md.

    python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
    .venv/bin/python controller.py                 # 230 V: ERC + design checks -> build/
    MAINS_V=120 .venv/bin/python controller.py     # 120 V variant

Outputs in build/: KiCad netlist, BOM CSV, and parts.json (placement input for
layout.py, which builds and routes the PCB).

Pin numbers come from the manufacturers' datasheets (ESP32-S3-WROOM-1 v1.3
table 3-1, MAX31855, ADS1115, H11AA1, MOC3063, HLK-5M05, AMS1117, USBLC6-2SC6,
AO3400A, BTA16 / Z0109 (pin 1 = MT1, 2 = MT2, 3 = G for TO-220; SOT-223: 1 = MT1,
2/tab = MT2, 3 = G)).
"""

import csv
import json
import os
import sys

from skidl import ERC, Net, Part, Pin, SKIDL, TEMPLATE, generate_netlist, KICAD8

import builtins

NC = builtins.NC  # SKiDL's no-connect net (installed into builtins by skidl)

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "build")
os.makedirs(OUT, exist_ok=True)

# Mains-side values depend on the supply. 230 V is the default.
MAINS_V = int(os.environ.get("MAINS_V", "230"))

T = Pin.types
BOM_EXTRA = {}  # ref -> dict(mpn=..., notes=..., populate=...)
MAINS_PINS = set()  # (ref, pin) on the mains side of the isolation barrier


def tmpl(name, prefix, footprint, pins, desc=""):
    """A part template defined entirely in code (no KiCad library needed)."""
    return Part(
        name=name,
        tool=SKIDL,
        dest=TEMPLATE,
        ref_prefix=prefix,
        footprint=footprint,
        description=desc,
        pins=[Pin(num=str(n), name=nm, func=f) for (n, nm, f) in pins],
    )


def part(template, ref, value="", mpn="", notes="", populate="Y", footprint=None, mains=None):
    """Instantiate. `mains`: True = every pin is mains side, or a list of mains-side pin numbers."""
    p = template(ref=ref, value=value, tag=ref)  # stable tag: KiCad keeps part identity across regenerations
    if footprint:
        p.footprint = footprint
    BOM_EXTRA[p.ref] = dict(mpn=mpn, notes=notes, populate=populate)
    if mains is True:
        MAINS_PINS.update((ref, pin.num) for pin in p.pins)
    elif mains:
        MAINS_PINS.update((ref, str(n)) for n in mains)
    return p


# --------------------------------------------------------------- templates
FP_R = "Resistor_SMD:R_0603_1608Metric"
FP_R_1206 = "Resistor_SMD:R_1206_3216Metric"
FP_R_MAINS = "Resistor_SMD:R_2512_6332Metric"
FP_C = "Capacitor_SMD:C_0603_1608Metric"
FP_C_BIG = "Capacitor_SMD:C_1206_3216Metric"

R = tmpl("R", "R", FP_R, [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Resistor")
C = tmpl("C", "C", FP_C, [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Capacitor")
CX2 = tmpl("C_X2", "C", "Capacitor_THT:C_Rect_L10.0mm_W4.0mm_P7.50mm_MKS4",
           [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "X2 safety capacitor 275 VAC")
CP = tmpl("CP", "C", "Capacitor_SMD:CP_Elec_6.3x7.7", [(1, "+", T.PASSIVE), (2, "-", T.PASSIVE)], "Electrolytic")
LED = tmpl("LED", "D", "LED_SMD:LED_0603_1608Metric", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "LED")
TVS = tmpl("PESD5V0S1BA", "D", "Diode_SMD:D_SOD-323", [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "ESD/TVS 5 V bidirectional")
SCHOTTKY = tmpl("SS14", "D", "Diode_SMD:D_SMA", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "Schottky 1 A 40 V")
FUSE_TR5 = tmpl("Fuse_TR5", "F", "Fuse:Fuse_Littelfuse_372_D8.50mm", [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Fuse, TR5 radial")
MOV = tmpl("Varistor", "RV", "Varistor:RV_Disc_D7mm_W4.2mm_P5mm", [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Varistor")
BUTTON = tmpl("SW_Push", "SW", "Button_Switch_SMD:SW_SPST_TL3342", [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Tactile switch")
HOLE = tmpl("MountingHole", "H", "MountingHole:MountingHole_3.2mm_M3", [], "M3 mounting hole (unplated, isolated)")


def conn(n, name, fp):
    return tmpl(name, "J", fp, [(i, str(i), T.PASSIVE) for i in range(1, n + 1)], name)


TB2_MAINS = conn(2, "Screw_Terminal_01x02_5.08", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MKDS-1,5-2-5.08_1x02_P5.08mm_Horizontal")
TB2 = conn(2, "Screw_Terminal_01x02_2.54", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MPT-0,5-2-2.54_1x02_P2.54mm_Horizontal")
JST2 = conn(2, "JST_PH_2", "Connector_JST:JST_PH_B2B-PH-K_1x02_P2.00mm_Vertical")
JST3 = conn(3, "JST_PH_3", "Connector_JST:JST_PH_B3B-PH-K_1x03_P2.00mm_Vertical")
JST4 = conn(4, "JST_PH_4", "Connector_JST:JST_PH_B4B-PH-K_1x04_P2.00mm_Vertical")

AMS1117 = tmpl("AMS1117-3.3", "U", "Package_TO_SOT_SMD:SOT-223-3_TabPin2",
               [(1, "GND", T.PWRIN), (2, "VOUT", T.PWROUT), (3, "VIN", T.PWRIN)], "LDO 3.3 V 1 A")
HLK5M05 = tmpl("HLK-5M05", "PS", "Converter_ACDC:Converter_ACDC_Hi-Link_HLK-5Mxx",
               [(1, "AC_L", T.PWRIN), (2, "AC_N", T.PWRIN), (3, "-VO", T.PWROUT), (4, "+VO", T.PWROUT)],
               "Isolated AC-DC 5 V 1 A (UL/CE, 3 kVAC isolation)")

# ESP32-S3-WROOM-1 (datasheet v1.3, table 3-1). Pad 41 = exposed GND pad.
_s3 = ["GND", "3V3", "EN", "IO4", "IO5", "IO6", "IO7", "IO15", "IO16", "IO17", "IO18", "IO8", "IO19", "IO20",
       "IO3", "IO46", "IO9", "IO10", "IO11", "IO12", "IO13", "IO14", "IO21", "IO47", "IO48", "IO45", "IO0",
       "IO35", "IO36", "IO37", "IO38", "IO39", "IO40", "IO41", "IO42", "RXD0", "TXD0", "IO2", "IO1", "GND2", "EPAD"]
ESP32S3 = tmpl("ESP32-S3-WROOM-1", "U", "RF_Module:ESP32-S3-WROOM-1",
               [(i + 1, n, T.PWRIN if n.startswith(("GND", "3V3", "EPAD")) else T.BIDIR) for i, n in enumerate(_s3)],
               "Wi-Fi / BLE 5 module, dual core")

MAX31855 = tmpl("MAX31855K", "U", "Package_SO:SOIC-8_3.9x4.9mm_P1.27mm",
                [(1, "GND", T.PWRIN), (2, "T-", T.INPUT), (3, "T+", T.INPUT), (4, "VCC", T.PWRIN),
                 (5, "SCK", T.INPUT), (6, "CS", T.INPUT), (7, "SO", T.OUTPUT), (8, "DNC", T.NOCONNECT)],
                "K-type thermocouple to digital")
ADS1115 = tmpl("ADS1115", "U", "Package_SO:MSOP-10_3x3mm_P0.5mm",
               [(1, "ADDR", T.INPUT), (2, "ALERT", T.OPENCOLL), (3, "GND", T.PWRIN), (4, "AIN0", T.INPUT),
                (5, "AIN1", T.INPUT), (6, "AIN2", T.INPUT), (7, "AIN3", T.INPUT), (8, "VDD", T.PWRIN),
                (9, "SDA", T.BIDIR), (10, "SCL", T.INPUT)], "16-bit 4-ch ADC, I2C")
# Wide-lead (10.16 mm / 0.4") DIP-6 optos: ~8 mm creepage across the slot.
H11AA1 = tmpl("H11AA1", "U", "Package_DIP:DIP-6_W10.16mm",
              [(1, "AC1", T.PASSIVE), (2, "AC2", T.PASSIVE), (3, "NC", T.NOCONNECT),
               (4, "E", T.PASSIVE), (5, "C", T.OPENCOLL), (6, "B", T.NOCONNECT)], "AC-input optocoupler (zero cross)")
MOC3063 = tmpl("MOC3063", "U", "Package_DIP:DIP-6_W10.16mm",
               [(1, "A", T.PASSIVE), (2, "K", T.PASSIVE), (3, "NC", T.NOCONNECT),
                (4, "MT1", T.PASSIVE), (5, "NC2", T.NOCONNECT), (6, "MT2", T.PASSIVE)],
               "Zero-cross triac driver opto, 600 V, 5 mA")
TRIAC_TO220 = tmpl("Triac_TO220", "Q", "Package_TO_SOT_THT:TO-220-3_Vertical",
                   [(1, "MT1", T.PASSIVE), (2, "MT2", T.PASSIVE), (3, "G", T.PASSIVE)], "Triac, insulated tab")
TRIAC_SOT223 = tmpl("Triac_SOT223", "Q", "Package_TO_SOT_SMD:SOT-223-3_TabPin2",
                    [(1, "MT1", T.PASSIVE), (2, "MT2", T.PASSIVE), (3, "G", T.PASSIVE)], "Triac, sensitive gate")
AO3400A = tmpl("AO3400A", "Q", "Package_TO_SOT_SMD:SOT-23",
               [(1, "G", T.INPUT), (2, "S", T.PASSIVE), (3, "D", T.PASSIVE)], "N-MOSFET 30 V logic-level")
USBLC6 = tmpl("USBLC6-2SC6", "U", "Package_TO_SOT_SMD:SOT-23-6",
              [(1, "IO1a", T.PASSIVE), (2, "GND", T.PASSIVE), (3, "IO2a", T.PASSIVE),
               (4, "IO2b", T.PASSIVE), (5, "VBUS", T.PASSIVE), (6, "IO1b", T.PASSIVE)], "USB ESD protection")
USBC = tmpl("USB_C_Receptacle_USB2.0", "J", "Connector_USB:USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal",
            [("A1", "GND_A1", T.PASSIVE), ("A4", "VBUS_A4", T.PASSIVE), ("A5", "CC1", T.PASSIVE),
             ("A6", "DP1", T.PASSIVE), ("A7", "DN1", T.PASSIVE), ("A9", "VBUS_A9", T.PASSIVE), ("A12", "GND_A12", T.PASSIVE),
             ("B1", "GND_B1", T.PASSIVE), ("B4", "VBUS_B4", T.PASSIVE), ("B5", "CC2", T.PASSIVE), ("B6", "DP2", T.PASSIVE),
             ("B7", "DN2", T.PASSIVE), ("B9", "VBUS_B9", T.PASSIVE), ("B12", "GND_B12", T.PASSIVE), ("S1", "SHIELD", T.PASSIVE)],
            "USB-C receptacle (USB 2.0): flashing only")

# --------------------------------------------------------------- nets
gnd, v5, v3, vbus = Net("GND"), Net("+5V"), Net("+3V3"), Net("VBUS")
for n in (gnd, v5, v3):
    n.drive = 100

# ================================================================ MAINS SIDE
# Connections to the machine (all 5.08 mm, 250 V / 17.5 A screw terminals; the install
# uses short pigtails with 6.3 mm Faston receptacles onto the stock harness).
l_in, n_in = Net("AC_L"), Net("AC_N")
l_in.drive = n_in.drive = 100  # the mains
j_ac = part(TB2_MAINS, "J1", "Mains in", mains=True,
            notes="MAINS. 1=L 2=N, from the machine's power switch (after its fuse). Feeds pump, valve, PSU, zero-cross only (~0.5 A).")
j_ac[1, 2] += l_in, n_in
htr_a, htr_b = Net("HTR_A"), Net("HTR_B")
j_htr = part(TB2_MAINS, "J2", "Heater", mains=True,
             notes="MAINS, HEATER CURRENT (6 A @ 230 V, 11.4 A @ 120 V). Wired IN PLACE OF the brew thermostat: "
                   "the stock steam thermostat and thermal fuse stay in series as hardware cut-outs.")
j_htr[1, 2] += htr_a, htr_b
pump_out, valve_out = Net("PUMP_L"), Net("VALVE_L")
j_load = part(TB2_MAINS, "J3", "Pump / valve", mains=True,
              notes="MAINS. 1=switched L to 3-way valve, 2=switched L to pump (the wires that came from the brew switch).")
j_load[1, 2] += valve_out, pump_out

# Board supply branch: fuse + MOV, then PSU and zero-cross.
l_f = Net("AC_L_FUSED")
l_f.drive = 100
f1 = part(FUSE_TR5, "F1", "T500mA", mpn="Littelfuse 37205000411 (TR5, 500 mA slow, 250 VAC)", mains=True,
          notes="MAINS. Protects PSU and zero-cross branch only")
f1[1, 2] += l_in, l_f
rv1 = part(MOV, "RV1", "10D561K" if MAINS_V >= 200 else "10D271K", mpn="Bourns MOV-10D561K / 10D271K", mains=True,
           notes="MAINS. Surge clamp across the PSU input")
rv1[1, 2] += l_f, n_in
ps1 = part(HLK5M05, "PS1", "HLK-5M05", mpn="Hi-Link HLK-5M05", mains=["1", "2"],
           notes="Certified isolated supply; straddles the slot (AC pins mains side, DC pins SELV side)")
ps1["AC_L"] += l_f
ps1["AC_N"] += n_in
ps1["+VO"] += Net("5V_PSU")
ps1["-VO"] += gnd

# Zero-cross: H11AA1, two series resistors (each sees half the voltage).
zc_a, zc_b = Net("ZC_A"), Net("ZC_B")
r_zc_val = "47k 1W" if MAINS_V >= 200 else "22k 1W"
r_zc1 = part(R, "R1", r_zc_val, footprint=FP_R_MAINS, mains=True, notes="MAINS: 2512, working voltage >= 200 V")
r_zc2 = part(R, "R2", r_zc_val, footprint=FP_R_MAINS, mains=True, notes="MAINS")
r_zc1[1, 2] += l_f, zc_a
r_zc2[1, 2] += n_in, zc_b
u_zc = part(H11AA1, "U1", "H11AA1", mpn="onsemi H11AA1TVM (0.4\" leads)", mains=["1", "2"],
            notes="Isolation barrier: pins 1-2 mains, 4-6 SELV")
u_zc["AC1"] += zc_a
u_zc["AC2"] += zc_b
u_zc["NC"] += NC
u_zc["B"] += NC


def triac_channel(name, idx, mt2_net, mt1_net, triac_tpl, triac_val, triac_mpn, snubber):
    """MOC3063 zero-cross opto + triac (onsemi AN-3004 'inductive load' circuit)."""
    opto, triac = f"U{2 + idx}", f"Q{1 + idx}"
    u = part(MOC3063, opto, "MOC3063", mpn="onsemi MOC3063TVM (0.4\" leads)", mains=["4", "5", "6"],
             notes=f"{name}: isolation barrier, pins 1-2 SELV, 4-6 mains. Fires only within ±20 V of a zero crossing")
    q = part(triac_tpl, triac, triac_val, mpn=triac_mpn, mains=True, notes=f"MAINS: {name}")
    gate, opto_mt2 = Net(f"{name}_GATE"), Net(f"{name}_OPTO")
    q["MT2"] += mt2_net
    q["MT1"] += mt1_net
    q["G"] += gate
    r_s = part(R, f"R{3 + 3 * idx}", "360", footprint=FP_R_1206, mains=True, notes="MAINS: limits opto surge current (1 A peak)")
    r_s[1, 2] += mt2_net, opto_mt2
    u["MT2"] += opto_mt2
    u["MT1"] += gate
    u["NC2"] += NC
    r_g = part(R, f"R{4 + 3 * idx}", "330", footprint=FP_R_1206, mains=True, notes="MAINS: gate-MT1, stops false triggering")
    r_g[1, 2] += gate, mt1_net
    if snubber:
        s_mid = Net(f"{name}_SNUB")
        r_n = part(R, f"R{5 + 3 * idx}", "100 1W", footprint=FP_R_MAINS, mains=True, notes="MAINS: snubber (inductive load)")
        c_n = part(CX2, f"C{20 + idx}", "10nF X2", mpn="WIMA MKP-X2 10nF 275VAC, 10x4 mm, P7.5", mains=True, notes="MAINS: snubber")
        r_n[1, 2] += mt2_net, s_mid
        c_n[1, 2] += s_mid, mt1_net
    u["NC"] += NC
    return u


# Heater: triac across the brew-thermostat terminals (MT1 on J2.1 so the copper doesn't cross). Resistive: no snubber.
u_h = triac_channel("HEATER", 0, htr_b, htr_a, TRIAC_TO220, "BTA16-600BW", "ST BTA16-600BWRG (16 A, insulated tab, snubberless)", False)
# Pump (vibratory, diode inside: inductive) and valve (solenoid: inductive).
u_p = triac_channel("PUMP", 1, l_in, pump_out, TRIAC_TO220, "BTA08-600CW", "ST BTA08-600CWRG (8 A, insulated tab, snubberless)", True)
u_v = triac_channel("VALVE", 2, l_in, valve_out, TRIAC_SOT223, "Z0109MN", "ST Z0109MN5AA4 (1 A, IGT 10 mA)", True)

# ================================================================ SELV SIDE
# Power: PSU 5 V, or USB VBUS on the bench (OR-ed, no backfeed).
v5_psu = ps1["+VO"].net
d1 = part(SCHOTTKY, "D1", "SS14", mpn="SS14")
d1["A"] += v5_psu
d1["K"] += v5
d2 = part(SCHOTTKY, "D2", "SS14", mpn="SS14", notes="USB power for bench bring-up (no mains)")
d2["A"] += vbus
d2["K"] += v5
c1 = part(CP, "C1", "100uF 16V", mpn="105 C electrolytic, 100 uF 16 V")
c1["+"] += v5
c1["-"] += gnd
u_ldo = part(AMS1117, "U7", "AMS1117-3.3", mpn="AMS1117-3.3", notes="Tab pour is the heatsink")
u_ldo["VIN"] += v5
u_ldo["GND"] += gnd
u_ldo["VOUT"] += v3
for ref, val, net in (("C2", "10uF", v5), ("C3", "22uF 6.3V", v3)):
    c = part(C, ref, val, footprint=FP_C_BIG)
    c[1, 2] += net, gnd

# MCU
u_mcu = part(ESP32S3, "U8", "ESP32-S3-WROOM-1-N8", mpn="ESP32-S3-WROOM-1-N8")
for p in ("GND", "GND2", "EPAD"):
    u_mcu[p] += gnd
u_mcu["3V3"] += v3
for ref, val in (("C4", "10uF"), ("C5", "100nF")):
    c = part(C, ref, val, footprint=FP_C_BIG if "10uF" in val else FP_C)
    c[1, 2] += v3, gnd
en = Net("EN")
u_mcu["EN"] += en
r_en = part(R, "R20", "10k")
r_en[1, 2] += v3, en
c_en = part(C, "C6", "1uF", notes="EN RC delay (Espressif guideline)")
c_en[1, 2] += en, gnd
sw_rst = part(BUTTON, "SW1", "RESET")
sw_rst[1, 2] += en, gnd
boot = Net("IO0_BOOT")
u_mcu["IO0"] += boot
r_boot = part(R, "R21", "10k", notes="GPIO0 high at reset = normal boot")
r_boot[1, 2] += v3, boot
sw_boot = part(BUTTON, "SW2", "BOOT", notes="GPIO0 low at reset = download mode")
sw_boot[1, 2] += boot, gnd

# USB: flashing only (native USB, GPIO19/20).
j_usb = part(USBC, "J4", "USB-C", mpn="GCT USB4105-GF-A", notes="Flashing only; also powers the board for bench work")
for p in ("VBUS_A4", "VBUS_A9", "VBUS_B4", "VBUS_B9"):
    j_usb[p] += vbus
for p in ("GND_A1", "GND_A12", "GND_B1", "GND_B12", "SHIELD"):
    j_usb[p] += gnd
dp, dn = Net("USB_D+"), Net("USB_D-")
j_usb["DP1", "DP2"] += dp
j_usb["DN1", "DN2"] += dn
for ref, cc in (("R22", "CC1"), ("R23", "CC2")):
    r = part(R, ref, "5.1k", notes="UFP identification")
    ccn = Net(cc)
    j_usb[cc] += ccn
    r[1, 2] += ccn, gnd
u_esd = part(USBLC6, "U9", "USBLC6-2SC6", mpn="USBLC6-2SC6")
u_esd["IO1a", "IO1b"] += dn
u_esd["IO2a", "IO2b"] += dp
u_esd["VBUS"] += vbus
u_esd["GND"] += gnd
u_mcu["IO19"] += dn
u_mcu["IO20"] += dp

# Zero-cross output (SELV side of U1)
zc = Net("ZC")
u_zc["E"] += gnd
u_zc["C"] += zc
r_zcpu = part(R, "R24", "47k", notes="Output HIGH for ~0.9 ms centred on each crossing")
r_zcpu[1, 2] += v3, zc
c_zc = part(C, "C7", "100pF")
c_zc[1, 2] += zc, gnd
u_mcu["IO4"] += zc
u_mcu["IO4"].func = T.INPUT

# Triac drivers: GPIO -> AO3400 low side -> opto LED in series with an indicator LED.
# 5 V - 1.2 V (opto) - 2.0 V (green) = 1.8 V / 150 R = 12 mA (MOC3063 IFT 5 mA max).
for i, (name, gpio, u) in enumerate((("HEATER", "IO5", u_h), ("PUMP", "IO6", u_p), ("VALVE", "IO7", u_v))):
    cmd, gate, a, k = Net(f"{name}_CMD"), Net(f"{name}_G"), Net(f"{name}_LED_A"), Net(f"{name}_LED_K")
    r_led = part(R, f"R{25 + 3 * i}", "150")
    r_led[1, 2] += v5, a
    led = part(LED, f"D{3 + i}", f"{name.lower()} green", notes="Lit = triac driven (same current as the opto LED)")
    led["A"] += a
    led["K"] += k
    u["A"] += k
    q = part(AO3400A, f"Q{4 + i}", "AO3400A", mpn="AO3400A")
    u["K"] += Net(f"{name}_DRV")
    q["D"] += u["K"].net
    q["S"] += gnd
    q["G"] += gate
    rg = part(R, f"R{26 + 3 * i}", "100", notes="Gate series")
    rg[1, 2] += cmd, gate
    rpd = part(R, f"R{27 + 3 * i}", "100k", notes="Pull-down: load OFF while the ESP32 boots/resets/flashes")
    rpd[1, 2] += gate, gnd
    u_mcu[gpio] += cmd

# Thermocouple
u_tc = part(MAX31855, "U10", "MAX31855KASA+", mpn="MAX31855KASA+T", notes="Keep away from the LDO, PSU and triacs: its cold junction is the board")
u_tc["GND"] += gnd
u_tc["VCC"] += v3
u_tc["DNC"] += NC
c = part(C, "C8", "100nF")
c[1, 2] += v3, gnd
tcp, tcm = Net("TC+"), Net("TC-")
j_tc = part(TB2, "J5", "Thermocouple K", notes="UNGROUNDED-junction probe only. 1=T+ (yellow, ANSI) 2=T- (red)")
j_tc[1, 2] += tcp, tcm
u_tc["T+"] += tcp
u_tc["T-"] += tcm
c = part(C, "C9", "10nF", notes="Differential filter (datasheet)")
c[1, 2] += tcp, tcm
sck, miso, cs = Net("TC_SCK"), Net("TC_SO"), Net("TC_CS")
u_tc["SCK"] += sck
u_tc["SO"] += miso
u_tc["CS"] += cs
r = part(R, "R34", "10k", notes="CS deselected during boot")
r[1, 2] += v3, cs
u_mcu["IO12"] += sck
u_mcu["IO13"] += miso
u_mcu["IO10"] += cs

# ADC: pressure, 5 V monitor, group NTC, spare
u_adc = part(ADS1115, "U11", "ADS1115IDGSR", mpn="ADS1115IDGSR")
u_adc["VDD"] += v3
u_adc["GND"] += gnd
u_adc["ADDR"] += gnd  # 0x48
c = part(C, "C10", "100nF")
c[1, 2] += v3, gnd
sda, scl, rdy = Net("SDA"), Net("SCL"), Net("ADC_RDY")
u_adc["SDA"] += sda
u_adc["SCL"] += scl
u_adc["ALERT"] += rdy
for ref, net, val in (("R35", sda, "4.7k"), ("R36", scl, "4.7k"), ("R37", rdy, "10k")):
    r = part(R, ref, val)
    r[1, 2] += v3, net
u_mcu["IO8"] += sda
u_mcu["IO9"] += scl
u_mcu["IO11"] += rdy
u_mcu["IO11"].func = T.INPUT

p_sig, p_div = Net("P_SIG"), Net("P_AIN0")
j_p = part(JST3, "J6", "Pressure 0-1.6MPa", notes="1=+5V 2=SIG 3=GND")
j_p[1, 2, 3] += v5, p_sig, gnd
d = part(TVS, "D6", "PESD5V0S1BA", mpn="PESD5V0S1BA")
d[1, 2] += p_sig, gnd
r = part(R, "R38", "10k 0.1%", notes="Divider 2/3: 4.5 V -> 3.0 V")
r[1, 2] += p_sig, p_div
r = part(R, "R39", "20k 0.1%")
r[1, 2] += p_div, gnd
c = part(C, "C11", "100nF", notes="fc ~ 240 Hz: keeps the 50 Hz pump ripple, kills RF")
c[1, 2] += p_div, gnd
u_adc["AIN0"] += p_div

v5_div = Net("V5_AIN1")
r = part(R, "R40", "10k 0.1%")
r[1, 2] += v5, v5_div
r = part(R, "R41", "20k 0.1%")
r[1, 2] += v5_div, gnd
c = part(C, "C12", "100nF")
c[1, 2] += v5_div, gnd
u_adc["AIN1"] += v5_div

ntc = Net("NTC_AIN2")
j_ntc = part(JST2, "J7", "Group NTC (opt)", populate="opt", notes="100k B3950 ring-lug NTC under a group screw. 1=NTC 2=GND")
j_ntc[1, 2] += ntc, gnd
r = part(R, "R42", "10k 0.1%", populate="opt")
r[1, 2] += v3, ntc
c = part(C, "C13", "100nF", populate="opt")
c[1, 2] += ntc, gnd
u_adc["AIN2"] += ntc
u_adc["AIN3"] += Net("AIN3_SPARE")
r = part(R, "R43", "100k", notes="Spare AIN3 tied low (expansion pad)")
r[1, 2] += u_adc["AIN3"].net, gnd

# Front-panel switches as dry contacts (rewired off mains).
j_sw = part(JST3, "J8", "Switches", notes="1=BREW 2=STEAM 3=GND. Stock switches rewired as dry contacts")
for i, (name, gpio) in enumerate((("BREW", "IO15"), ("STEAM", "IO16"))):
    wire, pin = Net(f"SW_{name}"), Net(f"SW_{name}_IN")
    j_sw[i + 1] += wire
    r = part(R, f"R{44 + 2 * i}", "10k", notes="Pull-up (switch to GND = on)")
    r[1, 2] += v3, wire
    r = part(R, f"R{45 + 2 * i}", "1k", notes="Series: limits ESD current into the GPIO")
    r[1, 2] += wire, pin
    c = part(C, f"C{14 + i}", "100nF", notes="0.1 ms RC on long leads near mains")
    c[1, 2] += pin, gnd
    d = part(TVS, f"D{7 + i}", "PESD5V0S1BA", mpn="PESD5V0S1BA")
    d[1, 2] += wire, gnd
    u_mcu[gpio] += pin
j_sw[3] += gnd

# Expansion: I2C (display, scale bridge) on the same bus as the ADC.
j_i2c = part(JST4, "J9", "I2C expansion", populate="opt", notes="1=+3V3 2=SDA 3=SCL 4=GND (Qwiic order)")
j_i2c[1, 2, 3, 4] += v3, sda, scl, gnd

# Status LED
st = Net("STATUS")
u_mcu["IO38"] += st
r = part(R, "R48", "1k")
r[1, 2] += st, Net("STATUS_A")
led = part(LED, "D9", "status blue")
led["A"] += r[2].net
led["K"] += gnd

# Unused module pins
for p in ("IO1", "IO2", "IO3", "IO14", "IO17", "IO18", "IO21", "IO35", "IO36", "IO37", "IO39", "IO40",
          "IO41", "IO42", "IO45", "IO46", "IO47", "IO48", "RXD0", "TXD0"):
    u_mcu[p] += NC

# Mounting holes: unplated, so no copper bridges the barrier through the chassis.
for i in range(4):
    part(HOLE, f"H{i + 1}", "M3")


# ================================================================ checks + outputs
def nets():
    return builtins.default_circuit.nets


def design_checks():
    """Checks ERC can't express. Generation fails if any of these break."""
    errors = []
    # 1. Isolation: a net is either all-mains or all-SELV. Only the opto LEDs/outputs
    #    and the PSU cross, and they do so inside certified packages.
    for net in nets():
        if not net.pins or net is NC:
            continue
        sides = {(p.part.ref, p.num) in MAINS_PINS for p in net.pins}
        if len(sides) > 1:
            where = ", ".join(f"{p.part.ref}.{p.num}" for p in net.pins)
            errors.append(f"ISOLATION: net {net.name} crosses the barrier ({where})")
    # 2. Firmware pin map (firmware/esp32/src/main.rs, esp32s3 feature).
    fw = {"IO4": "ZC", "IO5": "HEATER_CMD", "IO6": "PUMP_CMD", "IO7": "VALVE_CMD", "IO15": "SW_BREW_IN",
          "IO16": "SW_STEAM_IN", "IO12": "TC_SCK", "IO13": "TC_SO", "IO10": "TC_CS", "IO8": "SDA", "IO9": "SCL",
          "IO19": "USB_D-", "IO20": "USB_D+"}
    for pin, want in fw.items():
        got = u_mcu[pin].net.name if u_mcu[pin].net else None
        if got != want:
            errors.append(f"PINMAP: {pin} is on {got}, firmware expects {want}")
    # 3. Every load is held off while the ESP32 boots (gate pull-down on each driver).
    for name in ("HEATER", "PUMP", "VALVE"):
        g = [n for n in nets() if n.name == f"{name}_G"][0]
        if not any(p.part.value == "100k" for p in g.pins):
            errors.append(f"BOOT-SAFE: {name} driver gate has no pull-down")
    # 4. S3 strapping pins: 0 has its pull-up + BOOT button, 3/45/46 must stay unconnected.
    for p in ("IO3", "IO45", "IO46"):
        if u_mcu[p].net is not NC and u_mcu[p].net is not None and u_mcu[p].net.pins:
            if len([x for x in u_mcu[p].net.pins if x.part is not u_mcu]) > 0:
                errors.append(f"STRAP: {p} must be left unconnected")
    if errors:
        print("\n".join(errors))
        sys.exit(1)
    print("design checks: isolation OK, firmware pin map OK, loads off at boot, straps free")


def write_outputs():
    circ = builtins.default_circuit
    net_path = os.path.join(OUT, f"coffee-controller-{MAINS_V}v.net")
    generate_netlist(file_=net_path, tool=KICAD8)

    # Placement/routing input for layout.py (pcbnew runs in a different Python).
    parts = {}
    for p in circ.parts:
        parts[p.ref] = dict(
            footprint=p.footprint,
            value=str(p.value),
            pins={pin.num: (pin.net.name if pin.net is not None and pin.net is not NC and pin.net.pins else None) for pin in p.pins},
            mains=sorted(n for (r, n) in MAINS_PINS if r == p.ref),
            populate=BOM_EXTRA.get(p.ref, {}).get("populate", "Y"),
        )
    mains_nets = sorted({n.name for n in nets() if n.pins and n is not NC and all((p.part.ref, p.num) in MAINS_PINS for p in n.pins)})
    with open(os.path.join(OUT, f"parts-{MAINS_V}v.json"), "w") as f:
        json.dump(dict(mains_v=MAINS_V, parts=parts, mains_nets=mains_nets,
                       heater_nets=["HTR_A", "HTR_B"]), f, indent=1)

    rows = {}
    for p in circ.parts:
        extra = BOM_EXTRA.get(p.ref, {})
        key = (p.name, str(p.value), p.footprint, extra.get("mpn", ""), extra.get("populate", "Y"))
        rows.setdefault(key, {"refs": [], "notes": set(), "desc": p.description})
        rows[key]["refs"].append(p.ref)
        if extra.get("notes"):
            rows[key]["notes"].add(extra["notes"])

    def refkey(r):
        import re

        m = re.match(r"([A-Z]+)(\d+)", r)
        return (m.group(1), int(m.group(2))) if m else (r, 0)

    bom_path = os.path.join(OUT, f"bom-{MAINS_V}v.csv")
    with open(bom_path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Refs", "Qty", "Value", "Part / MPN", "Footprint", "Populate", "Description", "Notes"])
        for (name, value, fp, mpn, pop), d in sorted(rows.items(), key=lambda kv: refkey(sorted(kv[1]["refs"], key=refkey)[0])):
            refs = sorted(d["refs"], key=refkey)
            w.writerow([" ".join(refs), len(refs), value, mpn or name, fp, pop, d["desc"], " | ".join(sorted(d["notes"]))])
    print(f"wrote {net_path}\nwrote {bom_path}\n{len(circ.parts)} parts, {len([n for n in nets() if n.pins])} nets, "
          f"{len(mains_nets)} mains nets")


if __name__ == "__main__":
    ERC()
    design_checks()
    write_outputs()
