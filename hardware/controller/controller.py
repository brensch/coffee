#!/usr/bin/env python3
"""coffee controller board: schematic as code (SKiDL).

Generates a KiCad netlist (import into KiCad PCB Editor) and a BOM CSV, after an
electrical rules check. Everything except the isolated zero-cross section is
SELV (5 V / 3.3 V). Design notes and calculations: docs/CIRCUIT.md.

    python3 -m venv .venv && .venv/bin/pip install skidl
    .venv/bin/python hardware/controller/controller.py

Pin numbers come from the manufacturers' datasheets (ESP32-C6-WROOM-1 v1.4
table 3-1, MAX31855, ADS1115, H11AA1, AP2112K, USBLC6-2SC6, AO3400A).
"""

import csv
import os
import sys

from skidl import (
    ERC,
    Net,
    Part,
    Pin,
    SKIDL,
    TEMPLATE,
    generate_netlist,
    lib_search_paths,
    set_default_tool,
    KICAD8,
)

import builtins

NC = builtins.NC  # SKiDL's no-connect net (installed into builtins by skidl)

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "build")
os.makedirs(OUT, exist_ok=True)

# Mains-side component values depend on the supply. 230 V is the default.
MAINS_V = int(os.environ.get("MAINS_V", "230"))

T = Pin.types
BOM_EXTRA = {}  # ref -> dict(mpn=..., notes=..., populate=...)


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


def part(template, ref, value="", mpn="", notes="", populate="Y", footprint=None):
    p = template(ref=ref, value=value, tag=ref)  # stable tag: KiCad keeps part identity across regenerations
    if footprint:
        p.footprint = footprint
    BOM_EXTRA[p.ref] = dict(mpn=mpn, notes=notes, populate=populate)
    return p


# --------------------------------------------------------------- templates
FP_R = "Resistor_SMD:R_0603_1608Metric"
FP_R_MAINS = "Resistor_SMD:R_2512_6332Metric"
FP_C = "Capacitor_SMD:C_0603_1608Metric"
FP_C_BIG = "Capacitor_SMD:C_1206_3216Metric"

R = tmpl("R", "R", FP_R, [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Resistor")
C = tmpl("C", "C", FP_C, [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Capacitor")
CP = tmpl("CP", "C", "Capacitor_SMD:CP_Elec_6.3x7.7", [(1, "+", T.PASSIVE), (2, "-", T.PASSIVE)], "Electrolytic")
LED = tmpl("LED", "D", "LED_SMD:LED_0603_1608Metric", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "LED")
SCHOTTKY = tmpl("SS14", "D", "Diode_SMD:D_SMA", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "Schottky 1 A 40 V")
TVS = tmpl("PESD5V0S1BA", "D", "Diode_SMD:D_SOD-323", [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "ESD/TVS 5 V bidirectional")
FUSE = tmpl("Fuse", "F", "Fuse:Fuse_1206_3216Metric", [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Fuse")
BUTTON = tmpl("SW_Push", "SW", "Button_Switch_SMD:SW_SPST_TL3342", [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Tactile switch")


def conn(n, name, fp):
    return tmpl(name, "J", fp, [(i, str(i), T.PASSIVE) for i in range(1, n + 1)], name)


TB2_MAINS = conn(2, "Screw_Terminal_01x02_5.08", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MKDS-1,5-2-5.08_1x02_P5.08mm_Horizontal")
TB2 = conn(2, "Screw_Terminal_01x02_3.81", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MPT-0,5-2-2.54_1x02_P2.54mm_Horizontal")
TB3 = conn(3, "Screw_Terminal_01x03_3.81", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MPT-0,5-3-2.54_1x03_P2.54mm_Horizontal")
JST3 = conn(3, "JST_PH_3", "Connector_JST:JST_PH_B3B-PH-K_1x03_P2.00mm_Vertical")
HDR4 = conn(4, "PinHeader_1x04", "Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical")

AMS1117 = tmpl(
    "AMS1117-3.3", "U", "Package_TO_SOT_SMD:SOT-223-3_TabPin2",
    [(1, "GND", T.PWRIN), (2, "VOUT", T.PWROUT), (3, "VIN", T.PWRIN)],
    "LDO 3.3 V 1 A (SOT-223 for heat: ~0.15 W average, 0.5 W at Wi-Fi peaks)",
)

_esp_pins = [
    (1, "GND", T.PWRIN), (2, "3V3", T.PWRIN), (3, "EN", T.INPUT),
    (4, "IO4", T.BIDIR), (5, "IO5", T.BIDIR), (6, "IO6", T.BIDIR), (7, "IO7", T.BIDIR),
    (8, "IO0", T.BIDIR), (9, "IO1", T.BIDIR), (10, "IO8", T.BIDIR), (11, "IO10", T.BIDIR),
    (12, "IO11", T.BIDIR), (13, "IO12", T.BIDIR), (14, "IO13", T.BIDIR), (15, "IO9", T.BIDIR),
    (16, "IO18", T.BIDIR), (17, "IO19", T.BIDIR), (18, "IO20", T.BIDIR), (19, "IO21", T.BIDIR),
    (20, "IO22", T.BIDIR), (21, "IO23", T.BIDIR), (22, "NC", T.NOCONNECT), (23, "IO15", T.BIDIR),
    (24, "RXD0", T.BIDIR), (25, "TXD0", T.BIDIR), (26, "IO3", T.BIDIR), (27, "IO2", T.BIDIR),
    (28, "GND", T.PWRIN), (29, "EPAD", T.PWRIN),
]
ESP32C6 = tmpl("ESP32-C6-WROOM-1", "U", "RF_Module:ESP32-C6-WROOM-1", _esp_pins, "Wi-Fi 6 / BLE 5 module")

MAX31855 = tmpl(
    "MAX31855K", "U", "Package_SO:SOIC-8_3.9x4.9mm_P1.27mm",
    [(1, "GND", T.PWRIN), (2, "T-", T.INPUT), (3, "T+", T.INPUT), (4, "VCC", T.PWRIN),
     (5, "SCK", T.INPUT), (6, "CS", T.INPUT), (7, "SO", T.OUTPUT), (8, "DNC", T.NOCONNECT)],
    "K-type thermocouple to digital",
)
ADS1115 = tmpl(
    "ADS1115", "U", "Package_SO:MSOP-10_3x3mm_P0.5mm",
    [(1, "ADDR", T.INPUT), (2, "ALERT", T.OPENCOLL), (3, "GND", T.PWRIN), (4, "AIN0", T.INPUT),
     (5, "AIN1", T.INPUT), (6, "AIN2", T.INPUT), (7, "AIN3", T.INPUT), (8, "VDD", T.PWRIN),
     (9, "SDA", T.BIDIR), (10, "SCL", T.INPUT)],
    "16-bit 4-ch ADC, I2C",
)
H11AA1 = tmpl(
    "H11AA1", "U", "Package_DIP:SMDIP-6_W9.53mm",
    [(1, "AC1", T.PASSIVE), (2, "AC2", T.PASSIVE), (3, "NC", T.NOCONNECT),
     (4, "E", T.PASSIVE), (5, "C", T.OPENCOLL), (6, "B", T.NOCONNECT)],
    "AC-input optocoupler (zero cross)",
)
AO3400A = tmpl(
    "AO3400A", "Q", "Package_TO_SOT_SMD:SOT-23",
    [(1, "G", T.INPUT), (2, "S", T.PASSIVE), (3, "D", T.PASSIVE)],
    "N-MOSFET 30 V logic-level",
)
USBLC6 = tmpl(
    "USBLC6-2SC6", "U", "Package_TO_SOT_SMD:SOT-23-6",
    [(1, "IO1a", T.PASSIVE), (2, "GND", T.PASSIVE), (3, "IO2a", T.PASSIVE),
     (4, "IO2b", T.PASSIVE), (5, "VBUS", T.PASSIVE), (6, "IO1b", T.PASSIVE)],
    "USB ESD protection",
)
USBC = tmpl(
    "USB_C_Receptacle_USB2.0", "J", "Connector_USB:USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal",
    [("A1", "GND_A1", T.PASSIVE), ("A4", "VBUS_A4", T.PASSIVE), ("A5", "CC1", T.PASSIVE),
     ("A6", "DP1", T.PASSIVE), ("A7", "DN1", T.PASSIVE), ("A9", "VBUS_A9", T.PASSIVE), ("A12", "GND_A12", T.PASSIVE),
     ("B1", "GND_B1", T.PASSIVE), ("B4", "VBUS_B4", T.PASSIVE), ("B5", "CC2", T.PASSIVE), ("B6", "DP2", T.PASSIVE),
     ("B7", "DN2", T.PASSIVE), ("B9", "VBUS_B9", T.PASSIVE), ("B12", "GND_B12", T.PASSIVE), ("S1", "SHIELD", T.PASSIVE)],
    "USB-C receptacle (USB 2.0)",
)

# --------------------------------------------------------------- nets
gnd = Net("GND")
v5 = Net("+5V")
v3 = Net("+3V3")
vbus = Net("VBUS")
v5_hlk = Net("5V_HLK")
for n in (gnd, v5, v3):
    n.drive = 100  # POWER: driven by off-board supplies / regulator

# ================================================================ power
j_pwr = part(TB2, "J2", "5V in", notes="From HLK-5M05 5 W (off-board, isolated). 1=+5V 2=GND")
j_pwr[1] += v5_hlk
j_pwr[2] += gnd
d_hlk = part(SCHOTTKY, "D1", "SS14", mpn="SS14", notes="OR-ing: supply from mains or USB without backfeed")
d_hlk["A"] += v5_hlk
d_hlk["K"] += v5
d_usb = part(SCHOTTKY, "D2", "SS14", mpn="SS14")
d_usb["A"] += vbus
d_usb["K"] += v5
c_bulk = part(CP, "C1", "100uF 16V", mpn="generic 100uF 16V electrolytic")
c_bulk["+"] += v5
c_bulk["-"] += gnd
c_5v = part(C, "C2", "10uF 10V", footprint=FP_C_BIG)
c_5v[1, 2] += v5, gnd

u_ldo = part(AMS1117, "U1", "AMS1117-3.3", mpn="AMS1117-3.3", notes="Give the tab a copper pour: it is the heatsink")
u_ldo["VIN"] += v5
u_ldo["GND"] += gnd
u_ldo["VOUT"] += v3
for ref, val in (("C3", "10uF"), ("C4", "22uF 6.3V")):
    c = part(C, ref, val, footprint=FP_C_BIG)
    c[1] += v5 if ref == "C3" else v3
    c[2] += gnd

# ================================================================ MCU
u_mcu = part(ESP32C6, "U2", "ESP32-C6-WROOM-1-N8", mpn="ESP32-C6-WROOM-1-N8")
u_mcu["GND"] += gnd  # pins 1, 28
u_mcu["EPAD"] += gnd
u_mcu["3V3"] += v3
for ref, val in (("C5", "10uF"), ("C6", "100nF")):
    c = part(C, ref, val, footprint=FP_C_BIG if "10uF" in val else FP_C)
    c[1, 2] += v3, gnd

en = Net("EN")
u_mcu["EN"] += en
r_en = part(R, "R1", "10k")
r_en[1, 2] += v3, en
c_en = part(C, "C7", "1uF", notes="EN RC delay (Espressif guideline)")
c_en[1, 2] += en, gnd
sw_rst = part(BUTTON, "SW1", "RESET")
sw_rst[1, 2] += en, gnd
sw_boot = part(BUTTON, "SW2", "BOOT", notes="GPIO9 low at reset = download mode")
boot = Net("IO9_BOOT")
u_mcu["IO9"] += boot
sw_boot[1, 2] += boot, gnd

# Unused module pins: explicit no-connects.
for p in ("IO8", "IO11", "IO15", "NC"):
    u_mcu[p] += NC  # left open; IO15 floating = JTAG over USB (default)
dbg = part(HDR4, "J13", "UART debug", notes="1=3V3 2=TXD0 3=RXD0 4=GND")
dbg[1, 2, 3, 4] += v3, Net("TXD0"), Net("RXD0"), gnd
u_mcu["TXD0"] += dbg[2].net
u_mcu["RXD0"] += dbg[3].net

# ---------------------------------------------------------------- USB
j_usb = part(USBC, "J3", "USB-C", mpn="GCT USB4105-GF-A", notes="Flashing, logs (.clog), HIL bridge; can power the board for bench work")
for p in ("VBUS_A4", "VBUS_A9", "VBUS_B4", "VBUS_B9"):
    j_usb[p] += vbus
for p in ("GND_A1", "GND_A12", "GND_B1", "GND_B12", "SHIELD"):
    j_usb[p] += gnd
dp, dn = Net("USB_D+"), Net("USB_D-")
j_usb["DP1"] += dp
j_usb["DP2"] += dp
j_usb["DN1"] += dn
j_usb["DN2"] += dn
for ref, cc in (("R2", "CC1"), ("R3", "CC2")):
    r = part(R, ref, "5.1k", notes="UFP identification")
    r[1, 2] += j_usb[cc].net if j_usb[cc].net else Net(cc), gnd
    j_usb[cc] += r[1].net
u_esd = part(USBLC6, "U6", "USBLC6-2SC6", mpn="USBLC6-2SC6")
u_esd["IO1a"] += dn
u_esd["IO1b"] += dn
u_esd["IO2a"] += dp
u_esd["IO2b"] += dp
u_esd["VBUS"] += vbus
u_esd["GND"] += gnd
u_mcu["IO12"] += dn
u_mcu["IO13"] += dp

# ================================================================ zero cross (ISOLATED SECTION)
mains_l, mains_n = Net("MAINS_L"), Net("MAINS_N")
j_ac = part(TB2_MAINS, "J1", "Mains sense", notes="L/N for zero-cross ONLY (≈4 mA). Behind ≥6.4 mm isolation slot. 1=L 2=N")
j_ac[1, 2] += mains_l, mains_n
f_zc = part(FUSE, "F1", "100mA slow", mpn="generic 1206 100 mA slow-blow, 250 VAC", notes="MAINS")
zc_a, zc_b = Net("ZC_A"), Net("ZC_B")
f_zc[1, 2] += mains_l, Net("ZC_F")
r_zc_val = "47k 1W" if MAINS_V >= 200 else "22k 1W"
r_zc1 = part(R, "R4", r_zc_val, footprint=FP_R_MAINS, notes="MAINS: 2512, working voltage ≥ 250 V. Two in series split the voltage.")
r_zc2 = part(R, "R5", r_zc_val, footprint=FP_R_MAINS, notes="MAINS")
r_zc1[1, 2] += f_zc[2].net, zc_a
r_zc2[1, 2] += mains_n, zc_b
u_zc = part(H11AA1, "U5", "H11AA1", mpn="onsemi H11AA1SR2M (SMD wide)", notes="Isolation barrier: pins 1-2 mains, 4-6 SELV")
u_zc["AC1"] += zc_a
u_zc["AC2"] += zc_b
u_zc["NC"] += NC
u_zc["B"] += NC
zc = Net("ZC")
u_zc["E"] += gnd
u_zc["C"] += zc
r_zcpu = part(R, "R6", "47k", notes="Pull-up: output HIGH for ~0.9 ms centred on each zero crossing (10k would give ~4.4 ms)")
r_zcpu[1, 2] += v3, zc
c_zc = part(C, "C8", "100pF", notes="Edge clean-up")
c_zc[1, 2] += zc, gnd
u_mcu["IO4"] += zc  # firmware ZC input (MTMS strap: affects SDIO edge only)
u_mcu["IO4"].func = T.INPUT

# ================================================================ thermocouple
u_tc = part(MAX31855, "U3", "MAX31855KASA+", mpn="MAX31855KASA+T")
u_tc["GND"] += gnd
u_tc["VCC"] += v3
u_tc["DNC"] += NC
c_tcv = part(C, "C9", "100nF")
c_tcv[1, 2] += v3, gnd
tcp, tcm = Net("TC+"), Net("TC-")
j_tc = part(TB2, "J4", "Thermocouple K", notes="UNGROUNDED-junction probe only (the boiler is earthed). 1=T+ 2=T-")
j_tc[1, 2] += tcp, tcm
u_tc["T+"] += tcp
u_tc["T-"] += tcm
c_tcd = part(C, "C10", "10nF", notes="Differential filter per MAX31855 datasheet")
c_tcd[1, 2] += tcp, tcm
sck, miso, cs = Net("TC_SCK"), Net("TC_SO"), Net("TC_CS")
u_tc["SCK"] += sck
u_tc["SO"] += miso
u_tc["CS"] += cs
r_cs = part(R, "R7", "10k", notes="Keeps CS deselected during boot")
r_cs[1, 2] += v3, cs
u_mcu["IO6"] += sck
u_mcu["IO2"] += miso
u_mcu["IO7"] += cs

# ================================================================ ADC: pressure, 5 V monitor, group NTC, spare
u_adc = part(ADS1115, "U4", "ADS1115IDGSR", mpn="ADS1115IDGSR")
u_adc["VDD"] += v3
u_adc["GND"] += gnd
u_adc["ADDR"] += gnd  # 0x48
c_adcv = part(C, "C11", "100nF")
c_adcv[1, 2] += v3, gnd
sda, scl, alert = Net("SDA"), Net("SCL"), Net("ADC_RDY")
u_adc["SDA"] += sda
u_adc["SCL"] += scl
u_adc["ALERT"] += alert
for ref, net in (("R8", sda), ("R9", scl), ("R10", alert)):
    r = part(R, ref, "4.7k" if net is not alert else "10k")
    r[1, 2] += v3, net
u_mcu["IO22"] += sda
u_mcu["IO23"] += scl
u_mcu["IO10"] += alert  # conversion-ready (optional use)
u_mcu["IO10"].func = T.INPUT

# pressure transducer: 5 V ratiometric 0.5-4.5 V
p_sig, p_div = Net("P_SIG"), Net("P_AIN0")
j_p = part(JST3, "J5", "Pressure 0-1.6MPa", notes="1=+5V 2=SIG 3=GND")
j_p[1, 2, 3] += v5, p_sig, gnd
d_p = part(TVS, "D3", "PESD5V0S1BA", mpn="PESD5V0S1BA")
d_p[1, 2] += p_sig, gnd
r_p1 = part(R, "R11", "10k 0.1%", notes="Divider 2/3: 4.5 V -> 3.0 V")
r_p2 = part(R, "R12", "20k 0.1%")
r_p1[1, 2] += p_sig, p_div
r_p2[1, 2] += p_div, gnd
c_p = part(C, "C12", "100nF", notes="fc ≈ 240 Hz (6.7k Thevenin)")
c_p[1, 2] += p_div, gnd
u_adc["AIN0"] += p_div

# 5 V rail monitor for ratiometric correction of the transducer
v5_div = Net("V5_AIN1")
r_v1 = part(R, "R13", "10k 0.1%")
r_v2 = part(R, "R14", "20k 0.1%")
r_v1[1, 2] += v5, v5_div
r_v2[1, 2] += v5_div, gnd
c_v = part(C, "C13", "100nF")
c_v[1, 2] += v5_div, gnd
u_adc["AIN1"] += v5_div

# optional group-head NTC (100k B3950 ring lug)
ntc = Net("NTC_AIN2")
j_ntc = part(TB2, "J6", "Group NTC (opt)", notes="100k B3950 ring-lug NTC under a group screw. 1=NTC 2=GND", populate="opt")
j_ntc[1, 2] += ntc, gnd
r_ntc = part(R, "R15", "10k 0.1%", notes="Reference: ~1.4 V at 90 C, ~3.0 V at 25 C", populate="opt")
r_ntc[1, 2] += v3, ntc
c_ntc = part(C, "C14", "100nF", populate="opt")
c_ntc[1, 2] += ntc, gnd
u_adc["AIN2"] += ntc

# spare analog input (e.g. tank level sensor)
ain3, ain3_in = Net("SPARE_AIN3"), Net("SPARE_IN")
j_sp = part(TB3, "J7", "Spare analog", notes="1=+3V3 2=IN (0-3.3 V) 3=GND", populate="opt")
j_sp[1, 2, 3] += v3, ain3_in, gnd
r_sp = part(R, "R16", "1k", populate="opt")
r_sp[1, 2] += ain3_in, ain3
c_sp = part(C, "C15", "100nF", populate="opt")
c_sp[1, 2] += ain3, gnd
u_adc["AIN3"] += ain3

# ================================================================ SSR drivers (heater, pump, valve)
for i, (name, gpio, jref) in enumerate((("HEATER", "IO5", "J8"), ("PUMP", "IO18", "J9"), ("VALVE", "IO19", "J10"))):
    gate, drain, sig = Net(f"{name}_G"), Net(f"{name}_SSR-"), Net(f"{name}_CMD")
    j = part(TB2, jref, f"{name.title()} SSR", notes="1=+5V (SSR input +) 2=SSR input − (switched low)")
    j[1, 2] += v5, drain
    q = part(AO3400A, f"Q{i+1}", "AO3400A", mpn="AO3400A")
    q["D"] += drain
    q["S"] += gnd
    q["G"] += gate
    rg = part(R, f"R{17+i}", "100", notes="Gate series")
    rg[1, 2] += sig, gate
    rpd = part(R, f"R{20+i}", "100k", notes="Gate pull-down: OFF while the ESP32 boots/resets")
    rpd[1, 2] += gate, gnd
    led = part(LED, f"D{4+i}", f"{name.lower()} green", notes="ON when the SSR is driven")
    rl = part(R, f"R{23+i}", "1k")
    led_a = Net(f"{name}_LED")
    rl[1, 2] += v5, led_a
    led["A"] += led_a
    led["K"] += drain
    u_mcu[gpio] += sig

# ================================================================ front-panel switches (dry contacts)
j_sw = part(TB3, "J11", "Switches", notes="1=BREW 2=STEAM 3=GND. Switches rewired as dry contacts (low voltage)")
for i, (name, gpio) in enumerate((("BREW", "IO20"), ("STEAM", "IO21"))):
    wire, pin = Net(f"SW_{name}"), Net(f"SW_{name}_IN")
    j_sw[i + 1] += wire
    rpu = part(R, f"R{26+i}", "10k", notes="Pull-up (switch to GND = on)")
    rpu[1, 2] += v3, wire
    rs = part(R, f"R{28+i}", "1k", notes="Series: limits ESD current into the GPIO")
    rs[1, 2] += wire, pin
    cf = part(C, f"C{16+i}", "100nF", notes="RC 0.1 ms: debounce/EMI on long leads near mains")
    cf[1, 2] += pin, gnd
    d = part(TVS, f"D{7+i}", "PESD5V0S1BA", mpn="PESD5V0S1BA")
    d[1, 2] += wire, gnd
    u_mcu[gpio] += pin
j_sw[3] += gnd

# ================================================================ heater current transformer (optional)
ct_mid, ct_sig = Net("CT_MID"), Net("CT_ADC")
j_ct = part(TB2, "J12", "Heater CT (opt)", notes="SCT-013-020 (20 A : 1 V, burden inside) around the heater LIVE wire. 1=CT 2=CT", populate="opt")
r_b1 = part(R, "R30", "10k", populate="opt")
r_b2 = part(R, "R31", "10k", populate="opt")
r_b1[1, 2] += v3, ct_mid
r_b2[1, 2] += ct_mid, gnd
c_b = part(C, "C18", "10uF", footprint=FP_C_BIG, populate="opt", notes="Mid-rail bias 1.65 V")
c_b[1, 2] += ct_mid, gnd
j_ct[2] += ct_mid
r_ct = part(R, "R32", "1k", populate="opt")
j_ct[1] += Net("CT_IN")
r_ct[1, 2] += j_ct[1].net, ct_sig
c_ct = part(C, "C19", "1nF", populate="opt")
c_ct[1, 2] += ct_sig, gnd
u_mcu["IO0"] += ct_sig  # ADC1_CH0: confirms the heater actually draws current (SSR stuck on/off, open element)
u_mcu["IO1"] += NC  # ADC1_CH1 spare (e.g. a pump CT)

# ================================================================ status LED
st = Net("STATUS_LED")
led_s = part(LED, "D9", "status blue")
r_s = part(R, "R33", "1k")
u_mcu["IO3"] += st
r_s[1, 2] += st, Net("STATUS_A")
led_s["A"] += r_s[2].net
led_s["K"] += gnd

# ================================================================ outputs
def design_checks():
    """Checks ERC can't express. Generation fails if any of these break."""
    errors = []
    # 1. Isolation: mains nets may only touch the zero-cross input parts.
    allowed = {("J1", "1"), ("J1", "2"), ("F1", "1"), ("F1", "2"), ("R4", "1"), ("R4", "2"),
               ("R5", "1"), ("R5", "2"), ("U5", "1"), ("U5", "2")}
    for net in (mains_l, mains_n, f_zc[2].net, zc_a, zc_b):
        for pin in net.pins:
            if (pin.part.ref, pin.num) not in allowed:
                errors.append(f"ISOLATION: mains net {net.name} touches {pin.part.ref}.{pin.num}")
    # 2. Firmware pin map (firmware/esp32/src/main.rs) lands on the right signals.
    fw = {"IO4": "ZC", "IO5": "HEATER_CMD", "IO18": "PUMP_CMD", "IO19": "VALVE_CMD", "IO20": "SW_BREW_IN",
          "IO21": "SW_STEAM_IN", "IO6": "TC_SCK", "IO2": "TC_SO", "IO7": "TC_CS", "IO22": "SDA", "IO23": "SCL",
          "IO12": "USB_D-", "IO13": "USB_D+"}
    for pin, want in fw.items():
        got = u_mcu[pin].net.name if u_mcu[pin].net else None
        if got != want:
            errors.append(f"PINMAP: {pin} is on {got}, firmware expects {want}")
    # 3. Every SSR output is pulled off at boot.
    for name in ("HEATER", "PUMP", "VALVE"):
        g = [n for n in default_circuit_nets() if n.name == f"{name}_G"][0]
        if not any(p.part.value == "100k" for p in g.pins):
            errors.append(f"BOOT-SAFE: {name} gate has no pull-down")
    if errors:
        print("\n".join(errors))
        sys.exit(1)
    print("design checks: isolation OK, firmware pin map OK, SSR gates pulled down")


def default_circuit_nets():
    import builtins

    return builtins.default_circuit.nets


if __name__ == "__main__":
    ERC()
    design_checks()
    net_path = os.path.join(OUT, f"coffee-controller-{MAINS_V}v.net")
    generate_netlist(file_=net_path, tool=KICAD8)
    import builtins

    default_circuit = builtins.default_circuit

    rows = {}
    for p in default_circuit.parts:
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
    print(f"wrote {net_path}\nwrote {bom_path}\n{len(default_circuit.parts)} parts, {len(default_circuit.nets)} nets")
