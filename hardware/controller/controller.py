#!/usr/bin/env python3
"""coffee controller board v33: schematic as code (SKiDL).

The small loads' mains are on this board (v32+), behind a 7 mm isolation band:
- four 6.3 mm spades (N, L, P, 3), a 2 A fuse and a Mean Well IRM-10-5 that powers the board;
- the pump and the 3-way valve, each switched by a zero-cross opto-triac (MOC3083) and a triac;
- the heater (11 A) stays on an external zero-cross SSR, driven low-side from J2.
An ESP32-C6 (external-antenna module) reads a K-type thermocouple (MCP3421 + TMP117 on I2C), the
pressure transducer, the front switches and the scale.
Everything is SMD on one side except the IRM and the spades, from JLCPCB's basic library where one exists. Notes and
calculations: docs/CIRCUIT.md.

    python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
    .venv/bin/python controller.py                 # checks -> build/

Outputs in build/: KiCad netlist, BOM CSV, parts-120v.json (input to layout.py and jlc.py).

Pinouts: ESP32-C6-MINI-1U (Espressif datasheet / KiCad library, lib/Espressif.pretty),
TLV1117LV33 (SOT-223: 1 GND, 2 + tab VOUT, 3 VIN), AO3400A / AO3401A (SOT-23: 1 G, 2 S, 3 D).
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
OUT = os.environ.get("BUILD_DIR", os.path.join(HERE, "build"))   # v38: same override as layout.py
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
LED = tmpl("LED", "D", "LED_SMD:LED_0603_1608Metric", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "LED")
SCHOTTKY = tmpl("B5819W", "D", "Diode_SMD:D_SOD-123", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "Schottky 1 A 40 V")
SIGDIODE = tmpl("1N4148W", "D", "Diode_SMD:D_SOD-123", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "Switching diode 75 V 150 mA")
NMOS = tmpl("AO3400A", "Q", "Package_TO_SOT_SMD:SOT-23", [(1, "G", T.PASSIVE), (2, "S", T.PASSIVE), (3, "D", T.PASSIVE)], "N-MOSFET 30 V 5.7 A, 48 mOhm at 2.5 V gate")
PMOS = tmpl("AO3401A", "Q", "Package_TO_SOT_SMD:SOT-23", [(1, "G", T.PASSIVE), (2, "S", T.PASSIVE), (3, "D", T.PASSIVE)], "P-MOSFET 30 V 4 A, 60 mOhm at -4.5 V gate")
TESTPAD = tmpl("TestPad", "TP", "TestPoint:TestPoint_Pad_D1.0mm", [(1, "1", T.PASSIVE)], "Bare test pad")
HOLE = tmpl("MountingHole", "H", "MountingHole:MountingHole_3.2mm_M3", [], "M3 mounting hole (unplated)")


def conn(n, name, fp):
    return tmpl(name, "J", fp, [(i, str(i), T.PASSIVE) for i in range(1, n + 1)], name)


# Thermocouple front end (v31). MCP3421A0 (Microchip DS22003E, Table 3-1): 1 VIN+, 2 VSS, 3 SCL, 4 SDA,
# 5 VDD, 6 VIN-; I2C address 0x68. TMP117AIDRVR (TI SNOSD82D, Table 5-1, WSON): 1 SCL, 2 GND, 3 ALERT,
# 4 ADD0, 5 V+, 6 SDA; pad 7 is the thermal pad ("not connected to the device ground"; TI: unsoldered
# for best accuracy, or if soldered "floating or grounded"). Assembly solders it, so it goes to GND.
# ADD0 to GND = address 0x48.
ADC_TC = tmpl("MCP3421A0T-E/CH", "U", "Package_TO_SOT_SMD:SOT-23-6",
              [(1, "VIN+", T.INPUT), (2, "VSS", T.PWRIN), (3, "SCL", T.INPUT), (4, "SDA", T.BIDIR),
               (5, "VDD", T.PWRIN), (6, "VIN-", T.INPUT)],
              "18-bit delta-sigma ADC, I2C, 2.048 V reference, PGA x1-x8")
TSENS = tmpl("TMP117AIDRVR", "U", "Package_SON:WSON-6-1EP_2x2mm_P0.65mm_EP1x1.6mm",
             [(1, "SCL", T.INPUT), (2, "GND", T.PWRIN), (3, "ALERT", T.OPENCOLL), (4, "ADD0", T.INPUT),
              (5, "V+", T.PWRIN), (6, "SDA", T.BIDIR), (7, "EP", T.PASSIVE)],
             "+-0.1 C digital temperature sensor, I2C (thermocouple cold junction)")
# v32: the thermocouple's own wires are crimped into a GH 2-pin plug: the K-type alloys must reach the
# board itself, where the TMP117 measures the cold junction (a splice to copper anywhere else would be an
# unmeasured junction). The only 2-pin socket on the board, so no other lead fits it.
JST2 = None   # GH 2-pin, defined after conn()

# ---- v32 mains section templates (pinouts from the makers' datasheets; docs/evidence/parts.jsonl)
# Mean Well IRM-10-5 (KiCad symbol IRM-10-3.3/-5 and footprint IRM-10-xx: 1 AC/N, 2 AC/L, 3 -Vo, 4 +Vo;
# Mean Well drawing: AC and DC pin columns 38.5 mm apart, so it bridges the isolation band by itself).
PSU = tmpl("IRM-10-5", "PS", "Converter_ACDC:Converter_ACDC_MeanWell_IRM-10-xx_THT",
           [(1, "AC/N", T.PWRIN), (2, "AC/L", T.PWRIN), (3, "-Vo", T.PWROUT), (4, "+Vo", T.PWROUT)],
           "Isolated AC-DC 5 V 2 A, 4.2 kVac, UL/IEC 62368-1, EN 61558; 50 % load at 70 C")
# onsemi MOC3083SR2M (datasheet: 1 LED anode, 2 LED cathode, 3 NC, 4 main terminal, 5 substrate (do not
# connect), 6 main terminal; zero-cross, 800 V, IFT 5 mA max; external creepage and clearance >= 7 mm).
# KiCad's SMDIP-6_W9.53mm_Clearance8mm: 8.03 mm copper gap between the rows; plus a slot under the body.
OPTO = tmpl("MOC3083SR2M", "U", "Package_DIP:SMDIP-6_W9.53mm_Clearance8mm",
            [(1, "A", T.PASSIVE), (2, "K", T.PASSIVE), (3, "NC", T.NOCONNECT),
             (4, "MT1", T.PASSIVE), (5, "SUB", T.NOCONNECT), (6, "MT2", T.PASSIVE)],
            "Zero-cross opto-triac driver, 800 V, 4170 Vrms, IFT 5 mA max")
# ST T435-600B (DPAK; ST drawing: leads A1, (A2 cut), G; tab = A2). KiCad TO-252-2: 1 = left lead,
# 2 = tab, 3 = right lead -> 1 MT1 (A1), 2 MT2 (A2), 3 G.
TRIAC = tmpl("Triac_DPAK", "Q", "Package_TO_SOT_SMD:TO-252-2",
             [(1, "MT1", T.PASSIVE), (2, "MT2", T.PASSIVE), (3, "G", T.PASSIVE)],
             "Triac 4 A 600 V, ST T4 series (DPAK)")
CX2 = tmpl("C_X2", "C", "Coffee:C_2220_Murata_GA355", TWO, "Class X2 safety capacitor, 250 VAC, 2220")
FUSE = tmpl("Fuse", "F", "Coffee:Fuse_Littelfuse_Nano2_443", TWO, "Fuse, 2 A slow-blow, 250 VAC, 50 A breaking")
SPADE = tmpl("FASTON", "J", "Coffee:FASTON_TE_63824-1", [(1, "1", T.PASSIVE)], "6.3 mm (0.250\") PCB quick-connect tab")

# One connector type for every cable (one JLC extended-part fee): JST GH 3-pin (1.25 mm), SMD, side
# entry, positive latch (the vibration pump shakes the machine). 8.2 x 6.4 mm vs 11.2 x 10.2 for PH.
JST3 = conn(3, "JST_GH_3_SMD", "Connector_JST:JST_GH_SM03B-GHS-TB_1x03-1MP_P1.25mm_Horizontal")
# Front panel (3 switches + 3 LEDs + GND) and the drip-tray scale (dual HX711) need more pins.
# v35: every lead has its own size (2 thermocouple, 3 pressure, 4 heater SSR, 5 scale, 6 front panel), so no
# lead fits another's socket: the 7-pin front panel and scale (the last same-size pair) became 6 and 5.
JST6 = conn(6, "JST_GH_6_SMD", "Connector_JST:JST_GH_SM06B-GHS-TB_1x06-1MP_P1.25mm_Horizontal")
JST5 = conn(5, "JST_GH_5_SMD", "Connector_JST:JST_GH_SM05B-GHS-TB_1x05-1MP_P1.25mm_Horizontal")
JST2 = conn(2, "JST_GH_2_SMD", "Connector_JST:JST_GH_SM02B-GHS-TB_1x02-1MP_P1.25mm_Horizontal")
# v32 heater SSR: a 4-pin GH only so that its size is unique. No 3-pin pinout pair for it and the pressure
# socket is harmless both ways round (swap_check: either the SSR lead in J6 turns the heater on, or the
# transducer in J2 is back-fed through its protection diodes), so the two can't share a size.
JST4 = conn(4, "JST_GH_4_SMD", "Connector_JST:JST_GH_SM04B-GHS-TB_1x04-1MP_P1.25mm_Horizontal")


# TI TLV1117LV33DCYR (SBVS160C): 1 A, stable with >= 1 uF ceramic at any ESR, VIN <= 5.5 V (6 V abs),
# dropout 230 mV typ at 500 mA. SOT-223 because of heat: from 5 V it drops 1.7 V, ~0.17 W at the C6's
# ~100 mA average with Wi-Fi listening; RthJA 62.9 C/W -> +11 C. (The SOT-23-5 ME6211 it replaces is
# rated 300 mW, i.e. ~333 C/W -> +57 C: over Tj 125 C at 70 C ambient.)
LDO = tmpl("TLV1117LV33DCYR", "U", "Package_TO_SOT_SMD:SOT-223-3_TabPin2",
           [(1, "GND", T.PWRIN), (2, "VOUT", T.PWROUT), (3, "VIN", T.PWRIN)],
           "LDO 3.3 V 1 A, ceramic-stable")
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
PROGHDR = tmpl("Prog", "J", "Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical_SMD_Pin1Left",
               [(1, "VBUS", T.PASSIVE), (2, "D-", T.PASSIVE), (3, "D+", T.PASSIVE), (4, "GND", T.PASSIVE)],
               "Programming pads on the underside (nothing fitted): tack a USB lead on for the first flash")
USBC = tmpl("USB_C_Receptacle_USB2.0", "J", "Connector_USB:USB_C_Receptacle_HRO_TYPE-C-31-M-12",
            [("A1", "GND_A1", T.PASSIVE), ("A4", "VBUS_A4", T.PASSIVE), ("A5", "CC1", T.PASSIVE),
             ("A6", "DP1", T.PASSIVE), ("A7", "DN1", T.PASSIVE), ("A9", "VBUS_A9", T.PASSIVE), ("A12", "GND_A12", T.PASSIVE),
             ("B1", "GND_B1", T.PASSIVE), ("B4", "VBUS_B4", T.PASSIVE), ("B5", "CC2", T.PASSIVE), ("B6", "DP2", T.PASSIVE),
             ("B7", "DN2", T.PASSIVE), ("B9", "VBUS_B9", T.PASSIVE), ("B12", "GND_B12", T.PASSIVE), ("S1", "SHIELD", T.PASSIVE)],
            "USB-C receptacle (USB 2.0): flashing, logs, JTAG, bench power")

# --------------------------------------------------------------- nets
gnd, v5, v3, vbus = Net("GND"), Net("+5V"), Net("+3V3"), Net("VBUS")
for n in (gnd, v5, v3):
    n.drive = 100

# ================================================================ MAINS (v32)
# The board carries mains only for the small loads: the IRM-10-5 supply (~0.12 A), the pump (~0.45 A)
# and the 3-way valve (~0.1 A). The heater (11.4 A) stays on its external SSR, driven by J2 like before.
# Isolation: every mains part sits left of a copper-free band (layout plan "mains.band"); only the two
# opto-triacs and the IRM module bridge it, each over a milled slot. Spade order as Gaggiuino: N, L, P, 3.
ac_n, ac_l, ac_lf = Net("AC_N"), Net("AC_L"), Net("AC_L_F")
pump_sw, valve_sw = Net("PUMP_SW"), Net("VALVE_SW")
for _n in (ac_n, ac_l, ac_lf):
    _n.drive = 100     # mains sources (ERC)
for ref, net, what in (("J12", ac_n, "N: neutral (feeds the IRM module only)"), ("J13", ac_l, "L: live, unswitched (pump, valve, IRM)"),
                       ("J14", pump_sw, "P: switched live to the pump"), ("J15", valve_sw, "3: switched live to the 3-way valve")):
    j = part(SPADE, ref, what.split(":")[0], mpn="TE 63824-1", mains=True, notes=f"MAINS. {what}")
    j[1] += net
f1 = part(FUSE, "F1", "2A T", mpn="Littelfuse 0443002.DR", mains=True,
          notes="MAINS. 2 A slow-blow, 250 VAC, 50 A breaking: board live feed (load < 0.7 A; I2t 18 A2s rides the IRM inrush)")
f1[1, 2] += ac_l, ac_lf
# No MOV: the IRM passes 1 kV L-N surge (EN 61000-4-5) unaided, the triacs and optos are 600/800 V
# parts, and a worn MOV is a short the fuse would then have to clear (v14's reasoning, still holds).
ps1 = part(PSU, "PS1", "IRM-10-5", mpn="Mean Well IRM-10-5", mains=["1", "2"],
           notes="Isolated 5 V supply; bridges the isolation band (AC pins left, DC pins right). 0.45 A peak load vs 1 A derated at 70 C")
ps1["AC/N", "AC/L"] += ac_n, ac_lf
ps1["-Vo", "+Vo"] += gnd, v5


def triac_channel(name, n_u, n_q, r_led, r_6, r_sn, c_sn, out_net, triac, triac_note, r_gk=None, bleed=None):
    """Low-side FET -> opto LED (from +5V) -> MOC3083 zero-cross driver -> T435 triac; RC snubber across
    the triac (onsemi AN-3004 circuit; values: docs/evidence, sim/REPORT.md)."""
    a = Net(f"{name}_LED_A")
    r = part(R, r_led, "330", notes=f"{name} opto LED: (5 - 1.25) / 330 = 11 mA, 2x the 5 mA IFT max, under 50 mA")
    r[1, 2] += Net.get("+5V"), a
    u = part(OPTO, n_u, "MOC3083SR2M", mpn="onsemi MOC3083SR2M", mains=["4", "5", "6"],
             notes=f"{name}: isolation barrier (pins 1-3 SELV, 4-6 mains) over a slot. Fires only within 20 V of a zero crossing")
    u["A"] += a
    u["K"] += Net(f"{name}_OUT")      # the low-side FET's drain (OUTPUTS below joins it by name)
    u["NC"] += NC
    u["SUB"] += NC      # onsemi: pin 5 is the substrate, do not connect
    g, o6, sn = Net(f"{name}_G"), Net(f"{name}_O6"), Net(f"{name}_SNUB")
    q = part(TRIAC, n_q, triac, mpn=f"ST {triac}-TR", mains=True, notes=f"MAINS: {name}. {triac_note}")
    q["MT2"] += ac_lf
    q["MT1"] += out_net
    q["G"] += g
    u["MT1"] += g
    u["MT2"] += o6
    r = part(R, r_6, "330", footprint=FP_R_1206, mains=True,
             notes="MAINS: opto pin 6 to MT2: holds the opto's surge under its 1 A peak (onsemi AN-3004); 1206 for the 170 V peak")
    r[1, 2] += ac_lf, o6
    if r_gk:
        r = part(R, r_gk, "1k", footprint=FP_R_1206, mains=True,
                 notes="MAINS: gate-MT1 for the T410's 10 mA logic-level gate: dV/dt immunity (onsemi AN-3004: 330 R - 1k)")
        r[1, 2] += g, out_net
    r = part(R, r_sn, "100 1W", footprint=FP_R_2512, mpn="UNI-ROYAL 25121WF1000T4E", mains=True,
             notes="MAINS: snubber R (inductive load)")
    r[1, 2] += ac_lf, sn
    c = part(CX2, c_sn, "22nF X2", mpn="Murata GA355DR7GB223KW01L", mains=True,
             notes="MAINS: snubber C, Class X2 250 VAC (Gaggiuino uses an unrated MLCC here). ~1 mA off-state leakage")
    c[1, 2] += sn, out_net
    if bleed:
        # v33 (review M1): the pump's own series diode lets the snubber cap charge to the line peak and
        # then blocks its discharge, so ~170 V DC sits across the triac and the zero-cross opto (fires
        # only under its 20 V inhibit voltage) can never re-fire. 51k across the triac drains it (tau 1.1 ms).
        # Sized for the tight case, the pump running every cycle: the triac drops out late in the cycle and
        # leaves ~5 ms before the next positive zero crossing; 51k gets the triac to 1.8 V there (100k left
        # 13 V, over VINH's 12 V typical; re-review). Vishay CRCW2512: 500 V working (187 V peak at 132 V
        # line), 1 W (171 mW while the pump is off).
        r = part(R, bleed, "51k 500V", footprint=FP_R_2512, mpn="Vishay CRCW251251K0JNEG", mains=True,
                 notes="MAINS: pump bleeder across the triac: re-arms the zero-cross opto every cycle (review M1)")
        r[1, 2] += ac_lf, out_net
    return u, q


# Pump: T435 (IGT/IH 35 mA, 400 V/us, snubberless). The pump's own diode ends each stroke softly: 171-190 V
# at turn-off with the snubber (sim/REPORT.md). ~0.3-0.5 W: DPAK on its AC_L_F copper, ~90-100 C junction
# at 70 C ambient (Tj max 125 C).
triac_channel("PUMP", "U8", "Q5", "R47", "R49", "R51", "C15", pump_sw, "T435-600B",
              "Pump: IH 35 mA, 400 V/us. ~0.3-0.5 W on its AC_L_F copper (~90-100 C junction at 70 C ambient)",
              bleed="R54")
# Valve: T410 (IH 15 mA). The coil is still carrying current when the triac drops out at IH, and dumps
# it into the snubber: V ~ IH * sqrt(L/C). The T435's 35 mA gave 690 V (over its 600 V); the T410's 15 mA
# gives 456 V at 132 V line (76 %), dV/dt 23 V/us (its rating 40). sim/REPORT.md.
triac_channel("VALVE", "U9", "Q6", "R48", "R50", "R52", "C16", valve_sw, "T410-600B",
              "Valve: IH 15 mA (keeps the coil's turn-off kick to 456 V worst case), 40 V/us; ~0.1 W", r_gk="R53")

# ================================================================ POWER
# v32: the 5 V rail comes from the on-board Mean Well IRM-10-5 (MAINS section above). No external 5 V
# input any more, so no input connector and no reverse-polarity FET (v31's J1, Q4).

# ================================================================ CONTROLLER
u_mcu = part(ESP32C6, "U4", "ESP32-C6-MINI-1U-H4", mpn="ESP32-C6-MINI-1U-H4",
             notes="W.FL / MHF III antenna (not U.FL): pigtail to a 2.4 GHz antenna outside the steel case")
for pin in u_mcu.pins:
    if pin.name.startswith("GND"):
        pin += gnd
    elif pin.name.startswith("NC"):
        pin += NC
u_mcu["3V3"] += v3
for ref, val, fp in (("C3", "10uF 25V X5R", "Capacitor_SMD:C_0805_2012Metric"), ("C4", "100nF X7R", FP_C)):
    c = part(C, ref, val, footprint=fp, notes="3V3: C3 at the LDO output (>= 1 uF for stability), C4 at the module's top-left corner")
    c[1, 2] += v3, gnd
en = Net("EN")
u_mcu["EN"] += en
r = part(R, "R10", "10k", notes="EN pull-up (Espressif RC delay)")
r[1, 2] += v3, en
c = part(C, "C5", "1uF X5R", notes="EN delay")
c[1, 2] += en, gnd
# v31: GPIO8/9 are the I2C bus (thermocouple ADC + cold-junction sensor; a later board can bring it out). Both are
# strapping pins that must read high at reset (GPIO9 high = normal boot; GPIO8 high is what download mode
# needs), which is also an I2C bus's idle state: the bus pull-ups double as the strap pull-ups. Slaves only
# drive the bus during a transfer, so they can't hold a strap low at reset.
sda = Net("I2C_SDA")
u_mcu["IO8"] += sda
r = part(R, "R11", "4.7k", notes="I2C SDA pull-up; also GPIO8's strap pull-up (high at reset)")
r[1, 2] += v3, sda
scl = Net("I2C_SCL")
u_mcu["IO9"] += scl
r = part(R, "R46", "4.7k", notes="I2C SCL pull-up; also GPIO9 high at reset = normal boot")
r[1, 2] += v3, scl
tp = part(TESTPAD, "TP1", "BOOT", notes="Recovery only (USB Serial/JTAG resets into download mode by itself): short to GND (the USB-C shell) while plugging USB in. On SCL: harmless to the bus")
tp[1] += scl
# Status LED on U0TXD (GPIO16), v24: the ROM prints its boot log there and holds it high while it
# waits in download mode, so the LED flickers at every reset, glows while programming, then shows the
# firmware's heartbeat. (On the steam lamp, the same pin lit "steam ready" at every boot: review M1.)
st, st_a = Net("STATUS"), Net("STATUS_A")
u_mcu["TXD0"] += st
r = part(R, "R12", "1k", notes="red LED Vf ~2.0 V: ~1.3 mA from 3.3 V")
r[1, 2] += st, st_a
led = part(LED, "D1", "red", notes="Status: boot log flicker, on while programming, then the firmware heartbeat")
led["A"] += st_a
led["K"] += gnd

# Power: J1 -> Q4 -> +5V; USB VBUS joins through D2 (bench power, no back-feed into the PC).
d2 = part(SCHOTTKY, "D2", "B5819W", mpn="B5819W SL", notes="USB bench power")
d2["A"] += vbus
d2["K"] += v5
u_ldo = part(LDO, "U5", "TLV1117LV33DCYR", mpn="TI TLV1117LV33DCYR", notes="SOT-223 for heat: ~0.17 W average, +11 C")
u_ldo["VIN"] += v5
u_ldo["GND"] += gnd
u_ldo["VOUT"] += v3
for ref, val, net in (("C6", "22uF 25V X5R", v5), ("C7", "22uF 25V X5R", v3)):
    c = part(C, ref, val, footprint="Capacitor_SMD:C_0805_2012Metric", mpn="Samsung CL21A226MAQNNNE", notes="Ceramic bulk (X5R 25 V, 85 C: fine at the 60-70 C inside the machine; no electrolytics to age)")
    c[1, 2] += net, gnd

j_usb = part(USBC, "J4", "USB-C", mpn="HRO TYPE-C-31-M-12", notes="Flashing, logs, JTAG over the C6's USB Serial/JTAG; bench power")
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

# Outputs: GPIO -> AO3400A gate directly, 3.3k gate pull-down: off by hardware during reset, boot and
# flashing (under 0.5 V at the gate even against a 20k internal pull-up; sim/REPORT.md). Each FET sinks:
# - HEATER: the external heater SSR's input, on J2 (the only output that leaves the board, v32);
# - PUMP, VALVE: the LED of that channel's zero-cross opto-triac driver (MAINS section), from +5V
#   through R47/R48. The indicator LEDs D6-D8 sit on the gate lines (lit while the board drives them).
# GPIO order on the module edge (IO18, IO19, IO20) = valve, pump, heater.
# v32 J2 (4-pin GH, the only 4-pin socket): 1 = +5V (SSR 3 +), 2 = heater OUT (SSR 4 -), 3-4 empty.
j_out = part(JST4, "J2", "Heater SSR", mpn="JST SM04B-GHS-TB",
             notes="1=+5V (to SSR 3 +) 2=heater OUT (to SSR 4 -) 3-4=unused (4 pins only for a unique size)")
j_out[1] += v5
j_out[3] += NC
j_out[4] += NC
for name, gpio, qref, rpd in (("HEATER", "IO20", "Q1", "R4"), ("PUMP", "IO19", "Q2", "R5"), ("VALVE", "IO18", "Q3", "R6")):
    cmd, out = Net(f"{name}_CMD"), (Net.get(f"{name}_OUT") or Net(f"{name}_OUT"))
    u_mcu[gpio] += cmd
    r = part(R, rpd, "3.3k", notes="Gate pull-down: holds the gate < 0.5 V against a weak internal pull-up at reset (sim)")
    r[1, 2] += cmd, gnd
    q = part(NMOS, qref, "AO3400A", mpn="AO3400A",
             notes=f"{name} output: sinks " + ("the external heater SSR input (7-15 mA)" if name == "HEATER" else "the opto-triac LED (~9 mA)"))
    q["G"] += cmd
    q["S"] += gnd
    q["D"] += out
j_out[2] += Net.get("HEATER_OUT")

# Thermocouple (v31): ungrounded K-type, M4 thread, screwed into the brew-thermostat hole (Gaggiuino's
# probe). Its two wires clamp into J5, a screw terminal (v38; was a GH 2-pin). TC- is the board's ground reference (the
# probe's junction is insulated from the earthed boiler, so this is the only ground on it).
# - R43/R44 1k in each ADC input + C12: the noise filter. The 0.5 m probe lead runs past the heater and
#   pump wiring, and the SSRs switch 11 A beside it. (No lead can reach J5, so they aren't there for
#   mis-plugs; they do also keep any surge on the probe wires inside the MCP3421's +-2 mA input limit.)
#   Cost: the ADC's input impedance (2.25 MOhm / PGA, typ) loads them, a fixed ~0.7 % gain error at
#   PGA 8 that the firmware corrects (~0.35 C at a 50 C tip-to-board rise).
# - C12 1 uF across the inputs: ~80 Hz corner with 2 x 1k, ahead of the ADC's own sinc filter at 3.75 SPS.
# - R45 10 MOhm from 3V3 to TC+ on the connector side: open probe -> the 10M works against the ADC's
#   input impedance (~281k at PGA 8, plus R43/R44): ~90 mV across the inputs, settling in ~0.3 s with C12.
#   No K-type makes 90 mV below ~2000 C, so the firmware calls anything over ~15 mV (~370 C) an open probe.
#   With the probe in, its few ohms carry 0.33 uA: a few uV (~0.1 C), constant, calibrated out. (On the
#   ADC side of R43 the same current would drop 0.33 mV: ~8 C.)
# - TMP117 (U7) sits at J5's pads: it reads the cold junction (the alloy-to-copper joints in the terminal).
tc_p = Net("TC_P")
# v38: a screw terminal (was a GH 2-pin): the probe's own alloy wires clamp straight in, nothing to crimp,
# and no copper lead between the alloy and the cold junction U7 reads.
TERM2 = tmpl("Screw terminal 2P", "J", "TerminalBlock_Phoenix:TerminalBlock_Phoenix_MKDS-1,5-2-5.08_1x02_P5.08mm_Horizontal",
             [(1, "1", T.PASSIVE), (2, "2", T.PASSIVE)], "Phoenix MKDS 1,5/2-5,08 screw terminal, 0.14-1.5 mm2")
j_tc = part(TERM2, "J5", "Thermocouple", mpn="Phoenix MKDS 1,5/ 2-5,08",
            notes="1=TC+ (chromel: non-magnetic; yellow ANSI, green IEC) 2=TC- (alumel: the magnetic wire; red ANSI, white IEC); the probe's bare wires clamp straight in. Cold junction measured by U7 beside it")
j_tc[1, 2] += tc_p, gnd   # TC- is the reference, on GND at the socket (a named TC_N net would rename GND)
tc_inp, tc_inn = Net("TC_INP"), Net("TC_INN")
r = part(R, "R43", "1k", notes="TC+ input: noise filter with C12 (probe lead runs past the heater wiring)")
r[1, 2] += tc_p, tc_inp
r = part(R, "R44", "1k", notes="TC- input series: matches R43 (symmetric filter)")
r[1, 2] += gnd, tc_inn
c = part(C, "C12", "1uF X5R", notes="Differential filter across the ADC inputs: ~80 Hz with R43/R44")
c[1, 2] += tc_inp, tc_inn
r = part(R, "R45", "10M", notes="Open-probe bias on the connector side of R43: open -> ADC over range -> fault; probe in: ~3 uV, calibrated out")
r[1, 2] += v3, tc_p
u_tc = part(ADC_TC, "U6", "MCP3421A0T-E/CH", mpn="MCP3421A0T-E/CH", notes="Thermocouple ADC, I2C 0x68: PGA 8, 18 bit, 3.75 SPS (15.6 uV/8 = 2 uV per LSB, 0.05 C)")
u_tc["VIN+", "VIN-"] += tc_inp, tc_inn
u_tc["VSS", "VDD"] += gnd, v3
u_tc["SCL", "SDA"] += scl, sda
c = part(C, "C13", "100nF X7R", notes="MCP3421 VDD bypass")
c[1, 2] += v3, gnd
u_cj = part(TSENS, "U7", "TMP117AIDRVR", mpn="TI TMP117AIDRVR",
            notes="Cold junction: at J5's pads, I2C 0x48 (ADD0 to GND), +-0.2 C max to 100 C; thermal pad soldered to GND (TI allows floating or grounded)")
u_cj["GND", "ADD0"] += gnd, gnd
u_cj["V+"] += v3
u_cj["SCL", "SDA"] += scl, sda
u_cj["ALERT"] += NC
u_cj["EP"] += gnd   # TI: a soldered thermal pad "must be left floating or grounded"; JLC solders it, and
                    # grounding it joins the two GND pins under the part (they can't be routed round it)
c = part(C, "C14", "100nF X7R", notes="TMP117 V+ bypass")
c[1, 2] += v3, gnd

# Pressure: 0.5-4.5 V ratiometric transducer on +5V, divided by 2/3 (4.5 V -> 3.0 V) into ADC1.
p_sig, p_adc = Net("P_SIG"), Net("P_ADC")
j_p = part(JST3, "J6", "Pressure", mpn="JST SM03B-GHS-TB", notes="0.5-4.5 V ratiometric 5 V transducer (Gaggiuino 0-1.2 MPa; BUY.md 7). 1=SIG 2=+5V 3=GND (v29: +5V in the middle, so the supply lead in this socket just powers the board)")
j_p[1, 2, 3] += p_sig, v5, gnd
# 147 kOhm total: the TE M3200 datasheet asks for a load > 100 kOhm on its voltage output. Gain 100/147:
# 4.5 V -> 3.06 V. (300k would leave too little charge in the reservoir for the ADC's sampling cap.)
r = part(R, "R16", "47k", notes="Divider 100/147 (147k load: TE M3200 needs > 100k)")
r[1, 2] += p_sig, p_adc
r = part(R, "R17", "100k")
r[1, 2] += p_adc, gnd
c = part(C, "C9", "22nF X7R", notes="ADC reservoir; ~226 Hz with the 32k divider: keeps the pump's 60 Hz pulses")
c[1, 2] += p_adc, gnd
u_mcu["IO0"] += p_adc   # ADC1_CH0

# Front panel: the brew and steam rockers as dry contacts to GND, and the three lamps replaced by LEDs the
# firmware drives (power, coffee-ready, steam-ready). v35: the power rocker is stock again (it switches the
# machine's mains, the board included: off is off), so it isn't an input any more.
j_fp = part(JST6, "J7", "Front panel", mpn="JST SM06B-GHS-TB",
            notes="1=BREW 2=STEAM 3=GND 4=LED power 5=LED coffee 6=LED steam (LED cathodes to GND)")
# 1k series from the connector, then a 10k pull-up on the GPIO side (lets each switch's 1k sit alone at
# its pin and the pull-ups next to the module). Closed: 3.3 x 1k / 11k = 0.30 V at the GPIO (0.33 V with
# the internal pull-up too), 0.5 V under the C6's 0.825 V VIL (4.7k left only ~0.2 V: review S2);
# ~0.3 mA wetting current. A 5 V plug in the wrong socket pushes ~1.5 mA through the 1k into the clamp.
for i, (name, gpio, rpu, rs) in enumerate((("BREW", "IO6", "R20", "R18"), ("STEAM", "IO7", "R21", "R19"))):
    wire, pin = Net(f"SW_{name}_IN"), Net(f"SW_{name}")
    j_fp[i + 1] += wire
    r = part(R, rpu, "10k", notes="switch pull-up / wetting current")
    r[1, 2] += v3, pin
    r = part(R, rs, "1k", notes="GPIO series protection")
    r[1, 2] += wire, pin
    u_mcu[gpio] += pin
j_fp[3] += gnd
# LEDs: GPIO -> 330R -> lead -> pre-wired LED (its own 150-470R in the lead) -> GND: (3.3 - 2.0) /
# (330 + 150..470) = ~1.6-2.7 mA, visible behind the lamp lens.
for i, (name, gpio, rl) in enumerate((("PWR", "IO21", "R30"), ("COFFEE", "IO22", "R31"), ("STEAM", "IO23", "R32"))):
    led = Net(f"LED_{name}")
    r = part(R, rl, "330", notes=f"front-panel LED ({name.lower()}) current limit (plus the LED's own in its lead): ~2 mA")
    r[1, 2] += Net(f"LED_{name}_IO"), led
    u_mcu[gpio] += r[1].net
    j_fp[4 + i] += led

# v27 indicator panel: a red LED + 1k on each front-panel lamp GPIO (the same level the lamp gets) and on
# each output's gate drive (lit exactly while the board turns that SSR on), in a row with the status LED.
# 1k from 3.3 V: ~1.3 mA each. On the gate lines it only adds load to the 3.3k pull-down at boot.
for dref, rref, net, what in (("D3", "R37", "LED_PWR_IO", "power lamp"), ("D4", "R38", "LED_COFFEE_IO", "coffee lamp"),
                              ("D5", "R39", "LED_STEAM_IO", "steam lamp"), ("D6", "R40", "HEATER_CMD", "heater on"),
                              ("D7", "R41", "PUMP_CMD", "pump on"), ("D8", "R42", "VALVE_CMD", "valve on")):
    a_net = Net(f"{net}_IND")
    r = part(R, rref, "1k", notes=f"indicator LED: {what}")
    r[1, 2] += Net.get(net), a_net
    led = part(LED, dref, "red", notes=f"indicator: {what}")
    led["A"] += a_net
    led["K"] += gnd

# Drip-tray scale: two load cells with a dual HX711 board beside them in the tray base (Gaggiuino's
# layout), so only slow digital lines run here. v30: 3V3 only (VCC and VDD linked at the HX711 board;
# DOUT swings 0-3.3 V). No series resistors (see below); SCK pulled down so the HX711 stays awake
# while the C6 boots (SCK held high > 60 us powers it down). GPIO4/5 are strapping pins only for
# SDIO clock edges (C6 datasheet, "Strapping pins"), unused here, so they serve as the DOUT inputs.
j_sc = part(JST5, "J10", "Scale", mpn="JST SM05B-GHS-TB",
            notes="1=DOUT1 2=SCK 3=GND 4=DOUT2 5=+3V3 (VCC+VDD tied at the HX711 board; dual HX711 board in the tray base)")
# v30: the scale runs from 3V3 only (HX711 VSUP 2.6-5.5 V). v35: a 5-pin of its own (no empty pins).
j_sc[3, 5] += gnd, v3
# v23: no series resistors on the scale lines (GaggiMate and Gaggiuino have none). J10 sits over the
# module's DOUT pins with no room between, and SCK runs under the module; the C6's pins have their own
# ESD diodes, and the HX711s sit at the other end of a 30 cm lead at the same ground.
for pin_no, (name, gpio) in ((2, ("SCK", "RXD0")), (1, ("DOUT1", "IO4")), (4, ("DOUT2", "IO5"))):
    d = Net(f"SCALE_{name}")
    u_mcu[gpio] += d
    j_sc[pin_no] += d
r = part(R, "R36", "10k", notes="SCK pull-down: HX711 stays powered while the C6 boots")
r[1, 2] += u_mcu["RXD0"], gnd      # the GPIO side of SCK (Net("SCALE_SCK") here made a second, unconnected net: v19-v22 bug)

# Rail sense: the transducer is ratiometric to +5V but the ADC measures absolute volts, so the firmware
# divides by the measured rail (the IRM module's output drifts with load and temperature). v32: the 3V3
# sense is gone (it served the thermistor; the thermocouple ADC has its own reference, the C6 its own
# brownout detector), GPIO2 is spare.
for net, rail, gpio, rt_, rb_, vt, cref in (("V5_SNS", v5, "IO3", "R24", "R25", "20k", "C11"),):
    sns = Net(net)
    r = part(R, rt_, vt, notes=f"{net} divider top")
    r[1, 2] += rail, sns
    r = part(R, rb_, "10k", notes=f"{net} divider bottom")
    r[1, 2] += sns, gnd
    c = part(C, cref, "100nF X7R", notes="ADC reservoir")
    c[1, 2] += sns, gnd
    u_mcu[gpio] += sns

u_mcu["IO15"] += NC
u_mcu["IO2"] += NC    # v32: spare (the 3V3 rail sense is gone)
u_mcu["IO1"] += NC    # v31: the thermistor input is gone (thermocouple on I2C); spare ADC1 pin
u_mcu["IO14"] += NC   # v35: spare (the power rocker is a mains switch again, not an input)
# v35: no mounting holes. The IRM sits on the underside and drops into a pocket in the printed housing,
# which locates the board; the housing's floor under the mains half takes the receptacles' push force.


# ================================================================ checks + outputs
def nets():
    return builtins.default_circuit.nets


FIRMWARE_PINS = {"IO0": "P_ADC", "IO3": "V5_SNS", "IO18": "VALVE_CMD", "IO19": "PUMP_CMD", "IO20": "HEATER_CMD",
                 "IO6": "SW_BREW", "IO7": "SW_STEAM", "IO23": "LED_STEAM_IO", "IO12": "USB_D-", "IO13": "USB_D+",
                 "IO21": "LED_PWR_IO", "IO22": "LED_COFFEE_IO", "TXD0": "STATUS", "RXD0": "SCALE_SCK",
                 "IO4": "SCALE_DOUT1", "IO5": "SCALE_DOUT2",
                 "IO8": "I2C_SDA", "IO9": "I2C_SCL"}
STRAPS = ("IO4", "IO5", "IO8", "IO9", "IO15")   # ESP32-C6 strapping pins
ADC1 = ("IO0", "IO1", "IO2", "IO3", "IO4", "IO5", "IO6")


def netname(pin):
    n = u_mcu[pin].net
    return n.name if n is not None and n is not NC and n.pins else None


MAINS_REFS = {"J12", "J13", "J14", "J15", "F1", "PS1", "U8", "U9", "Q5", "Q6", "R49", "R50", "R51", "R52", "R53", "R54", "C15", "C16"}
THT_OK = {"PS1", "J12", "J13", "J14", "J15", "J5"}   # the IRM module, the spade tabs and (v38) the thermocouple screw terminal: JLC THT or hand-soldered


def design_checks():
    errors = []
    # 1. v32 mains section. (a) No net joins a mains pin to a SELV pin: mains and SELV meet only inside
    #    a bridging part (the opto-triacs, the IRM module). (b) Every mains part is a known one.
    #    (c) Single-sided SMD, except the hand-solderable (or JLC THT) mains parts listed in THT_OK.
    for n in nets():
        pins = [(p.part.ref, p.num) for p in n.pins]
        if not pins or n is NC:
            continue
        m = [x for x in pins if x in MAINS_PINS]
        if m and len(m) != len(pins):
            errors.append(f"ISOLATION: net {n.name} joins mains pins {m} to SELV pins {[x for x in pins if x not in MAINS_PINS]}")
    for ref in sorted({r for r, _ in MAINS_PINS}):
        if ref not in MAINS_REFS:
            errors.append(f"MAINS: {ref} carries mains but isn't in MAINS_REFS")
    for p in builtins.default_circuit.parts:
        tht = any(k in p.footprint for k in ("THT", "TO-220", "TerminalBlock", "FASTON")) or p.footprint.startswith("Package_DIP:DIP")
        if tht and p.ref not in THT_OK and BOM_EXTRA.get(p.ref, {}).get("populate", "Y") == "Y":
            errors.append(f"SMD: {p.ref} ({p.footprint}) is through-hole")
    # 2. Firmware pin map (firmware/esp32/src/main.rs).
    for pin, want in FIRMWARE_PINS.items():
        if netname(pin) != want:
            errors.append(f"PINMAP: {pin} is on {netname(pin)}, firmware expects {want}")
    # 3. Analog inputs on ADC1 (ADC2 doesn't exist on the C6; ADC1 works alongside Wi-Fi).
    for net in ("P_ADC",):
        pin = next((p for p in FIRMWARE_PINS if FIRMWARE_PINS[p] == net), None)
        if pin not in ADC1:
            errors.append(f"ADC: {net} must be on an ADC1 pin")
    # 4. Straps: loads never on a strapping pin; GPIO8/9 carry only the I2C bus (high at reset) and the BOOT pad.
    for pin in STRAPS:
        if netname(pin) and netname(pin).endswith("_CMD"):
            errors.append(f"BOOT-SAFE: {netname(pin)} is on strapping pin {pin}")
    # GPIO4/5 (MTMS/MTDI) strap only SDIO clock edges: allowed as the scale's data *inputs*, nothing else.
    for pin, ok in (("IO4", "SCALE_DOUT1"), ("IO5", "SCALE_DOUT2"), ("IO15", None)):
        if netname(pin) != ok:
            errors.append(f"STRAP: {pin} must be {ok or 'unconnected'} (is {netname(pin)})")
    for pin, ref in (("IO8", "R11"), ("IO9", "R46")):   # v31: I2C pull-ups are the strap pull-ups
        if not any(p.part.ref == ref for p in u_mcu[pin].net.pins):
            errors.append(f"STRAP: {pin} needs its pull-up {ref}")
    if errors:
        print("\n".join(errors))
        sys.exit(1)
    print("design checks: mains/SELV only meet inside the bridging parts, mains parts known, SMD except the "
          "listed THT, firmware pin map OK, analog on ADC1, straps OK")


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
    json.dump(dict(mains_v=MAINS_V, parts=parts, mains_nets=mains_nets, heater_nets=[]),
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
    # every lead in every same-size socket must be harmless (docs/SWAP-SAFETY.md): fail the build if not
    import subprocess
    if subprocess.run([sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), "swap_check.py")]).returncode:
        sys.exit("swap_check.py: a mis-plugged lead is hazardous")
