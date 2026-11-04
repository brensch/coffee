#!/usr/bin/env python3
"""coffee controller board v6: schematic as code (SKiDL).

The minimum hardware for a Gaggia Classic Pro controller; everything that can live in
firmware lives there.
- An ESP32-C6 (external-antenna module) reads the thermistor and the pressure transducer on
  its own ADC.
- Three zero-cross opto-triacs fire three triacs (heater, pump, valve) straight from GPIOs.
- An isolated 5 V module powers it.
The mains half and the SELV half meet only inside the opto-triacs and the PSU module, across
a 7 mm copper-free band with milled slots. Notes and calculations: docs/CIRCUIT.md.

    python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
    .venv/bin/python controller.py                 # 120 V (the target machine): checks -> build/
    MAINS_V=230 .venv/bin/python controller.py     # 230 V variant

Outputs in build/: KiCad netlist, BOM CSV, parts-<V>v.json (input to layout.py, audit.py and
schematic_svg.py).

Pinouts: ESP32-C6-MINI-1U (Espressif datasheet / KiCad library, lib/Espressif.pretty),
MOC3083, RECOM RAC03E-K/277, TLV1117LV (AMS1117 pinout), BTA24/BTA08 (TO-220: 1 MT1, 2 MT2, 3 G),
Z0109 (SOT-223: 1 MT1, 2/tab MT2, 3 G).
"""

import csv
import json
import os
import re
import sys

from skidl import ERC, Net, Part, Pin, SKIDL, TEMPLATE, generate_netlist, KICAD8

import builtins

NC = builtins.NC

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "build")
os.makedirs(OUT, exist_ok=True)
MAINS_V = int(os.environ.get("MAINS_V", "120"))   # the target machine is a US 120 V Gaggia Classic Pro

T = Pin.types
BOM_EXTRA = {}
MAINS_PINS = set()


def tmpl(name, prefix, footprint, pins, desc=""):
    return Part(name=name, tool=SKIDL, dest=TEMPLATE, ref_prefix=prefix, footprint=footprint, description=desc,
                pins=[Pin(num=str(n), name=nm, func=f) for (n, nm, f) in pins])


def part(template, ref, value="", mpn="", notes="", populate="Y", footprint=None, mains=None):
    p = template(ref=ref, value=value, tag=ref)
    if footprint:
        p.footprint = footprint
    BOM_EXTRA[p.ref] = dict(mpn=mpn, notes=notes, populate=populate)
    if mains is True:
        MAINS_PINS.update((ref, pin.num) for pin in p.pins)
    elif mains:
        MAINS_PINS.update((ref, str(n)) for n in mains)
    return p


# --------------------------------------------------------------- templates
FP_R, FP_R_1206, FP_R_2512 = "Resistor_SMD:R_0603_1608Metric", "Resistor_SMD:R_1206_3216Metric", "Resistor_SMD:R_2512_6332Metric"
FP_C, FP_C_1206 = "Capacitor_SMD:C_0603_1608Metric", "Capacitor_SMD:C_1206_3216Metric"
TWO = [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)]
R = tmpl("R", "R", FP_R, TWO, "Resistor")
C = tmpl("C", "C", FP_C, TWO, "Capacitor")
CX2 = tmpl("C_X2", "C", "Capacitor_SMD:C_2220_5750Metric", TWO, "X2 safety capacitor, ceramic, 250 VAC")
LED = tmpl("LED", "D", "LED_SMD:LED_0603_1608Metric", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "LED")
SCHOTTKY = tmpl("SS14", "D", "Diode_SMD:D_SMA", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "Schottky 1 A 40 V")
FUSE = tmpl("Fuse_TR5", "F", "Fuse:Fuse_Littelfuse_372_D8.50mm", TWO, "Fuse, TR5 radial")
TESTPAD = tmpl("TestPad", "TP", "TestPoint:TestPoint_Pad_D1.5mm", [(1, "1", T.PASSIVE)], "Bare test pad")
HOLE = tmpl("MountingHole", "H", "MountingHole:MountingHole_3.2mm_M3", [], "M3 mounting hole (unplated)")
HOLE_M25 = tmpl("MountingHole_M2.5", "H", "MountingHole:MountingHole_2.7mm_M2.5", [], "M2.5 mounting hole (unplated), beside the PSU")


def conn(n, name, fp):
    return tmpl(name, "J", fp, [(i, str(i), T.PASSIVE) for i in range(1, n + 1)], name)


TB2_MAINS = conn(2, "Screw_Terminal_01x02_5.08", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MKDS-1,5-2-5.08_1x02_P5.08mm_Horizontal")
TB2_HEATER = conn(2, "Screw_Terminal_01x02_5.08_24A", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MKDS-3-2-5.08_1x02_P5.08mm_Horizontal")   # 11.4 A at 60-70 C ambient
JST2 = conn(2, "JST_PH_2", "Connector_JST:JST_PH_B2B-PH-K_1x02_P2.00mm_Vertical")
JST3 = conn(3, "JST_PH_3", "Connector_JST:JST_PH_B3B-PH-K_1x03_P2.00mm_Vertical")

# TI TLV1117LV33: stable with ceramic output caps (the AMS1117 wants tantalum ESR and can oscillate on
# ceramics), 455 mV dropout at 1 A, AMS1117 pinout.
LDO = tmpl("TLV1117LV33", "U", "Package_TO_SOT_SMD:SOT-223-3_TabPin2",
               [(1, "GND", T.PWRIN), (2, "VOUT", T.PWROUT), (3, "VIN", T.PWRIN)], "LDO 3.3 V 1 A, ceramic-stable")
# RECOM RAC03E-05SK/277 (datasheet REV 3/2023): -40..+85 C (full load to 75 C), 4 kVAC reinforced,
# EN60335-1 / IEC61558 / UL62368, internal fusible input resistor, EMC class B with no external parts.
PSU = tmpl("RAC03E-05SK/277", "PS", "Recom:RAC03E-K_277",
           [(1, "AC_L", T.PWRIN), (2, "AC_N", T.PWRIN), (3, "NC", T.NOCONNECT), (4, "-VO", T.PWROUT), (5, "+VO", T.PWROUT)],
           "Isolated AC-DC 5 V 0.6 A, 3 W, 85 C rated, 4 kVAC reinforced (EN60335-1)")
# ESP32-C6-MINI-1U pads (Espressif datasheet / KiCad symbol "ESP32-C6-MINI-1/U"; lib/README.md).
# 36-53 are GND: the inner row, the 3x3 exposed pad (49, nine sub-pads) and the four corners.
_c6 = {1: "GND", 2: "GND", 3: "3V3", 4: "NC", 5: "IO2", 6: "IO3", 7: "NC", 8: "EN", 9: "IO4", 10: "IO5", 11: "GND",
       12: "IO0", 13: "IO1", 14: "GND", 15: "IO6", 16: "IO7", 17: "IO12", 18: "IO13", 19: "IO14", 20: "IO15", 21: "NC",
       22: "IO8", 23: "IO9", 24: "IO18", 25: "IO19", 26: "IO20", 27: "IO21", 28: "IO22", 29: "IO23", 30: "RXD0",
       31: "TXD0", **{n: "NC" for n in range(32, 36)}, **{n: "GND" for n in range(36, 54)}}
_uniq = lambda n, nm: f"{nm}{n}" if nm in ("GND", "NC") else nm
ESP32C6 = tmpl("ESP32-C6-MINI-1U", "U", "Espressif:ESP32-C6-MINI-1U",
               [(n, _uniq(n, nm), T.PWRIN if nm in ("GND", "3V3") else (T.NOCONNECT if nm == "NC" else T.BIDIR))
                for n, nm in _c6.items()],
               "Wi-Fi 6 / BLE 5 RISC-V module, W.FL / MHF III antenna connector, 4 MB flash, 105 C rated")
MOC3083 = tmpl("MOC3083", "U", "Package_DIP:DIP-6_W10.16mm",
               [(1, "A", T.PASSIVE), (2, "K", T.PASSIVE), (3, "NC", T.NOCONNECT),
                (4, "MT1", T.PASSIVE), (5, "NC2", T.NOCONNECT), (6, "MT2", T.PASSIVE)],
               "Zero-cross triac driver opto, 800 V, IFT 5 mA max")
TRIAC_TO220 = tmpl("Triac_TO220", "Q", "Package_TO_SOT_THT:TO-220-3_Vertical",
                   [(1, "MT1", T.PASSIVE), (2, "MT2", T.PASSIVE), (3, "G", T.PASSIVE)], "Triac, insulated tab")
TRIAC_SOT223 = tmpl("Triac_SOT223", "Q", "Package_TO_SOT_SMD:SOT-223-3_TabPin2",
                    [(1, "MT1", T.PASSIVE), (2, "MT2", T.PASSIVE), (3, "G", T.PASSIVE)], "Triac, sensitive gate")
USBC = tmpl("USB_C_Receptacle_USB2.0", "J", "Connector_USB:USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal",
            [("A1", "GND_A1", T.PASSIVE), ("A4", "VBUS_A4", T.PASSIVE), ("A5", "CC1", T.PASSIVE),
             ("A6", "DP1", T.PASSIVE), ("A7", "DN1", T.PASSIVE), ("A9", "VBUS_A9", T.PASSIVE), ("A12", "GND_A12", T.PASSIVE),
             ("B1", "GND_B1", T.PASSIVE), ("B4", "VBUS_B4", T.PASSIVE), ("B5", "CC2", T.PASSIVE), ("B6", "DP2", T.PASSIVE),
             ("B7", "DN2", T.PASSIVE), ("B9", "VBUS_B9", T.PASSIVE), ("B12", "GND_B12", T.PASSIVE), ("S1", "SHIELD", T.PASSIVE)],
            "USB-C receptacle (USB 2.0): flashing, logs, JTAG, bench power")

# --------------------------------------------------------------- nets
gnd, v5, v3, vbus = Net("GND"), Net("+5V"), Net("+3V3"), Net("VBUS")
for n in (gnd, v5, v3):
    n.drive = 100

# ================================================================ MAINS SIDE
l_in, n_in = Net("AC_L"), Net("AC_N")
l_in.drive = n_in.drive = 100
j_ac = part(TB2_MAINS, "J1", "Mains in", mains=True, mpn="Phoenix Contact MKDS 1,5/2-5.08 (1715721)",
            notes="MAINS. 1=L 2=N, after the machine's power switch. Feeds pump, valve and PSU (~0.5 A).")
j_ac[1, 2] += l_in, n_in
htr_a, htr_b = Net("HTR_A"), Net("HTR_B")
j_htr = part(TB2_HEATER, "J2", "Heater", mains=True, mpn="Phoenix Contact MKDS 3/2-5.08 (1711725), 24 A IEC / UL 15 A, 4 mm2",
             notes="MAINS, HEATER CURRENT (11.4 A at 120 V). In place of the brew thermostat: the stock steam thermostat and thermal fuse stay in series.")
j_htr[1, 2] += htr_a, htr_b
pump_out, valve_out = Net("PUMP_L"), Net("VALVE_L")
j_load = part(TB2_MAINS, "J3", "Valve / pump", mains=True, mpn="Phoenix Contact MKDS 1,5/2-5.08 (1715721)", notes="MAINS. 1=switched L to the 3-way valve, 2=switched L to the pump.")
j_load[1, 2] += valve_out, pump_out

l_f = Net("AC_L_FUSED")
l_f.drive = 100
f1 = part(FUSE, "F1", "T500mA", mpn="Littelfuse 37205000411 (TR5, 500 mA slow, 250 VAC)", mains=True,
          notes="MAINS. Protects the PSU branch")
f1[1, 2] += l_in, l_f
# No MOV: behind a T500mA fuse a failing MOV can burn at 60-110 W before the fuse opens, a real surge
# blows the fuse anyway, and the RECOM passes its 1 kV surge test unaided (EN 61000-4-5). The triacs
# and opto-triacs are 800 V parts.
ps1 = part(PSU, "PS1", "RAC03E-05SK/277", mpn="RECOM RAC03E-05SK/277", mains=["1", "2"],
           notes="Isolated supply; straddles the slot. Rated to 85 C: the machine's case runs at 60-70 C")
ps1["NC"] += NC
ps1["AC_L"] += l_f
ps1["AC_N"] += n_in
ps1["+VO"] += v5
ps1["-VO"] += gnd


def triac_channel(name, n, mt2_net, mt1_net, triac_tpl, triac_val, triac_mpn, rs_ref, snub=None, rgk_ref=None):
    """MOC3083 zero-cross opto-triac firing a triac (onsemi AN-3004). Gate-MT1 resistor only for a
    sensitive-gate triac; RC snubber only for an inductive load (sim/REPORT.md)."""
    u = part(MOC3083, f"U{n}", "MOC3083", mpn="onsemi MOC3083TVM (0.4\" leads)", mains=["4", "5", "6"],
             notes=f"{name}: isolation barrier, pins 1-2 SELV, 4-6 mains. Fires only within ±20 V of a zero crossing")
    q = part(triac_tpl, f"Q{n}", triac_val, mpn=triac_mpn, mains=True, notes=f"MAINS: {name}")
    gate, opto_mt2 = Net(f"{name}_GATE"), Net(f"{name}_OPTO")
    q["MT2"] += mt2_net
    q["MT1"] += mt1_net
    q["G"] += gate
    r_s = part(R, rs_ref, "360", footprint=FP_R_1206, mains=True, notes="MAINS: limits opto surge current (1 A peak)")
    r_s[1, 2] += mt2_net, opto_mt2
    u["MT2"] += opto_mt2
    u["MT1"] += gate
    u["NC"] += NC
    u["NC2"] += NC
    if rgk_ref:
        r_g = part(R, rgk_ref, "330", footprint=FP_R_1206, mains=True, notes="MAINS: gate-MT1 for a sensitive-gate triac")
        r_g[1, 2] += gate, mt1_net
    if snub:
        r_ref, c_ref, c_val = snub
        s_mid = Net(f"{name}_SNUB")
        r_n = part(R, r_ref, "100 1W", footprint=FP_R_2512, mains=True, notes="MAINS: snubber (inductive load)")
        c_n = part(CX2, c_ref, f"{c_val} X2", mpn="Murata GA355QR7GB103KW01L (10 nF X2 250 VAC, 2220)", mains=True, notes="MAINS: snubber")
        r_n[1, 2] += mt2_net, s_mid
        c_n[1, 2] += s_mid, mt1_net
    return u


# Heater (resistive): BTA24, 25 A, Rth(j-c) 1.7 C/W; ~11 W at 11.4 A -> ~24 C over the chassis it is bolted to.
u_h = triac_channel("HEATER", 1, htr_b, htr_a, TRIAC_TO220, "BTA24-800BW", "ST BTA24-800BWRG (25 A, 800 V, insulated tab, snubberless)", "R1")
# All three channels use 800 V parts: line surges reach them unclamped (no MOV), and an inductive load
# turned off near the line peak rings to ~2x that peak (sim/REPORT.md).
# Pump (Ulka, diode inside: inductive). Without a snubber the triac sees ~574 V at turn-off.
u_p = triac_channel("PUMP", 2, l_in, pump_out, TRIAC_TO220, "BTA08-800BW", "ST BTA08-800BWRG (8 A, 800 V, insulated tab, snubberless; the CW gate grade is 600 V only)", "R2",
                    snub=("R3", "C1", "10nF"))
# Valve (AC solenoid). The triac drops out at its holding current with the coil still carrying it:
# ~2 kV without a snubber, <= 477 V with 100 R + 10 nF (8 nF after ceramic derating). Z0109 is
# sensitive-gate: keeps its gate resistor. Both snubbers use the same SMD X2 part.
u_v = triac_channel("VALVE", 3, l_in, valve_out, TRIAC_SOT223, "Z0109NN", "ST Z0109NN5AA4 (1 A, 800 V, IGT 10 mA)", "R4",
                    snub=("R6", "C2", "10nF"), rgk_ref="R5")

# ================================================================ SELV SIDE
u_mcu = part(ESP32C6, "U4", "ESP32-C6-MINI-1U-H4", mpn="ESP32-C6-MINI-1U-H4",
             notes="W.FL / MHF III antenna (not U.FL): pigtail to a 2.4 GHz antenna outside the steel case")
for pin in u_mcu.pins:
    if pin.name.startswith("GND"):
        pin += gnd
    elif pin.name.startswith("NC"):
        pin += NC
u_mcu["3V3"] += v3
for ref, val, fp in (("C3", "10uF 10V X7R", "Capacitor_SMD:C_0805_2012Metric"), ("C4", "100nF X7R", FP_C)):
    c = part(C, ref, val, footprint=fp, notes="Module decoupling at the 3V3 pin")
    c[1, 2] += v3, gnd
en = Net("EN")
u_mcu["EN"] += en
r = part(R, "R10", "10k", notes="EN pull-up (Espressif RC delay)")
r[1, 2] += v3, en
c = part(C, "C5", "1uF X7R", notes="EN delay")
c[1, 2] += en, gnd
strap8 = Net("IO8_STRAP")
u_mcu["IO8"] += strap8
r = part(R, "R11", "10k", notes="GPIO8 high: required for download mode from the BOOT pad")
r[1, 2] += v3, strap8
boot = Net("IO9_BOOT")
u_mcu["IO9"] += boot   # internal pull-up: normal boot
tp = part(TESTPAD, "TP1", "BOOT", notes="Recovery only: short to TP2 (GND) while plugging USB in")
tp[1] += boot
tp = part(TESTPAD, "TP2", "GND")
tp[1] += gnd
st, st_a = Net("STATUS"), Net("STATUS_A")
u_mcu["IO23"] += st
r = part(R, "R12", "100", notes="green LED Vf ~2.9-3.1 V: ~2-4 mA from 3.3 V")
r[1, 2] += st, st_a
led = part(LED, "D1", "green", notes="Heartbeat / ready / fault. Light-pipe it to the front of the machine (optional)")
led["A"] += st_a
led["K"] += gnd

# Power: PSU -> +5V; USB VBUS joins through D2 (bench power without mains, no back-feed into the PC).
d2 = part(SCHOTTKY, "D2", "SS14", mpn="SS14", notes="USB bench power")
d2["A"] += vbus
d2["K"] += v5
u_ldo = part(LDO, "U5", "TLV1117LV33", mpn="TI TLV1117LV33DCYR", notes="Tab pour is the heatsink")
u_ldo["VIN"] += v5
u_ldo["GND"] += gnd
u_ldo["VOUT"] += v3
for ref, val, net in (("C6", "22uF 10V X7R", v5), ("C7", "22uF 10V X7R", v3)):
    c = part(C, ref, val, footprint="Capacitor_SMD:C_0805_2012Metric", mpn="Murata GRM21BZ71A226ME15L", notes="Ceramic bulk, X7R (125 C), no electrolytics to age at 60-70 C")
    c[1, 2] += net, gnd

j_usb = part(USBC, "J4", "USB-C", mpn="GCT USB4105-GF-A", notes="Flashing, logs, JTAG over the C6's USB Serial/JTAG; bench power")
for p in ("VBUS_A4", "VBUS_A9", "VBUS_B4", "VBUS_B9"):
    j_usb[p] += vbus
for p in ("GND_A1", "GND_A12", "GND_B1", "GND_B12", "SHIELD"):
    j_usb[p] += gnd
dp, dn = Net("USB_D+"), Net("USB_D-")
j_usb["DP1", "DP2"] += dp
j_usb["DN1", "DN2"] += dn
for ref, cc in (("R13", "CC1"), ("R14", "CC2")):
    ccn = Net(cc)
    j_usb[cc] += ccn
    r = part(R, ref, "5.1k", notes="USB-C sink identification")
    r[1, 2] += ccn, gnd
u_mcu["IO12"] += dn
u_mcu["IO13"] += dp

# Triac drive: GPIO -> 150 R -> opto LED -> GND. 8-12 mA at every corner (sim/REPORT.md).
# Loads are off at boot by physics: a floating or weakly-pulled pin can't push the 5 mA the opto needs.
# GPIO order on the module edge (IO18, IO19, IO20 top to bottom) matches the opto order (valve, pump, heater).
for name, gpio, u, ref in (("HEATER", "IO20", u_h, "R7"), ("PUMP", "IO19", u_p, "R8"), ("VALVE", "IO18", u_v, "R9")):
    cmd, anode = Net(f"{name}_CMD"), Net(f"{name}_OPTO_A")
    r = part(R, ref, "150", notes="Opto LED current from the GPIO")
    r[1, 2] += cmd, anode
    u["A"] += anode
    u["K"] += gnd
    u_mcu[gpio] += cmd

# Thermistor: 100k B3950 NTC (probe) to GND, 8.2k reference to 3V3: ~95 C sits near mid-scale.
ntc = Net("NTC")
j_ntc = part(JST2, "J5", "Thermistor", notes="100k B3950 M4 probe in the brew-thermostat hole. 1=NTC 2=GND (no polarity)")
j_ntc[1, 2] += ntc, gnd
r = part(R, "R15", "8.2k 0.1%", notes="NTC reference: 3.05 V at 25 C, ~1.65 V at 95 C, 0.7 V at 145 C")
r[1, 2] += v3, ntc
c = part(C, "C8", "100nF X7R", notes="ADC sampling reservoir / noise filter")
c[1, 2] += ntc, gnd
u_mcu["IO1"] += ntc   # ADC1_CH1

# Pressure: 0.5-4.5 V ratiometric transducer on +5V, divided by 2/3 (4.5 V -> 3.0 V) into ADC1.
p_sig, p_adc = Net("P_SIG"), Net("P_ADC")
j_p = part(JST3, "J6", "Pressure", notes="0-1.6 MPa, 5 V, 0.5-4.5 V. 1=+5V 2=SIG 3=GND")
j_p[1, 2, 3] += v5, p_sig, gnd
r = part(R, "R16", "10k 0.1%", notes="Divider 2/3")
r[1, 2] += p_sig, p_adc
r = part(R, "R17", "20k 0.1%")
r[1, 2] += p_adc, gnd
c = part(C, "C9", "100nF X7R", notes="ADC reservoir; ~240 Hz with the divider: keeps the pump's 60 Hz pulses")
c[1, 2] += p_adc, gnd
u_mcu["IO0"] += p_adc   # ADC1_CH0

# Front-panel switches: dry contacts to GND; internal pull-ups, debounced in firmware.
j_sw = part(JST3, "J7", "Switches", notes="1=BREW 2=STEAM 3=GND. Stock switches rewired as dry contacts")
# GPIO6/7: plain GPIOs on the module edge beside the analog pins, so all three sensor/switch
# connectors sit together on the right edge (GPIO21/22 face the optos and are boxed in).
# 4.7k pull-up at the connector: ~0.7 mA wetting current keeps old switch contacts clean. 1k series
# into the GPIO: a 5 V plug in the wrong socket only pushes ~1.7 mA into the pin's clamp diode.
for i, (name, gpio, rpu, rs) in enumerate((("BREW", "IO6", "R20", "R18"), ("STEAM", "IO7", "R21", "R19"))):
    wire, pin = Net(f"SW_{name}_IN"), Net(f"SW_{name}")
    j_sw[i + 1] += wire
    r = part(R, rpu, "4.7k", notes="switch pull-up / wetting current")
    r[1, 2] += v3, wire
    r = part(R, rs, "1k", notes="GPIO series protection")
    r[1, 2] += wire, pin
    u_mcu[gpio] += pin
j_sw[3] += gnd

# Rail sense: the thermistor divider is ratiometric to 3V3 and the transducer to +5V, but the ADC
# measures absolute volts. Measuring both rails lets the firmware divide them out, so an open
# thermistor reads exactly full scale (3300 mV) whatever the rail, and 0 C stays well clear of it.
for net, rail, gpio, rt_, rb_, vt, cref in (("V3_SNS", v3, "IO2", "R22", "R23", "10k 0.1%", "C10"), ("V5_SNS", v5, "IO3", "R24", "R25", "20k 0.1%", "C11")):
    sns = Net(net)
    r = part(R, rt_, vt, notes=f"{net} divider top")
    r[1, 2] += rail, sns
    r = part(R, rb_, "10k 0.1%", notes=f"{net} divider bottom")
    r[1, 2] += sns, gnd
    c = part(C, cref, "100nF X7R", notes="ADC reservoir")
    c[1, 2] += sns, gnd
    u_mcu[gpio] += sns

for p in ("IO4", "IO5", "IO14", "IO15", "IO21", "IO22", "RXD0", "TXD0"):
    u_mcu[p] += NC
# No mounting holes: the enclosure holds the board by its edges (and the heater triac's tab is bolted
# to the chassis), so no copper is given up to screws.


# ================================================================ checks + outputs
def nets():
    return builtins.default_circuit.nets


FIRMWARE_PINS = {"IO0": "P_ADC", "IO1": "NTC", "IO2": "V3_SNS", "IO3": "V5_SNS", "IO18": "VALVE_CMD", "IO19": "PUMP_CMD", "IO20": "HEATER_CMD",
                 "IO6": "SW_BREW", "IO7": "SW_STEAM", "IO23": "STATUS", "IO12": "USB_D-", "IO13": "USB_D+"}
STRAPS = ("IO4", "IO5", "IO8", "IO9", "IO15")   # ESP32-C6 strapping pins
ADC1 = ("IO0", "IO1", "IO2", "IO3", "IO4", "IO5", "IO6")


def netname(pin):
    n = u_mcu[pin].net
    return n.name if n is not None and n is not NC and n.pins else None


def design_checks():
    errors = []
    # 1. Isolation: every net is entirely mains or entirely SELV.
    for net in nets():
        if not net.pins or net is NC:
            continue
        sides = {(p.part.ref, p.num) in MAINS_PINS for p in net.pins}
        if len(sides) > 1:
            errors.append(f"ISOLATION: net {net.name} crosses the barrier ({', '.join(f'{p.part.ref}.{p.num}' for p in net.pins)})")
    # 2. Firmware pin map (firmware/esp32/src/main.rs).
    for pin, want in FIRMWARE_PINS.items():
        if netname(pin) != want:
            errors.append(f"PINMAP: {pin} is on {netname(pin)}, firmware expects {want}")
    # 3. Analog inputs on ADC1 (ADC2 doesn't exist on the C6; ADC1 works alongside Wi-Fi).
    for net in ("NTC", "P_ADC"):
        pin = next((p for p in FIRMWARE_PINS if FIRMWARE_PINS[p] == net), None)
        if pin not in ADC1:
            errors.append(f"ADC: {net} must be on an ADC1 pin")
    # 4. Straps: loads never on a strapping pin; only GPIO8 (pull-up) and GPIO9 (BOOT pad) connected.
    for pin in STRAPS:
        if netname(pin) and netname(pin).endswith("_CMD"):
            errors.append(f"BOOT-SAFE: {netname(pin)} is on strapping pin {pin}")
    for pin in ("IO4", "IO5", "IO15"):
        if netname(pin):
            errors.append(f"STRAP: {pin} must be left unconnected (is {netname(pin)})")
    if netname("IO8") != "IO8_STRAP" or not any(p.part.ref == "R11" for p in u_mcu["IO8"].net.pins):
        errors.append("STRAP: GPIO8 needs its pull-up")
    if errors:
        print("\n".join(errors))
        sys.exit(1)
    print("design checks: isolation OK, firmware pin map OK, analog on ADC1, straps OK")


def write_outputs():
    circ = builtins.default_circuit
    net_path = os.path.join(OUT, f"coffee-controller-{MAINS_V}v.net")
    generate_netlist(file_=net_path, tool=KICAD8)
    parts = {}
    for p in circ.parts:
        parts[p.ref] = dict(
            footprint=p.footprint, value=str(p.value),
            pins={pin.num: (pin.net.name if pin.net is not None and pin.net is not NC and pin.net.pins else None) for pin in p.pins},
            pin_names={pin.num: pin.name for pin in p.pins},
            mains=sorted(n for (r_, n) in MAINS_PINS if r_ == p.ref),
            populate=BOM_EXTRA.get(p.ref, {}).get("populate", "Y"),
            desc=p.description, mpn=BOM_EXTRA.get(p.ref, {}).get("mpn", ""), notes=BOM_EXTRA.get(p.ref, {}).get("notes", ""),
        )
    mains_nets = sorted({n.name for n in nets() if n.pins and n is not NC and all((p.part.ref, p.num) in MAINS_PINS for p in n.pins)})
    json.dump(dict(mains_v=MAINS_V, parts=parts, mains_nets=mains_nets, heater_nets=["HTR_A", "HTR_B"]),
              open(os.path.join(OUT, f"parts-{MAINS_V}v.json"), "w"), indent=1)
    refkey = lambda r_: (re.match(r"([A-Z]+)", r_).group(1), int(re.sub(r"\D", "", r_) or 0))
    rows = {}
    for p in circ.parts:
        extra = BOM_EXTRA.get(p.ref, {})
        key = (p.name, str(p.value), p.footprint, extra.get("mpn", ""), extra.get("populate", "Y"))
        rows.setdefault(key, {"refs": [], "notes": set(), "desc": p.description})
        rows[key]["refs"].append(p.ref)
        if extra.get("notes"):
            rows[key]["notes"].add(extra["notes"])
    bom_path = os.path.join(OUT, f"bom-{MAINS_V}v.csv")
    with open(bom_path, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Refs", "Qty", "Value", "Part / MPN", "Footprint", "Populate", "Description", "Notes"])
        for (name, value, fp, mpn, pop), d in sorted(rows.items(), key=lambda kv: refkey(sorted(kv[1]["refs"], key=refkey)[0])):
            refs = sorted(d["refs"], key=refkey)
            w.writerow([" ".join(refs), len(refs), value, mpn or name, fp, pop, d["desc"], " | ".join(sorted(d["notes"]))])
    print(f"wrote {net_path}\nwrote {bom_path}\n{len(circ.parts)} parts, {len([n for n in nets() if n.pins])} nets, {len(mains_nets)} mains nets")


if __name__ == "__main__":
    ERC()
    design_checks()
    write_outputs()
