#!/usr/bin/env python3
"""coffee controller board v7 (low voltage only): schematic as code (SKiDL).

No mains on this board. Certified modules mounted beside it in the machine do the mains work:
- a 5 V AC-DC module feeds J1;
- the heater and the pump are switched by zero-cross SSRs, the 3-way valve by a relay module;
  the board sinks each module's input through a small MOSFET (J2 heater, J3 pump, J8 valve).
- An ESP32-C6 (external-antenna module) reads the thermistor, the pressure transducer and the
  front switches on its own ADC and GPIOs.
Everything is SMD on one side, from JLCPCB's basic library where one exists. Notes and
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
LED = tmpl("LED", "D", "LED_SMD:LED_0603_1608Metric", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "LED")
SCHOTTKY = tmpl("B5819W", "D", "Diode_SMD:D_SOD-123", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "Schottky 1 A 40 V")
SIGDIODE = tmpl("1N4148W", "D", "Diode_SMD:D_SOD-123", [(1, "K", T.PASSIVE), (2, "A", T.PASSIVE)], "Switching diode 75 V 150 mA")
NMOS = tmpl("AO3400A", "Q", "Package_TO_SOT_SMD:SOT-23", [(1, "G", T.PASSIVE), (2, "S", T.PASSIVE), (3, "D", T.PASSIVE)], "N-MOSFET 30 V 5.7 A, 48 mOhm at 2.5 V gate")
PMOS = tmpl("AO3401A", "Q", "Package_TO_SOT_SMD:SOT-23", [(1, "G", T.PASSIVE), (2, "S", T.PASSIVE), (3, "D", T.PASSIVE)], "P-MOSFET 30 V 4 A, 60 mOhm at -4.5 V gate")
TESTPAD = tmpl("TestPad", "TP", "TestPoint:TestPoint_Pad_D1.0mm", [(1, "1", T.PASSIVE)], "Bare test pad")
HOLE = tmpl("MountingHole", "H", "MountingHole:MountingHole_3.2mm_M3", [], "M3 mounting hole (unplated)")


def conn(n, name, fp):
    return tmpl(name, "J", fp, [(i, str(i), T.PASSIVE) for i in range(1, n + 1)], name)


# One connector type for every cable (one JLC extended-part fee): JST GH 3-pin (1.25 mm), SMD, side
# entry, positive latch (the vibration pump shakes the machine). 8.2 x 6.4 mm vs 11.2 x 10.2 for PH.
JST2 = conn(2, "JST_GH_2_SMD", "Connector_JST:JST_GH_SM02B-GHS-TB_1x02-1MP_P1.25mm_Horizontal")
JST3 = conn(3, "JST_GH_3_SMD", "Connector_JST:JST_GH_SM03B-GHS-TB_1x03-1MP_P1.25mm_Horizontal")
# Front panel (3 switches + 3 LEDs + GND) and the drip-tray scale (dual HX711) need more pins.
JST7 = conn(7, "JST_GH_7_SMD", "Connector_JST:JST_GH_SM07B-GHS-TB_1x07-1MP_P1.25mm_Horizontal")
JST6 = conn(6, "JST_GH_6_SMD", "Connector_JST:JST_GH_SM06B-GHS-TB_1x06-1MP_P1.25mm_Horizontal")

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

# ================================================================ POWER IN
# 5 V from the external AC-DC module. Reverse-polarity protection by a P-MOSFET (drain to the
# connector, source to the rail, gate to GND): ~0.3 V less drop than a diode, so the transducer
# keeps its full 5 V. A swapped lead leaves the body diode reverse biased and the gate at 0 V: off.
v5_in = Net("+5V_IN")
# v24: a 2-pin socket, unlike the 3-pin outputs: the supply lead can't go into J2/J3/J8 and an output
# lead can't go into J1 (with a 3-pin J1, that swap back-fed +5V through Q4 and held a load on; review S1).
j_pwr = part(JST2, "J1", "5V in", mpn="JST SM02B-GHS-TB", notes="1=+5V 2=GND, from the 5 V AC-DC module")
j_pwr[1, 2] += v5_in, gnd
q_rp = part(PMOS, "Q4", "AO3401A", mpn="AO3401A", notes="Reverse-polarity protection")
q_rp["D"] += v5_in
q_rp["S"] += v5
q_rp["G"] += gnd      # gate straight to GND: Vgs = -5 V, inside the AO3401A's +-12 V (v23 dropped the 10k R26)

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
strap8 = Net("IO8_STRAP")
u_mcu["IO8"] += strap8
r = part(R, "R11", "10k", notes="GPIO8 high: required for download mode from the BOOT pad")
r[1, 2] += v3, strap8
boot = Net("IO9_BOOT")
u_mcu["IO9"] += boot   # internal pull-up: normal boot
tp = part(TESTPAD, "TP1", "BOOT", notes="Recovery only (USB Serial/JTAG resets into download mode by itself): short to GND (the USB-C shell) while plugging USB in")
tp[1] += boot
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

# Outputs: GPIO -> AO3400A gate directly, 3.3k gate pull-down. The FET sinks the module's input:
# connector 1 = +5V, 2 = switched low (OUT), 3 = GND. Off by hardware during reset, boot and flashing
# (3.3k pull-down: under 0.5 V at the gate even against a 20k internal pull-up; sim/REPORT.md).
# Every module has its own input LED, so the board has none per output. All three outputs drive SSR
# boards (no coil), so there's no flyback diode (v23 dropped D5; a relay module brings its own).
# GPIO order on the module edge (IO18, IO19, IO20) matches the connector order (valve, pump, heater).
for name, gpio, jref, qref, rpd, dfb in (("HEATER", "IO20", "J2", "Q1", "R4", None), ("PUMP", "IO19", "J3", "Q2", "R5", None),
                                         ("VALVE", "IO18", "J8", "Q3", "R6", None)):
    cmd, out = Net(f"{name}_CMD"), Net(f"{name}_OUT")
    u_mcu[gpio] += cmd
    r = part(R, rpd, "3.3k", notes="Gate pull-down: holds the gate < 0.5 V against a weak internal pull-up at reset (sim)")
    r[1, 2] += cmd, gnd
    q = part(NMOS, qref, "AO3400A", mpn="AO3400A", notes=f"{name} output: sinks the module input (SSR 7-15 mA, relay coil < 100 mA)")
    q["G"] += cmd
    q["S"] += gnd
    q["D"] += out
    j = part(JST3, jref, name.capitalize(), mpn="JST SM03B-GHS-TB", notes=f"{name}: 1=+5V 2=OUT (low = on) 3=GND. To the module input + / -")
    j[1, 2, 3] += v5, out, gnd
    if dfb:
        d = part(SIGDIODE, dfb, "1N4148W", mpn="1N4148W", notes="Flyback for the relay coil")
        d["A"] += out
        d["K"] += v5

# Thermistor: Molex 218537-3919 50k NTC ring-tab probe to GND, 4.7k reference to 3V3: ~1.5 V at 93 C
# (the reference sits near the probe's ~3.9k there, for the most mV per degree).
# R27 1k between the node and GPIO1: every cable uses the same GH 3-pin plug, and the 5 V lead in J5
# would otherwise put the supply straight onto the pin's clamp diode (~1 mA through 1k). No current
# flows in R27 in normal use, so it adds no measurement error.
ntc, ntc_adc = Net("NTC"), Net("NTC_ADC")
j_ntc = part(JST3, "J5", "Thermistor", mpn="JST SM03B-GHS-TB", notes="Molex 218537-3919 50k NTC ring-tab under an M4 screw at the brew-thermostat seat. 1=NTC 2=GND (no polarity), 3 unused")
j_ntc[1, 2] += ntc, gnd
j_ntc[3] += NC
r = part(R, "R15", "4.7k", notes="NTC reference: 3.02 V at 25 C, 1.50 V at 93 C, 0.59 V at 140 C (Molex 218537-3919 table)")
r[1, 2] += v3, ntc
c = part(C, "C8", "100nF X7R", notes="ADC sampling reservoir / noise filter")
c[1, 2] += ntc, gnd
r = part(R, "R27", "1k", notes="GPIO1 series protection: a 5 V plug in the wrong socket pushes ~1 mA into the clamp diode")
r[1, 2] += ntc, ntc_adc
u_mcu["IO1"] += ntc_adc   # ADC1_CH1

# Pressure: 0.5-4.5 V ratiometric transducer on +5V, divided by 2/3 (4.5 V -> 3.0 V) into ADC1.
p_sig, p_adc = Net("P_SIG"), Net("P_ADC")
j_p = part(JST3, "J6", "Pressure", mpn="JST SM03B-GHS-TB", notes="0.5-4.5 V ratiometric 5 V transducer (GaggiMate 0-1.6 MPa; BUY.md 10b). 1=+5V 2=SIG 3=GND")
j_p[1, 2, 3] += v5, p_sig, gnd
# 147 kOhm total: the TE M3200 datasheet asks for a load > 100 kOhm on its voltage output. Gain 100/147:
# 4.5 V -> 3.06 V. (300k would leave too little charge in the reservoir for the ADC's sampling cap.)
r = part(R, "R16", "47k", notes="Divider 100/147 (147k load: TE M3200 needs > 100k)")
r[1, 2] += p_sig, p_adc
r = part(R, "R17", "100k")
r[1, 2] += p_adc, gnd
c = part(C, "C9", "22nF X7R", notes="ADC reservoir; ~226 Hz with the 32k divider: keeps the pump's 60 Hz pulses")
c[1, 2] += p_adc, gnd
u_mcu["IO0"] += p_adc   # ADC1_CH0

# Front panel: the machine's three latching rockers (power, brew, steam) as dry contacts to GND, and
# its three lamps replaced by LEDs the firmware drives (power, coffee-ready, steam-ready). The power
# switch no longer switches mains: the controller is always on, and the switch means "on / standby".
j_fp = part(JST7, "J7", "Front panel", mpn="JST SM07B-GHS-TB",
            notes="1=POWER 2=BREW 3=STEAM 4=GND 5=LED power 6=LED coffee 7=LED steam (LED cathodes to GND)")
# 1k series from the connector, then a 10k pull-up on the GPIO side (lets each switch's 1k sit alone at
# its pin and the pull-ups next to the module). Closed: 3.3 x 1k / 11k = 0.30 V at the GPIO (0.33 V with
# the internal pull-up too), 0.5 V under the C6's 0.825 V VIL (4.7k left only ~0.2 V: review S2);
# ~0.3 mA wetting current. A 5 V plug in the wrong socket pushes ~1.5 mA through the 1k into the clamp.
for i, (name, gpio, rpu, rs) in enumerate((("POWER", "IO14", "R28", "R29"), ("BREW", "IO6", "R20", "R18"), ("STEAM", "IO7", "R21", "R19"))):
    wire, pin = Net(f"SW_{name}_IN"), Net(f"SW_{name}")
    j_fp[i + 1] += wire
    r = part(R, rpu, "10k", notes="switch pull-up / wetting current")
    r[1, 2] += v3, pin
    r = part(R, rs, "1k", notes="GPIO series protection")
    r[1, 2] += wire, pin
    u_mcu[gpio] += pin
j_fp[4] += gnd
# LEDs: GPIO -> 330R -> LED anode at the panel, cathode to GND: (3.3 - 2.0) / 330 = ~4 mA, bright
# enough behind the lamp lens, well within the C6's GPIO drive.
for i, (name, gpio, rl) in enumerate((("PWR", "IO21", "R30"), ("COFFEE", "IO22", "R31"), ("STEAM", "IO23", "R32"))):
    led = Net(f"LED_{name}")
    r = part(R, rl, "330", notes=f"front-panel LED ({name.lower()}) current limit, ~4 mA")
    r[1, 2] += Net(f"LED_{name}_IO"), led
    u_mcu[gpio] += r[1].net
    j_fp[5 + i] += led

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
# layout), so only slow digital lines run here. 5 V for the HX711's analog supply, 3V3 for its logic
# (DOUT then swings 0-3.3 V). 100R series on every line; SCK pulled down so the HX711 stays awake
# while the C6 boots (SCK held high > 60 us powers it down). GPIO4/5 are strapping pins only for
# SDIO clock edges (C6 datasheet, "Strapping pins"), unused here, so they serve as the DOUT inputs.
j_sc = part(JST6, "J10", "Scale", mpn="JST SM06B-GHS-TB",
            notes="1=+5V 2=+3V3 3=GND 4=SCK 5=DOUT1 6=DOUT2 (dual HX711 board in the tray base)")
j_sc[1, 2, 3] += v5, v3, gnd
# v23: no series resistors on the scale lines (GaggiMate and Gaggiuino have none). J10 sits over the
# module's DOUT pins with no room between, and SCK runs under the module; the C6's pins have their own
# ESD diodes, and the HX711s sit at the other end of a 30 cm lead at the same ground.
for pin_no, (name, gpio) in ((4, ("SCK", "RXD0")), (5, ("DOUT1", "IO4")), (6, ("DOUT2", "IO5"))):
    d = Net(f"SCALE_{name}")
    u_mcu[gpio] += d
    j_sc[pin_no] += d
r = part(R, "R36", "10k", notes="SCK pull-down: HX711 stays powered while the C6 boots")
r[1, 2] += u_mcu["RXD0"], gnd      # the GPIO side of R33 (Net("SCALE_SCK") here made a second, unconnected net: v19-v22 bug)

# Rail sense: the thermistor divider is ratiometric to 3V3 and the transducer to +5V, but the ADC
# measures absolute volts. Measuring both rails lets the firmware divide them out, so an open
# thermistor reads exactly full scale (3300 mV) whatever the rail, and 0 C stays well clear of it.
for net, rail, gpio, rt_, rb_, vt, cref in (("V3_SNS", v3, "IO2", "R22", "R23", "10k", "C10"), ("V5_SNS", v5, "IO3", "R24", "R25", "20k", "C11")):
    sns = Net(net)
    r = part(R, rt_, vt, notes=f"{net} divider top")
    r[1, 2] += rail, sns
    r = part(R, rb_, "10k", notes=f"{net} divider bottom")
    r[1, 2] += sns, gnd
    c = part(C, cref, "100nF X7R", notes="ADC reservoir")
    c[1, 2] += sns, gnd
    u_mcu[gpio] += sns

u_mcu["IO15"] += NC
# No mounting holes (each costs ~50 mm2 of board): the printed holder grips the board's edges.


# ================================================================ checks + outputs
def nets():
    return builtins.default_circuit.nets


FIRMWARE_PINS = {"IO0": "P_ADC", "IO1": "NTC_ADC", "IO2": "V3_SNS", "IO3": "V5_SNS", "IO18": "VALVE_CMD", "IO19": "PUMP_CMD", "IO20": "HEATER_CMD",
                 "IO6": "SW_BREW", "IO7": "SW_STEAM", "IO14": "SW_POWER", "IO23": "LED_STEAM_IO", "IO12": "USB_D-", "IO13": "USB_D+",
                 "IO21": "LED_PWR_IO", "IO22": "LED_COFFEE_IO", "TXD0": "STATUS", "RXD0": "SCALE_SCK",
                 "IO4": "SCALE_DOUT1", "IO5": "SCALE_DOUT2"}
STRAPS = ("IO4", "IO5", "IO8", "IO9", "IO15")   # ESP32-C6 strapping pins
ADC1 = ("IO0", "IO1", "IO2", "IO3", "IO4", "IO5", "IO6")


def netname(pin):
    n = u_mcu[pin].net
    return n.name if n is not None and n is not NC and n.pins else None


def design_checks():
    errors = []
    # 1. Low voltage only, single-sided SMD assembly: no mains parts, no through-hole parts.
    if MAINS_PINS:
        errors.append(f"MAINS: {sorted(MAINS_PINS)} on a low-voltage board")
    for p in builtins.default_circuit.parts:
        if any(k in p.footprint for k in ("TO-220", "DIP", "THT", "TerminalBlock", "Vertical")) \
                and BOM_EXTRA.get(p.ref, {}).get("populate", "Y") == "Y":    # bare holes (J4 programming) are fine
            errors.append(f"SMD: {p.ref} ({p.footprint}) is through-hole")
    # 2. Firmware pin map (firmware/esp32/src/main.rs).
    for pin, want in FIRMWARE_PINS.items():
        if netname(pin) != want:
            errors.append(f"PINMAP: {pin} is on {netname(pin)}, firmware expects {want}")
    # 3. Analog inputs on ADC1 (ADC2 doesn't exist on the C6; ADC1 works alongside Wi-Fi).
    for net in ("NTC_ADC", "P_ADC"):
        pin = next((p for p in FIRMWARE_PINS if FIRMWARE_PINS[p] == net), None)
        if pin not in ADC1:
            errors.append(f"ADC: {net} must be on an ADC1 pin")
    # 4. Straps: loads never on a strapping pin; only GPIO8 (pull-up) and GPIO9 (BOOT pad) connected.
    for pin in STRAPS:
        if netname(pin) and netname(pin).endswith("_CMD"):
            errors.append(f"BOOT-SAFE: {netname(pin)} is on strapping pin {pin}")
    # GPIO4/5 (MTMS/MTDI) strap only SDIO clock edges: allowed as the scale's data *inputs*, nothing else.
    for pin, ok in (("IO4", "SCALE_DOUT1"), ("IO5", "SCALE_DOUT2"), ("IO15", None)):
        if netname(pin) != ok:
            errors.append(f"STRAP: {pin} must be {ok or 'unconnected'} (is {netname(pin)})")
    if netname("IO8") != "IO8_STRAP" or not any(p.part.ref == "R11" for p in u_mcu["IO8"].net.pins):
        errors.append("STRAP: GPIO8 needs its pull-up")
    if errors:
        print("\n".join(errors))
        sys.exit(1)
    print("design checks: no mains, all SMD, firmware pin map OK, analog on ADC1, straps OK")


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
