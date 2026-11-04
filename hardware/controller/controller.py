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
ME6211 (SOT-23-5: 1 VIN, 2 GND, 3 EN, 5 VOUT), AO3400A / AO3401A (SOT-23: 1 G, 2 S, 3 D).
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
JST3 = conn(3, "JST_GH_3_SMD", "Connector_JST:JST_GH_SM03B-GHS-TB_1x03-1MP_P1.25mm_Horizontal")

# Microne ME6211C33M5G-N: 500 mA, stable with ceramic caps (the AMS1117 wants tantalum ESR), 6 V max in,
# SOT-23-5 (8.8 x 7.2 mm SOT-223 courtyard -> 3.3 x 3.6). Peak ESP32-C6 TX ~350 mA.
LDO = tmpl("ME6211C33M5G-N", "U", "Package_TO_SOT_SMD:SOT-23-5",
           [(1, "VIN", T.PWRIN), (2, "GND", T.PWRIN), (3, "EN", T.INPUT), (4, "NC", T.NOCONNECT), (5, "VOUT", T.PWROUT)],
           "LDO 3.3 V 500 mA, ceramic-stable")
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
j_pwr = part(JST3, "J1", "5V in", mpn="JST SM03B-GHS-TB", notes="1=+5V 2=GND 3=GND, from the 5 V AC-DC module")
j_pwr[1, 2, 3] += v5_in, gnd, gnd
q_rp = part(PMOS, "Q4", "AO3401A", mpn="AO3401A", notes="Reverse-polarity protection")
q_rp["D"] += v5_in
q_rp["S"] += v5
rg = Net("RP_GATE")
q_rp["G"] += rg
r = part(R, "R26", "10k", notes="P-FET gate to GND")
r[1, 2] += rg, gnd

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
    c = part(C, ref, val, footprint=fp, notes="Module decoupling at the 3V3 pin")
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
tp = part(TESTPAD, "TP1", "BOOT", notes="Recovery only (USB Serial/JTAG resets into download mode by itself): short to GND (USB shell) while plugging USB in")
tp[1] += boot
st, st_a = Net("STATUS"), Net("STATUS_A")
u_mcu["IO23"] += st
r = part(R, "R12", "1k", notes="red LED Vf ~2.0 V: ~1.3 mA from 3.3 V")
r[1, 2] += st, st_a
led = part(LED, "D1", "red", notes="Heartbeat / ready / fault. Light-pipe it to the front of the machine (optional)")
led["A"] += st_a
led["K"] += gnd

# Power: J1 -> Q4 -> +5V; USB VBUS joins through D2 (bench power, no back-feed into the PC).
d2 = part(SCHOTTKY, "D2", "B5819W", mpn="B5819W SL", notes="USB bench power")
d2["A"] += vbus
d2["K"] += v5
u_ldo = part(LDO, "U5", "ME6211C33M5G-N", mpn="Microne ME6211C33M5G-N", notes="EN tied to VIN")
u_ldo["VIN"] += v5
u_ldo["EN"] += v5
u_ldo["GND"] += gnd
u_ldo["VOUT"] += v3
u_ldo["NC"] += NC
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
# Every module has its own input LED, so the board has none per output. J8 (valve) drives a relay
# coil and gets the 1N4148W flyback; the SSR inputs (J2, J3) aren't inductive.
# GPIO order on the module edge (IO18, IO19, IO20) matches the connector order (valve, pump, heater).
for name, gpio, jref, qref, rpd, dfb in (("HEATER", "IO20", "J2", "Q1", "R4", None), ("PUMP", "IO19", "J3", "Q2", "R5", None),
                                         ("VALVE", "IO18", "J8", "Q3", "R6", "D5")):
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

# Thermistor: 100k NTC (Semitec 104NT, B 4267) to GND, 8.2k reference to 3V3: 1.52 V at 93 C.
# R27 1k between the node and GPIO1: every cable uses the same GH 3-pin plug, and the 5 V lead in J5
# would otherwise put the supply straight onto the pin's clamp diode (~1 mA through 1k). No current
# flows in R27 in normal use, so it adds no measurement error.
ntc, ntc_adc = Net("NTC"), Net("NTC_ADC")
j_ntc = part(JST3, "J5", "Thermistor", mpn="JST SM03B-GHS-TB", notes="NTC probe in the brew-thermostat hole. 1=NTC 2=GND (no polarity), 3 unused")
j_ntc[1, 2] += ntc, gnd
j_ntc[3] += NC
r = part(R, "R15", "8.2k", notes="NTC reference: 3.10 V at 20 C, 1.52 V at 93 C, 0.50 V at 150 C (B 4267)")
r[1, 2] += v3, ntc
c = part(C, "C8", "100nF X7R", notes="ADC sampling reservoir / noise filter")
c[1, 2] += ntc, gnd
r = part(R, "R27", "1k", notes="GPIO1 series protection: a 5 V plug in the wrong socket pushes ~1 mA into the clamp diode")
r[1, 2] += ntc, ntc_adc
u_mcu["IO1"] += ntc_adc   # ADC1_CH1

# Pressure: 0.5-4.5 V ratiometric transducer on +5V, divided by 2/3 (4.5 V -> 3.0 V) into ADC1.
p_sig, p_adc = Net("P_SIG"), Net("P_ADC")
j_p = part(JST3, "J6", "Pressure", mpn="JST SM03B-GHS-TB", notes="0-1.6 MPa, 5 V, 0.5-4.5 V. 1=+5V 2=SIG 3=GND")
j_p[1, 2, 3] += v5, p_sig, gnd
r = part(R, "R16", "10k", notes="Divider 2/3")
r[1, 2] += p_sig, p_adc
r = part(R, "R17", "20k")
r[1, 2] += p_adc, gnd
c = part(C, "C9", "100nF X7R", notes="ADC reservoir; ~240 Hz with the divider: keeps the pump's 60 Hz pulses")
c[1, 2] += p_adc, gnd
u_mcu["IO0"] += p_adc   # ADC1_CH0

# Front-panel switches: dry contacts to GND; internal pull-ups, debounced in firmware.
j_sw = part(JST3, "J7", "Switches", mpn="JST SM03B-GHS-TB", notes="1=BREW 2=STEAM 3=GND. Stock switches rewired as dry contacts")
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
for net, rail, gpio, rt_, rb_, vt, cref in (("V3_SNS", v3, "IO2", "R22", "R23", "10k", "C10"), ("V5_SNS", v5, "IO3", "R24", "R25", "20k", "C11")):
    sns = Net(net)
    r = part(R, rt_, vt, notes=f"{net} divider top")
    r[1, 2] += rail, sns
    r = part(R, rb_, "10k", notes=f"{net} divider bottom")
    r[1, 2] += sns, gnd
    c = part(C, cref, "100nF X7R", notes="ADC reservoir")
    c[1, 2] += sns, gnd
    u_mcu[gpio] += sns

for p in ("IO4", "IO5", "IO14", "IO15", "IO21", "IO22", "RXD0", "TXD0"):
    u_mcu[p] += NC
# No mounting holes (each costs ~50 mm2 of board): the printed holder grips the board's edges.


# ================================================================ checks + outputs
def nets():
    return builtins.default_circuit.nets


FIRMWARE_PINS = {"IO0": "P_ADC", "IO1": "NTC_ADC", "IO2": "V3_SNS", "IO3": "V5_SNS", "IO18": "VALVE_CMD", "IO19": "PUMP_CMD", "IO20": "HEATER_CMD",
                 "IO6": "SW_BREW", "IO7": "SW_STEAM", "IO23": "STATUS", "IO12": "USB_D-", "IO13": "USB_D+"}
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
        if any(k in p.footprint for k in ("TO-220", "DIP", "THT", "TerminalBlock", "Vertical")):
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
    for pin in ("IO4", "IO5", "IO15"):
        if netname(pin):
            errors.append(f"STRAP: {pin} must be left unconnected (is {netname(pin)})")
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
