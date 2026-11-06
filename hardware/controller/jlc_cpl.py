#!/usr/bin/env python3
"""Position file (CPL) for JLC assembly, solved against JLC's own footprints.

JLC places its EasyEDA footprint for each LCSC part at the CPL point and angle. KiCad's footprint
origins and zero angles differ from EasyEDA's part by part, and on the bottom side JLC mirrors the
footprint about the Y axis (so a bottom angle is 180 - KiCad's, before any per-part difference).
Rather than trusting conventions, this fetches each part's EasyEDA footprint and solves for the
angle and position at which JLC's pads land on the real board pads, then reports the worst pad
error per part. Any part whose pads don't fit within 0.25 mm (land patterns differ a little between libraries) fails loudly.

JLC bottom-side model (from kicad-jlcpcb-tools, confirmed on the USB-C in JLC's preview):
    board_pad = P + Mx(R(a) . p)      Mx: x -> -x,   R(a): KiCad's y-down rotation
top side:
    board_pad = P + R(a) . p

    source ~/opt/kienv.sh && python3 jlc_cpl.py      # rewrites build/jlc-120v/cpl.csv, prints the check
"""
import csv, json, math, os, sys, urllib.request

import pcbnew

HERE = os.path.dirname(os.path.abspath(__file__))
B = os.path.join(HERE, "build")
CACHE = os.path.join(B, "easyeda-cache")


# Parts whose land we changed on purpose (manufacturer's recommended pads differ from JLC's generic
# footprint): the centre and angle are still solved exactly; only the pad-size tolerance is wider.
TOLERANCE = {"Q4": 0.35,   # AO3401A C15127: JLC's SOT-23 land has its pads at +-1.15 mm vs KiCad's +-0.94 (same part, longer land); the angle fit is unique
             # v32: same parts, different land patterns, both centred on the body (the angle fit is unique):
             "Q5": 0.45, "Q6": 0.45,   # ST DPAK: KiCad TO-252-2 vs JLC's TO-252-2 land, 0.37 mm on 6.4 x 5.8 / 2.2 x 1.2 mm pads
             "C15": 0.40, "C16": 0.40}  # Murata GA355 X2 2220: Murata's recommended land (Coffee lib) vs JLC's generic C2220, 0.32 mm


# EasyEDA's API refused this machine (CloudFront 403) when C8598 was added: its rotation is solved from
# the cached record of the same-package diode C81598. Verified 2026-11-04 from JLC's own footprint and
# symbol drawings (easyeda /svgs via jlcpcb.com): both put pad 1 on the same side, and pin 1 is the
# cathode in both (C8598 '-', C81598 'K').
PROXY = {"C8598": "C81598",
         # v23 (EasyEDA refused again): unpolarised 0402 capacitors solved from cached 0402 capacitor
         # records of the same package (C60474 / C107017 are the matcher's first picks, same land).
         "C1525": "C60474", "C1532": "C107017",
         # v32: EasyEDA has no record for the ST T410-600B-TR (C2970064, 404): the T435-600B-TR (C145128) is
         # the same T4-series DPAK from the same ST datasheet (DocID7699: leads A1, A2 (cut), G; tab A2).
         "C2970064": "C145128"}


# Parts whose JLC pin numbers differ from the board footprint's: JLC pad -> board pad. Checked against
# JLC's own symbol: KT-0603R (C2286) numbers 1 = anode, 2 = cathode; KiCad's LED_0603 has 1 = cathode.
PAD_MAP = {"C2286": {"1": "2", "2": "1"}}
HOLE_WEIGHT = 50.0


def easyeda_pads(lcsc):
    lcsc = PROXY.get(lcsc, lcsc) if not os.path.exists(os.path.join(CACHE, lcsc + ".json")) else lcsc
    os.makedirs(CACHE, exist_ok=True)
    path = os.path.join(CACHE, lcsc + ".json")
    if not os.path.exists(path):
        url = f"https://easyeda.com/api/products/{lcsc}/components?version=6.4.19.5"
        req = urllib.request.Request(url, headers={"User-Agent": "coffee-jlc-cpl"})
        data = urllib.request.urlopen(req, timeout=30).read()
        json.loads(data)                      # never cache an error page
        open(path, "wb").write(data)
    d = json.load(open(path))["result"]["packageDetail"]["dataStr"]
    ox, oy = float(d["head"]["x"]), float(d["head"]["y"])
    pads = []
    for s in d["shape"]:
        f = s.split("~")
        if f[0] == "PAD":
            pads.append((f[8], (float(f[2]) - ox) * 0.254, (float(f[3]) - oy) * 0.254))   # 10 mil units, y down
        elif f[0] == "HOLE":                  # non-plated locating holes (USB-C pegs): fitted too, see solve()
            pads.append(("HOLE", (float(f[1]) - ox) * 0.254, (float(f[2]) - oy) * 0.254))
    return pads, d["head"].get("c_para", {}).get("package", "")


def rot(a, x, y):
    t = math.radians(a)
    c, s = round(math.cos(t), 9), round(math.sin(t), 9)
    return x * c + y * s, -x * s + y * c


def place(a, bottom, x, y):
    x, y = rot(a, x, y)
    return (-x, y) if bottom else (x, y)


def solve(board_pads, ee_pads, bottom):
    """Least-squares fit of JLC's pads onto the board pads, matched by pad number, for each of the
    four angles. Returns (worst pad error, angle, P, runner-up error) - the runner-up shows how
    clearly the angle is determined (a polarized part must have exactly one good angle)."""
    fits = []
    for a in (0, 90, 180, 270):
        pairs = []
        for n, x, y in ee_pads:
            if n in board_pads:
                px, py = place(a, bottom, x, y)
                # pads sharing a number (SOT-223 tab, USB shell): pick the nearest after a rough fit
                pairs.append((n, px, py))
        if not pairs:
            continue
        # rough translation from pad means, then pair repeated numbers by nearest
        bx = sum(sum(q[0] for q in board_pads[n]) / len(board_pads[n]) for n, _, _ in pairs) / len(pairs)
        by = sum(sum(q[1] for q in board_pads[n]) / len(board_pads[n]) for n, _, _ in pairs) / len(pairs)
        ex = sum(p[1] for p in pairs) / len(pairs); ey = sum(p[2] for p in pairs) / len(pairs)
        P = (bx - ex, by - ey)
        matched = []
        for n, px, py in pairs:
            q = min(board_pads[n], key=lambda q: math.hypot(px + P[0] - q[0], py + P[1] - q[1]))
            matched.append((px, py, q))
        # Locating pegs decide where the part physically sits, so they outweigh the pads (whose
        # copper has slack): the USB-C's pads and pegs differ by 0.18 mm between JLC's footprint and
        # HRO's drawing, and a 0.50 mm peg has only 0.05 mm of room in its 0.60 mm hole.
        wt = [HOLE_WEIGHT if n == "HOLE" else 1.0 for n, _, _ in pairs]
        W = sum(wt)
        P = (sum(w * (q[0] - px) for w, (px, py, q) in zip(wt, matched)) / W,
             sum(w * (q[1] - py) for w, (px, py, q) in zip(wt, matched)) / W)
        err = max(math.hypot(px + P[0] - q[0], py + P[1] - q[1]) for px, py, q in matched)
        fits.append((err, a, P))
    fits.sort()
    return fits[0] + (fits[1][0] if len(fits) > 1 else 99,)


def main():
    board = pcbnew.LoadBoard(os.path.join(B, "coffee-controller-120v.kicad_pcb"))
    D = os.environ.get("JLC_DIR", os.path.join(B, "jlc-120v"))     # jlc.py --both-sides: jlc-120v-2side
    bom = list(csv.DictReader(open(os.path.join(D, "bom.csv"))))
    rows, bad = [], []
    print(f"{'ref':5} {'side':6} {'LCSC':9} {'angle':>5}  pad error  angle-fit  EasyEDA package")
    for line in bom:
        code = line["LCSC Part #"]
        if not code:
            continue
        ee, pkg = easyeda_pads(code)
        if code in PAD_MAP:
            ee = [(PAD_MAP[code].get(n, n), x, y) for n, x, y in ee]
        for ref in line["Designator"].split(","):
            fp = board.FindFootprintByReference(ref)
            bottom = fp.IsFlipped()
            bp = {}
            for p in fp.Pads():
                c = p.GetPosition()
                if p.GetAttribute() in (pcbnew.PAD_ATTRIB_SMD, pcbnew.PAD_ATTRIB_PTH):
                    bp.setdefault(p.GetNumber(), []).append((pcbnew.ToMM(c.x), pcbnew.ToMM(c.y)))
                elif p.GetAttribute() == pcbnew.PAD_ATTRIB_NPTH:
                    bp.setdefault("HOLE", []).append((pcbnew.ToMM(c.x), pcbnew.ToMM(c.y)))
            err, a, P, second = solve(bp, ee, bottom)
            polar = second > 0.25          # only one angle fits: orientation is determined by the pads
            tol = TOLERANCE.get(ref, 0.25)
            flag = "" if err < tol else "   <-- DOES NOT FIT"
            if err >= tol:
                bad.append(ref)
            print(f"{ref:5} {'bottom' if bottom else 'top':6} {code:9} {a:5d}  {err:7.3f} mm  {'unique' if polar else 'symmetric':9} {pkg}{flag}")
            rows.append([ref, f"{P[0]:.3f}mm", f"{-P[1]:.3f}mm", "Bottom" if bottom else "Top", f"{a}"])
    out = os.path.join(D, "cpl.csv")
    with open(out, "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Designator", "Mid X", "Mid Y", "Layer", "Rotation"])
        w.writerows(sorted(rows))
    print(f"\nwrote {out}: {len(rows)} parts; " + (f"FAILED: {' '.join(bad)}" if bad else "every part's JLC pads land on the board pads"))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
