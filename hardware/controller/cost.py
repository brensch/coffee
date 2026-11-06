#!/usr/bin/env python3
"""Rough JLC order cost for each order set in build/ (full assembly vs SMD-only + hand-soldered THT).

Part prices: JLC's own tiered price at the ordered quantity (extended parts may round up to a minimum buy).
Fees: JLC's published standard-PCBA schedule as assumed below (check the quote: they change).
    source ~/opt/kienv.sh && python3 cost.py [boards=5]
"""
import csv, json, os, sys
import pcbnew

HERE = os.path.dirname(os.path.abspath(__file__))
N = int(sys.argv[1]) if len(sys.argv) > 1 else 5
FEES = dict(setup=8.00, stencil=1.50, extended_type=3.00, smd_joint=0.0016, tht_joint=0.0173, tht_setup=3.50,
            pcb_5=7.00)   # pcb_5: 5 bare 2-layer 98x55.5 boards incl. slots, before shipping

b = pcbnew.LoadBoard(os.path.join(HERE, "build/coffee-controller-120v.kicad_pcb"))
pads = {fp.GetReference(): len([p for p in fp.Pads() if p.GetNumber()]) for fp in b.GetFootprints()}


_PC = os.path.join(HERE, "build/jlc-price-cache.json")
_cache = json.load(open(_PC)) if os.path.exists(_PC) else {}


def price(code, qty=1):
    """JLC's own tiered unit price at this order quantity (cached in build/jlc-price-cache.json)."""
    if code not in _cache:
        from jlc import query
        r = [x for x in query(code, 5) if x.get("componentCode") == code]
        _cache[code] = (r[0].get("componentPrices") or []) if r else []
        json.dump(_cache, open(_PC, "w"), indent=0)
    tiers = _cache[code]
    if not tiers:
        return None
    fit = [t for t in tiers if t["startNumber"] <= qty and (t["endNumber"] in (-1, None) or qty <= t["endNumber"])]
    return float((fit or tiers[:1])[0]["productPrice"])


def order(d):
    rows = list(csv.DictReader(open(os.path.join(HERE, d, "bom.csv"))))
    lib = {}
    for line in open(os.path.join(HERE, d, "report.md")):
        c = [x.strip() for x in line.split("|")]
        if len(c) > 6 and c[4].startswith("C"):
            lib[c[4]] = c[5]
    hand = list(csv.DictReader(open(os.path.join(HERE, d, "hand.csv"))))
    parts = joints = ext = 0.0; missing = []
    tht_j = 0
    for r in rows:
        code = r["LCSC Part #"]; refs = [x.strip() for x in r["Designator"].split(",")]
        p = price(code, N * len(refs))
        if p is None:
            missing.append(code); p = 0
        parts += p * len(refs)
        js = sum(pads.get(x, 2) for x in refs)
        if any(k in r["Footprint"] for k in ("_THT", "FASTON")):
            tht_j += js
        else:
            joints += js
        ext += lib.get(code) == "expand"
    hand_cost = sum((price(r["LCSC Part #"]) or 0) for r in hand)
    fee = FEES["setup"] + FEES["stencil"] + ext * FEES["extended_type"] + N * (joints * FEES["smd_joint"] + tht_j * FEES["tht_joint"]) + (FEES["tht_setup"] if tht_j else 0)
    return dict(parts=parts, hand=hand_cost, fee=fee, ext=int(ext), joints=int(joints), tht=tht_j, missing=missing,
                total=FEES["pcb_5"] * N / 5 + fee + N * parts)


for d, label in (("build/jlc-120v", "full assembly (JLC solders everything)"), ("build/jlc-120v-smd", "SMD by JLC, you solder 4 spades + IRM")):
    o = order(d)
    print(f"{label}:\n  {N} boards ~ ${o['total']:.0f}  (parts ${o['parts']:.2f}/board, {o['ext']} extended types, "
          f"{o['joints']} SMD + {o['tht']} THT joints/board, fees ${o['fee']:.0f})")
    if o["hand"]:
        print(f"  + parts you buy and solder: ${o['hand']:.2f}/board (x{N} = ${o['hand']*N:.0f})  -> ${o['total'] + o['hand']*N:.0f}")
    if o["missing"]:
        print("  no price for", o["missing"])
