#!/usr/bin/env python3
"""Mis-plug check: every lead in every wrong same-size socket must be harmless.

Reads the board's connector pinouts from build/parts-120v.json, classifies each socket pin by what the
board puts on it, models what each lead's device puts between its pins, and checks every lead against
every other same-size socket. Exits 1 on any hazard. See docs/SWAP-SAFETY.md for the model.

    python3 swap_check.py
"""
import json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
P = json.load(open(os.path.join(HERE, "build", "parts-120v.json")))["parts"]

# What the board drives onto a socket pin, by net. hi/lo = can source/sink >= ~1 mA; gpio = direct GPIO pin.
NETTYPE = {"+5V": "S5", "+5V_IN": "S5IN", "+3V3": "S3", "GND": "GND", "HEATER_OUT": "DR", "PUMP_OUT": "DR",
           "VALVE_OUT": "DR", "LED_PWR": "LEDR", "LED_COFFEE": "LEDR", "LED_STEAM": "LEDR",
           "SW_POWER_IN": "SWIN", "SW_BREW_IN": "SWIN", "SW_STEAM_IN": "SWIN", "SCALE_SCK": "SCK",
           "SCALE_DOUT1": "DIN", "SCALE_DOUT2": "DIN", "P_SIG": "PSIG", "NTC": "NTCN", None: "NC"}
T = {"S5": (1, 0, 1), "S5IN": (0, 0, 0), "S3": (1, 0, 0), "GND": (0, 1, 0), "DR": (0, 1, 0), "LEDR": (1, 1, 0),
     "SWIN": (0, 0, 0), "SCK": (1, 1, 1), "DIN": (0, 0, 1), "PSIG": (0, 0, 0), "NTCN": (0, 0, 0), "NC": (0, 0, 0)}
HI, LO, GPIO = 0, 1, 2   # tuple index; S5's third field is reused below only via the name

def socket(ref):
    pins = P[ref]["pins"]
    return [NETTYPE.get(pins[str(i)], "NC") for i in range(1, len(pins) + 1)]

def lead(ref):
    """The device on each lead, as two-terminal elements (kind, pin a, pin b) on lead pins (0-based)."""
    pins = P[ref]["pins"]; at = {v: int(k) - 1 for k, v in pins.items() if v}
    if ref == "J2":   # heater SSR input, two SSR boards (VCC+IN on +5V, GND on OUT)
        return [("SSR_H", at["+5V"], at["HEATER_OUT"]), ("SSR_B", at["+5V"], at["PUMP_OUT"]), ("SSR_B", at["+5V"], at["VALVE_OUT"])]
    if ref == "J7":
        g = at["GND"]
        return [("SW", at[n], g) for n in ("SW_POWER_IN", "SW_BREW_IN", "SW_STEAM_IN")] + \
               [("LED", at[n], g) for n in ("LED_PWR", "LED_COFFEE", "LED_STEAM")]
    if ref == "J10":
        return [("HX", {"GND": at["GND"], "AVDD": at["+5V"], "VDD": at["+3V3"], "SCK": at["SCALE_SCK"],
                        "D1": at["SCALE_DOUT1"], "D2": at["SCALE_DOUT2"]})]
    if ref == "J1":   # the 5 V supply: a strong source, + on its +5V pin, - on its GND pins
        return [("SRC", at["+5V_IN"], [i for i, v in enumerate(P[ref]["pins"].values()) if v == "GND"])]
    if ref == "J6":   # transducer: needs supply on its +5V pin, drives SIG weakly
        return [("XDCR", at["+5V"], at["GND"])]
    if ref == "J5":   # NTC: a 50k resistor between its two used pins
        return [("RES", at["NTC"], at["GND"])]
    raise KeyError(ref)

def hazards(L, S):
    hz = []
    hi = lambda i: T[S[i]][HI] == 1
    lo = lambda i: T[S[i]][LO] == 1
    for el in L:
        k = el[0]
        if k == "SSR_H" and hi(el[1]) and lo(el[2]): hz.append(f"heater SSR on ({S[el[1]]} over {S[el[2]]})")
        if k == "SSR_H" and S[el[1]] == "S5" and S[el[2]] == "S3": hz.append("heater SSR at 1.7 V (undefined)")
        if k == "SSR_B" and S[el[1]] == "S5" and lo(el[2]): hz.append(f"SSR board on (5 V over {S[el[2]]})")
        if k == "SW":
            for a, b in ((el[1], el[2]), (el[2], el[1])):
                if S[a] in ("S5", "S3", "SCK") and S[b] in ("GND", "DR", "SCK") and a != b: hz.append(f"switch shorts {S[a]} to {S[b]}")
                if S[a] == "S5" and T[S[b]][GPIO]: hz.append(f"switch puts 5 V on GPIO ({S[b]})")
        if k == "LED" and S[el[1]] in ("S5", "S3", "SCK") and S[el[2]] in ("GND", "DR", "SCK"): hz.append(f"LED without resistor ({S[el[1]]} to {S[el[2]]})")
        if k == "HX":
            pins = el[1]
            sh = [r for r in pins if S[pins[r]] in ("S5", "S3")]
            sl = [r for r in pins if S[pins[r]] in ("GND", "DR")]
            powered = S[pins["GND"]] == "GND" and all(r in ("AVDD", "VDD") for r in sh)
            if S[pins["GND"]] in ("S5", "S3"): hz.append("HX711 ground on a supply")
            if sh and sl and not powered: hz.append(f"HX711 diode path ({sh} high, {sl} low)")
        if k == "SRC":   # external 5 V: its + must not meet a board supply, ground, drain or GPIO; its - pins must not meet a board supply
            p = el[1]
            # onto the board's own +5V rail is the same polarity: the board just runs from it (D2 keeps USB clear)
            if S[p] in ("S3", "GND", "DR", "SCK", "DIN", "LEDR"): hz.append(f"supply +5V onto board {S[p]}")
            if S[p] in ("NTCN", "PSIG", "SWIN"): pass   # behind 1k / 47k / 4.7k: < 2 mA into a clamp
            for g in el[2]:
                if S[g] in ("S5", "S3"): hz.append(f"supply ground onto board {S[g]}")
        if k == "XDCR":   # the transducer powered backwards from a board supply
            if S[el[2]] in ("S5", "S3") and S[el[1]] in ("GND", "DR"): hz.append("transducer reverse-powered")
    return hz

groups = {}
for ref, p in P.items():
    if ref.startswith("J") and ref != "J4":
        groups.setdefault(len(p["pins"]), []).append(ref)
bad = 0
for n, refs in sorted(groups.items()):
    for l in refs:
        for s in refs:
            if l == s:
                continue
            h = hazards(lead(l), socket(s))
            print(f"{n}-pin: {l} lead in {s} socket: " + ("; ".join(h) if h else "harmless"))
            bad += bool(h)
print("ALL HARMLESS" if not bad else f"{bad} HAZARDOUS COMBINATIONS")
sys.exit(1 if bad else 0)
