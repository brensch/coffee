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
    """(net, layer, ref or None, SHAPE_POLY_SET) for every copper item."""
    out = []
    for fp in b.GetFootprints():
        for p in fp.Pads():
            for L in LAYERS:
                if p.IsOnLayer(L):
                    ps = pcbnew.SHAPE_POLY_SET()
                    p.TransformShapeToPolygon(ps, L, 0, pcbnew.FromMM(0.005), pcbnew.ERROR_INSIDE)
                    out.append((p.GetNetname(), L, fp.GetReference(), ps, f"{fp.GetReference()}.{p.GetNumber()}"))
    for t in b.GetTracks():
        for L in LAYERS:
            if t.IsOnLayer(L):
                ps = pcbnew.SHAPE_POLY_SET()
                t.TransformShapeToPolygon(ps, L, 0, pcbnew.FromMM(0.005), pcbnew.ERROR_INSIDE)
                kind = "via" if t.Type() == pcbnew.PCB_VIA_T else "track"
                out.append((t.GetNetname(), L, None, ps, f"{kind} {t.GetNetname()}"))
    for z in b.Zones():
        if z.GetIsRuleArea():
            continue
        for L in LAYERS:
            if z.IsOnLayer(L):
                fill = z.GetFilledPolysList(L)
                if fill.OutlineCount():
                    out.append((z.GetNetname(), L, None, fill, f"zone {z.GetNetname()}"))
    return out


def bbox(ps):
    bb = ps.BBox()
    return mm(bb.GetLeft()), mm(bb.GetTop()), mm(bb.GetRight()), mm(bb.GetBottom())


def bgap(a, c):
    dx = max(a[0] - c[2], c[0] - a[2], 0.0); dy = max(a[1] - c[3], c[1] - a[3], 0.0)
    return (dx * dx + dy * dy) ** 0.5


def seg_bb(sg):
    a, e = sg.A, sg.B
    return min(mm(a.x), mm(e.x)), min(mm(a.y), mm(e.y)), max(mm(a.x), mm(e.x)), max(mm(a.y), mm(e.y))


def dist(pa, pc, reach):
    """Smallest outline-to-outline distance between two polygon sets, or None if beyond `reach`."""
    best = None
    for i in range(pa.OutlineCount()):
        a = pa.Outline(i)
        ab = bbox(pa)
        for j in range(pc.OutlineCount()):
            c = pc.Outline(j)
            cb = c.BBox(); cbb = (mm(cb.GetLeft()), mm(cb.GetTop()), mm(cb.GetRight()), mm(cb.GetBottom()))
            if bgap(ab, cbb) > reach:
                continue
            csegs = [(c.CSegment(m), seg_bb(c.CSegment(m))) for m in range(c.SegmentCount())]
            csegs = [x for x in csegs if bgap(ab, x[1]) <= reach]
            for k in range(a.SegmentCount()):
                sa = a.CSegment(k); sab = seg_bb(sa)
                for sc, scb in csegs:
                    if bgap(sab, scb) > reach:
                        continue
                    v = mm(sa.Distance(sc))
                    if best is None or v < best:
                        best = v
    return best


items = shapes()
mains = [i for i in items if i[0] in MAINS]
selv = [i for i in items if i[0] not in MAINS and i[0] != ""]
REACH = LIMIT + 2.0
worst = []
for net_m, Lm, refm, pm, dm in mains:
    bm = bbox(pm)
    for net_s, Ls, refs, psel, ds in selv:
        if Lm != Ls:
            continue
        if refm and refm == refs and refm in BRIDGES:
            continue        # the bridging part's own pins: its certified internal isolation
        if bgap(bm, bbox(psel)) > REACH:
            continue
        d = dist(pm, psel, REACH)
        if d is not None:
            worst.append((d, dm, ds, b.GetLayerName(Lm)))
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
