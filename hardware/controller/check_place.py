#!/usr/bin/env python3
"""Check a placement before routing it: courtyard overlaps, parts off the board, and a picture
with every part's courtyard, pads, reference and the ratsnest (the straight lines between pads
that must connect), so the placement can be judged by eye. It moves nothing.

    source ~/opt/kienv.sh && PLACE_JSON=plan.json python3 check_place.py out.png
"""
import json, math, os, subprocess, sys

import layout, pcbnew

out = sys.argv[1] if len(sys.argv) > 1 else "place.png"
spec = json.load(open(os.path.join(layout.BUILD, "parts-120v.json")))
board = layout.build(spec, 120)
W, H = layout.W, layout.H
fps = list(board.GetFootprints())
if os.environ.get("PADS_OUT"):          # optional: every pad's net and position, to judge placement by
    with open(os.environ["PADS_OUT"], "w") as f:
        for fp in fps:
            for p in fp.Pads():
                if p.GetNetname():
                    q = p.GetPosition()
                    f.write(f"{fp.GetReference()} {p.GetNumber()} {p.GetNetname()} {pcbnew.ToMM(q.x):.2f} {pcbnew.ToMM(q.y):.2f}\n")
for fp in fps:
    fp.BuildCourtyardCaches()


def cy(fp):
    b = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd).BBox()
    return [pcbnew.ToMM(v) for v in (b.GetLeft(), b.GetTop(), b.GetRight(), b.GetBottom())]


boxes = {fp.GetReference(): cy(fp) for fp in fps}
side = {fp.GetReference(): "B" if fp.IsFlipped() else "F" for fp in fps}
problems = []
refs = sorted(boxes)
for i, a in enumerate(refs):
    A = boxes[a]
    for b in refs[i + 1:]:
        if side[a] != side[b]:
            continue                      # top and bottom parts may overlap in plan view
        B = boxes[b]
        ox, oy = min(A[2], B[2]) - max(A[0], B[0]), min(A[3], B[3]) - max(A[1], B[1])
        if ox > 0.01 and oy > 0.01:
            problems.append(f"overlap {a}/{b}: {ox:.2f} x {oy:.2f} mm")
pads = []
for fp in fps:
    for p in fp.Pads():
        bb = p.GetBoundingBox()
        x0, y0, x1, y1 = (pcbnew.ToMM(v) for v in (bb.GetLeft(), bb.GetTop(), bb.GetRight(), bb.GetBottom()))
        if x0 < 0.3 or y0 < 0.3 or x1 > W - 0.3 or y1 > H - 0.3:
            problems.append(f"pad {fp.GetReference()}.{p.GetNumber()} within 0.3 mm of the edge")
        pads.append((fp.GetReference(), p.GetNumber(), p.GetNetname(), (x0 + x1) / 2, (y0 + y1) / 2, x1 - x0, y1 - y0))

# ratsnest: minimum spanning tree per net
nets = {}
for r, n, net, x, y, w, h in pads:
    if net:
        nets.setdefault(net, []).append((x, y))
lines = []
for net, pts in nets.items():
    if len(pts) < 2:
        continue
    used, d, par = [False] * len(pts), [math.inf] * len(pts), [-1] * len(pts)
    d[0] = 0
    for _ in pts:
        i = min((k for k in range(len(pts)) if not used[k]), key=d.__getitem__)
        used[i] = True
        if par[i] >= 0:
            lines.append((net, pts[par[i]], pts[i]))
        for k in range(len(pts)):
            if not used[k]:
                dd = math.dist(pts[i], pts[k])
                if dd < d[k]:
                    d[k], par[k] = dd, i
total = sum(math.dist(a, b) for n, a, b in lines if n != "GND")

S = 22
svg = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{W * S + 40}" height="{H * S + 40}" viewBox="-1 -1 {W + 2} {H + 2}">',
       f'<rect x="-1" y="-1" width="{W + 2}" height="{H + 2}" fill="#fff"/>',
       f'<rect x="0" y="0" width="{W}" height="{H}" fill="#f4f1e8" stroke="#000" stroke-width="0.1"/>']
bad = {p.split()[1] for p in problems if p.startswith("overlap")}
bad = {r for pair in bad for r in pair.rstrip(":").split("/")}
for ref, (x0, y0, x1, y1) in boxes.items():
    col = "#d22" if ref in bad else ("#69c" if side[ref] == "B" else "#999")
    svg.append(f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" fill="none" stroke="{col}" stroke-width="0.06"/>')
for r, n, net, x, y, w, h in pads:
    col = "#c33" if net == "GND" else "#b58900" if net in ("+5V", "+3V3", "VBUS", "+5V_IN") else "#268bd2" if net else "#bbb"
    svg.append(f'<rect x="{x - w / 2}" y="{y - h / 2}" width="{w}" height="{h}" fill="{col}" opacity="0.8"/>')
for net, a, b in lines:
    col = "#e88" if net == "GND" else "#2a2"
    svg.append(f'<line x1="{a[0]}" y1="{a[1]}" x2="{b[0]}" y2="{b[1]}" stroke="{col}" stroke-width="0.07" opacity="0.8"/>')
for ref, (x0, y0, x1, y1) in boxes.items():
    svg.append(f'<text x="{(x0 + x1) / 2}" y="{(y0 + y1) / 2 + 0.3}" font-size="0.9" text-anchor="middle" font-family="sans-serif" font-weight="bold" fill="#000" opacity="0.75">{ref}</text>')
svg.append("</svg>")
svgp = out[:-4] + ".svg"
open(svgp, "w").write("".join(svg))
subprocess.run(["google-chrome", "--headless=new", "--disable-gpu", "--no-sandbox", f"--window-size={int(W * S + 60)},{int(H * S + 60)}",
                f"--screenshot={os.path.abspath(out)}", "file://" + os.path.abspath(svgp)], capture_output=True, timeout=60)
print(f"{W} x {H} = {W * H:.0f} mm2, {len(problems)} problems, ratsnest (no GND) {total:.0f} mm")
for p in problems:
    print("  " + p)
print(out)
