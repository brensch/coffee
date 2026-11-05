#!/usr/bin/env python3
"""JLCPCB assembly files: BOM with LCSC part numbers + placement (CPL).

    python3 jlc.py [--mains 120]                 # top side only (Economic PCBA): bottom parts -> hand.csv
    python3 jlc.py --both-sides                  # top + bottom (Standard PCBA): writes build/jlc-<V>v-2side/

Reads build/bom-<V>v.csv (controller.py) and build/coffee-controller-<V>v-fab/positions.csv
(layout.py), looks every line up in JLCPCB's parts library and writes build/jlc-<V>v/:
  bom.csv  - Comment, Designator, Footprint, LCSC Part #   (SMD parts JLC places)
  cpl.csv  - Designator, Mid X, Mid Y, Layer, Rotation     (same parts)
  hand.csv - through-hole parts you solder yourself (with LCSC numbers where found)
  report.md - what matched, stock, basic/extended, and anything that needs a decision

Chips are matched by exact manufacturer part number; passives by value + package,
preferring JLC "basic" parts (no per-part setup fee) with stock.
"""
import argparse
import csv
import json
import os
import re
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
API = "https://jlcpcb.com/api/overseas-pcb-order/v1/shoppingCart/smtGood/selectSmtComponentList"

PKG = {  # KiCad footprint fragment -> JLC package string fragment
    "R_0603": "0603", "C_0603": "0603", "LED_0603": "0603", "C_0805": "0805",
    "R_1206": "1206", "C_1206": "1206", "R_2512": "2512",
    "CP_Elec_6.3x7.7": "6.3", "D_SMA": "SMA", "D_SOD-323": "SOD-323",
    "SOT-23-6": "SOT-23-6", "SOT-23": "SOT-23", "SOT-223": "SOT-223",
    "SOIC-8": "SOIC-8", "MSOP-10": "MSOP-10", "TL3342": "SMD",
}
THT = ("TestPoint", "MountingHole")   # bare pads/holes: not assembled. Everything else is SMD, placed by JLC.
# MPN fixes: the part JLC actually stocks for the same function.
MPN_HINTS = {
    "ESP32-C6-MINI-1U-H4": "ESP32-C6-MINI-1U-H4", "HRO TYPE-C-31-M-12": "TYPE-C-31-M-12",
    "JST SM03B-GHS-TB": "SM03B-GHS-TB", "JST SM06B-GHS-TB": "SM06B-GHS-TB", "JST SM07B-GHS-TB": "SM07B-GHS-TB", "TI TLV1117LV33DCYR": "TLV1117LV33DCYR",
    "AO3400A": "AO3400A", "AO3401A": "AO3401A", "1N4148W": "1N4148W", "B5819W SL": "B5819W SL",
    "Samsung CL21A226MAQNNNE": "CL21A226MAQNNNE",
}


def query(keyword, size=30):
    body = json.dumps({"keyword": keyword, "currentPage": 1, "pageSize": size}).encode()
    req = urllib.request.Request(API, data=body, headers={"Content-Type": "application/json", "User-Agent": "coffee-bom/1"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                d = json.load(r)
            return (d.get("data") or {}).get("componentPageInfo", {}).get("list") or []
        except Exception:
            time.sleep(2 * (attempt + 1))
    return []


def best(cands, need=lambda c: True):
    ok = [c for c in cands if need(c) and (c.get("stockCount") or 0) > 0]
    ok.sort(key=lambda c: (c.get("componentLibraryType") != "base", -(c.get("stockCount") or 0)))
    return ok[0] if ok else None


def norm_value(v):
    """'10k 0.1%' -> ('10kΩ', '0.1%'), '100nF' -> ('100nF', None)."""
    tol = None
    m = re.search(r"([\d.]+%)", v)
    if m:
        tol = m.group(1)
    v0 = v.split()[0]
    if re.fullmatch(r"[\d.]+[kKmM]?", v0):
        v0 = v0.replace("K", "k") + "Ω"
    return v0, tol


BODY_CENTRE = ("U4",)   # footprint origin is not the body centre
# JLC's part models whose zero orientation differs from KiCad's footprint (checked in JLC's preview)
ROT_FIX = {}   # rotations come from jlc_cpl.py (solved against JLC's footprints)
FIXED = {"D_SOD-123|B5819W": "C8598",   # B5819W SL, basic part (its model name carries the "SL" suffix)
         "SOT-223-3_TabPin2|TLV1117LV33DCYR": "C15578",   # TI's own part (C48937499 shares the name: 12 V, not TI)
         "C_0603_1608Metric|22nF X7R": "C21122"}   # Samsung CL10B223KB8NNNC, basic (the matcher picked an extended one)


def lookup(row):
    value, mpn, fp = row["Value"], row["Part / MPN"], row["Footprint"]
    for key, code in FIXED.items():
        frag, val = key.split("|")
        if frag in fp and value == val:
            c = best(query(code), lambda c: c.get("componentCode") == code)
            return c, f"fixed {code}"
    pkg = next((p for k, p in PKG.items() if k in fp), None)
    key = MPN_HINTS.get(mpn)
    if key:
        core = re.sub(r"(-?TR|RG)$", "", key, flags=re.I).lower()   # the full part number, packaging suffix aside
        c = best(query(key), lambda c: (c.get("componentModelEn") or "").lower().startswith(core))
        return c, f"mpn {key}"
    if fp.startswith(("Resistor_SMD", "Capacitor_SMD", "LED_SMD")):
        v, tol = norm_value(value)
        kind = "LED" if fp.startswith("LED") else ""
        if kind:
            colour = value.split()[-1]
            kw = f"{pkg} {colour} LED"
            c = best(query(kw), lambda c: pkg in (c.get("componentSpecificationEn") or "") and colour.lower() in (c.get("describe") or "").lower())
            return c, kw
        kw = f"{v} {tol or ''} {pkg}".replace("  ", " ")
        def need(c):
            d = (c.get("describe") or "") + " " + (c.get("componentModelEn") or "")
            spec = c.get("componentSpecificationEn") or ""
            if pkg not in spec:
                return False
            if v.replace("Ω", "") not in d.replace("Ω", "").replace(" ", ""):
                return False
            if "X7R" in value and "X7R" not in d:   # temperature grade is part of the spec (X5R is 85 C only)
                return False
            return tol is None or tol in d
        if "X7R" in value:
            kw += " X7R"
        return best(query(kw), need), kw
    return None, "no rule"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mains", type=int, default=120)
    ap.add_argument("--both-sides", action="store_true", help="JLC places the bottom-side parts too (Standard PCBA)")
    a = ap.parse_args()
    b = os.path.join(HERE, "build")
    rows = list(csv.DictReader(open(os.path.join(b, f"bom-{a.mains}v.csv"))))
    pos = {r["Ref"]: r for r in csv.DictReader(open(os.path.join(b, f"coffee-controller-{a.mains}v-fab", "positions.csv")))}
    out = os.path.join(b, f"jlc-{a.mains}v" + ("-2side" if a.both_sides else ""))
    os.makedirs(out, exist_ok=True)
    smd, hand, report = [], [], []
    for row in rows:
        refs = row["Refs"].split()
        if row["Populate"] != "Y" or "MountingHole" in row["Footprint"] or "TestPoint" in row["Footprint"]:
            continue
        c, how = lookup(row)
        code = c["componentCode"] if c else ""
        lib = c.get("componentLibraryType", "") if c else ""
        stock = c.get("stockCount", 0) if c else 0
        model = c.get("componentModelEn", "") if c else ""
        # through-hole parts, and anything placed on the bottom (connectors you solder yourself:
        # Economic PCBA assembles the top side only), go on the hand-solder list
        is_tht = any(t in row["Footprint"] for t in THT) or (not a.both_sides and all(
            pos.get(r, {}).get("Side", "top").lower().startswith("bottom") for r in refs))
        line = dict(Comment=row["Value"], Designator=",".join(refs), Footprint=row["Footprint"].split(":")[-1], **{"LCSC Part #": code})
        (hand if is_tht else smd).append(line)
        flag = "" if code else "  **NO MATCH: pick by hand**"
        report.append(f"| {' '.join(refs)} | {row['Value']} | {model} | {code} | {lib} | {stock} | {'hand' if is_tht else 'JLC'} |{flag}")
    # one line per LCSC part: JLC rejects two BOM lines that map to the same part
    merged = {}
    for line in smd:
        k = line["LCSC Part #"] or line["Designator"]
        if k in merged:
            merged[k]["Designator"] += "," + line["Designator"]
        else:
            merged[k] = dict(line)
    smd = list(merged.values())
    fields = ["Comment", "Designator", "Footprint", "LCSC Part #"]
    for name, data in (("bom.csv", smd), ("hand.csv", hand)):
        with open(os.path.join(out, name), "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=fields)
            w.writeheader()
            w.writerows(data)
    placed = {d for line in smd for d in line["Designator"].split(",")}
    with open(os.path.join(out, "cpl.csv"), "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Designator", "Mid X", "Mid Y", "Layer", "Rotation"])
        # JLC centres its part model on the given point; footprints whose origin isn't the body centre
        # (the ESP32 module: 2.05 mm off) get their courtyard centre, measured on the real board.
        centre = {}
        try:
            import pcbnew
            brd = pcbnew.LoadBoard(os.path.join(b, f"coffee-controller-{a.mains}v.kicad_pcb"))
            for ref in BODY_CENTRE:
                fp = brd.FindFootprintByReference(ref)
                if fp:
                    c = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd).BBox().GetCenter()
                    centre[ref] = (pcbnew.ToMM(c.x), -pcbnew.ToMM(c.y))     # positions.csv is y-up
        except ImportError:
            print("pcbnew not available: U4 keeps its footprint origin (2 mm off its body centre)")
        for ref in sorted(placed):
            p = pos.get(ref)
            if p:
                x, y = centre.get(ref, (float(p['PosX']), float(p['PosY'])))
                rot = (float(p['Rot']) + ROT_FIX.get(ref, 0)) % 360
                w.writerow([ref, f"{x:.3f}mm", f"{y:.3f}mm",
                            "Top" if p["Side"].lower().startswith("top") else "Bottom", f"{rot:.0f}"])
    with open(os.path.join(out, "report.md"), "w") as f:
        f.write(f"# JLCPCB parts, {a.mains} V board\n\n"
                "Looked up live in JLCPCB's library. 'base' = basic part (no setup fee), 'expand' = extended "
                "(~$3 setup per part type). Check rotations in JLC's placement preview: footprint zero "
                "orientations differ between KiCad and JLC for some packages (SOT-223, USB-C, the ESP32 module).\n\n"
                "| Refs | Value | JLC part | LCSC | library | stock | who |\n|---|---|---|---|---|---|---|\n")
        f.write("\n".join(report) + "\n")
    # the CPL is then re-solved against JLC's own footprints (jlc_cpl.py): angles and positions proven
    import subprocess
    env = dict(os.environ, JLC_DIR=out)
    r = subprocess.run([sys.executable, os.path.join(HERE, "jlc_cpl.py")], capture_output=True, text=True, env=env)
    print(r.stdout.strip().splitlines()[-1] if r.stdout else r.stderr[-500:])
    if r.returncode:
        raise SystemExit("CPL check failed: see jlc_cpl.py output")
    # and every polarised pin's function checked against JLC's own symbol (catches pad numbers that agree
    # while functions don't, e.g. an LED numbered anode-first by JLC and cathode-first by KiCad)
    r = subprocess.run([sys.executable, os.path.join(HERE, "jlc_pins.py")], capture_output=True, text=True, env=env)
    print(r.stdout.strip().splitlines()[-1] if r.stdout.strip() else r.stderr[-500:])
    if r.returncode:
        print(r.stdout)
        raise SystemExit("pin-function check failed: see jlc_pins.py output")
    print(f"wrote {out}: {len(smd)} SMD lines, {len(hand)} hand-solder lines, "
          f"{sum(1 for r in report if 'NO MATCH' in r)} unmatched")


if __name__ == "__main__":
    main()
