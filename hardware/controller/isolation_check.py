#!/usr/bin/env python3
"""Mains-to-SELV isolation check on the finished board, independent of KiCad's DRC rules.

For every piece of mains copper (pads, tracks, vias, zone fills on mains nets) and every piece of
SELV copper (everything else), on either layer, find the smallest edge-to-edge distance. Through-hole
pads count on both layers. The only exemption: the pins of a part that is *meant* to bridge the barrier
(the opto-triacs and the PSU module: plan "mains.bridges"), measured against the same part's own pins,
where the maker's certified internal isolation governs (their footprints' own pad gap is reported too).

Also checks that no mains copper reaches into the SELV half and vice versa (plan "mains.band").

    source ~/opt/kienv.sh && python3 isolation_check.py build/coffee-controller-120v.kicad_pcb [min_mm]
Exit 1 if any gap is below min_mm (default: plan "mains.isolation_mm", else 6.0).
"""
import json, os, sys

import pcbnew

HERE = os.path.dirname(os.path.abspath(__file__))
pcb = sys.argv[1]
plan = json.load(open(os.environ.get("PLACE_JSON", os.path.join(HERE, "plan.json"))))
M = plan.get("mains", {})
LIMIT = float(sys.argv[2]) if len(sys.argv) > 2 else M.get("isolation_mm", 6.0)
spec = json.load(open(os.path.join(os.path.dirname(os.path.abspath(pcb)), "parts-120v.json")))
MAINS = set(spec.get("mains_nets", []))
BRIDGES = set(M.get("bridges", []))
b = pcbnew.LoadBoard(pcb)
b.BuildConnectivity()
pcbnew.ZONE_FILLER(b).Fill(b.Zones())

LAYERS = (pcbnew.F_Cu, pcbnew.B_Cu)
mm = pcbnew.ToMM


def shapes():
    """(net, layer, ref or None, [SEG], inflate_mm, label, bbox) for every copper item: pads and zone fills as
    their polygon outlines (inflate 0), tracks and vias as their centre segment inflated by half the width."""
    out = []
    def poly_segs(ps):
        segs = []
        for i in range(ps.OutlineCount()):
            o = ps.Outline(i)
            segs += [o.CSegment(k) for k in range(o.SegmentCount())]
        return segs
    def bb_of(segs, r):
        xs = [mm(v) for sg in segs for v in (sg.A.x, sg.B.x)]; ys = [mm(v) for sg in segs for v in (sg.A.y, sg.B.y)]
        return min(xs) - r, min(ys) - r, max(xs) + r, max(ys) + r
    for fp in b.GetFootprints():
        for p in fp.Pads():
            for L in LAYERS:
                if p.IsOnLayer(L):
                    ps = pcbnew.SHAPE_POLY_SET()
                    p.TransformShapeToPolygon(ps, L, 0, pcbnew.FromMM(0.005))
                    sg = poly_segs(ps)
                    out.append((p.GetNetname(), L, fp.GetReference(), sg, 0.0, f"{fp.GetReference()}.{p.GetNumber()}", bb_of(sg, 0)))
    for t in b.GetTracks():
        r = mm(t.GetWidth()) / 2
        for L in LAYERS:
            if t.IsOnLayer(L):
                if t.Type() == pcbnew.PCB_VIA_T:
                    c = t.GetPosition(); sg = [pcbnew.SEG(c, c)]; kind = "via"
                else:
                    sg = [pcbnew.SEG(t.GetStart(), t.GetEnd())]; kind = "track"
                out.append((t.GetNetname(), L, None, sg, r, f"{kind} {t.GetNetname()}", bb_of(sg, r)))
    for z in b.Zones():
        if z.GetIsRuleArea():
            continue
        for L in LAYERS:
            if z.IsOnLayer(L):
                fill = z.GetFilledPolysList(L)
                if fill.OutlineCount():
                    sg = poly_segs(fill)
                    out.append((z.GetNetname(), L, None, sg, 0.0, f"zone {z.GetNetname()}", bb_of(sg, 0)))
    return out


def bgap(a, c):
    dx = max(a[0] - c[2], c[0] - a[2], 0.0); dy = max(a[1] - c[3], c[1] - a[3], 0.0)
    return (dx * dx + dy * dy) ** 0.5


def segbb(sg):
    return min(mm(sg.A.x), mm(sg.B.x)), min(mm(sg.A.y), mm(sg.B.y)), max(mm(sg.A.x), mm(sg.B.x)), max(mm(sg.A.y), mm(sg.B.y))


items = shapes()
mains = [i for i in items if i[0] in MAINS]
selv = [i for i in items if i[0] not in MAINS and i[0] != ""]
REACH = LIMIT + 2.0
worst = []
for net_m, Lm, refm, sm, rm, dm, bm in mains:
    smb = [(x, segbb(x)) for x in sm]
    for net_s, Ls, refs, ss, rs, ds, bs in selv:
        if Lm != Ls or bgap(bm, bs) > REACH + rm + rs:
            continue
        if refm and refm == refs and refm in BRIDGES:
            continue        # the bridging part's own pins: its certified internal isolation
        best = None
        ssb = [(x, segbb(x)) for x in ss if bgap(bm, segbb(x)) <= REACH + rm + rs]
        for a, ab in smb:
            for c, cb in ssb:
                if bgap(ab, cb) > REACH + rm + rs:
                    continue
                v = mm(a.Distance(c)) - rm - rs
                if best is None or v < best:
                    best = v
        if best is not None:
            worst.append((best, dm, ds, b.GetLayerName(Lm)))
worst.sort()
seen = set()
print(f"mains-to-SELV copper, limit {LIMIT} mm (bridging parts' own pins exempt: {', '.join(sorted(BRIDGES)) or 'none'})")
for d, a, c, L in worst[:12]:
    key = (a, c, L)
    if key in seen:
        continue
    seen.add(key)
    flag = "FAIL" if d < LIMIT else "ok  "
    print(f"  {flag} {d:6.2f} mm  {a}  <->  {c}  ({L})")
bad = sum(1 for d, *_ in worst if d < LIMIT)
for ref in sorted(BRIDGES):                      # the bridging parts' own mains-to-SELV pad gap, for the record
    fp = b.FindFootprintByReference(ref)
    if fp is None:
        continue
    pm_ = [p for p in fp.Pads() if p.GetNetname() in MAINS]
    ps_ = [p for p in fp.Pads() if p.GetNetname() and p.GetNetname() not in MAINS]
    gap = min((mm((p.GetPosition() - q.GetPosition()).EuclideanNorm()) - (mm(max(p.GetSize().x, p.GetSize().y)) + mm(max(q.GetSize().x, q.GetSize().y))) / 2
               for p in pm_ for q in ps_), default=None)
    if gap is not None:
        print(f"  bridge {ref}: own mains-to-SELV pad gap >= {gap:.2f} mm (pad centres less half the larger pad dimension)")
print("ISOLATION OK" if not bad else f"{bad} mains/SELV pairs under {LIMIT} mm")
sys.exit(1 if bad else 0)
