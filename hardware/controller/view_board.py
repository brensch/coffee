#!/usr/bin/env python3
"""Labelled PNG of a routed board for review: top and bottom renders side by side (bottom mirrored as
seen from below, like the fab render), every part's reference drawn at its position.

    source ~/opt/kienv.sh && python3 view_board.py BUILD_DIR [out.png]
"""
import os, re, subprocess, sys

import pcbnew

d = sys.argv[1]
out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(d, "review.png")
stem = os.path.join(d, "coffee-controller-120v")
b = pcbnew.LoadBoard(stem + ".kicad_pcb")
bb = b.GetBoardEdgesBoundingBox()
W, H = pcbnew.ToMM(bb.GetWidth()), pcbnew.ToMM(bb.GetHeight())
x0, y0 = pcbnew.ToMM(bb.GetX()), pcbnew.ToMM(bb.GetY())


def vb(svg):
    return [float(v) for v in re.search(r'viewBox="([^"]+)"', open(svg).read(2000)).group(1).split()]


S = 18  # px per mm
html = ["<html><body style='margin:0;background:#fff;font:bold 12px sans-serif'><div style='display:flex;gap:20px;padding:10px'>"]
for side in ("top", "bottom"):
    svg = f"{stem}-{side}.svg"
    v = vb(svg)
    ox, oy = (v[2] - W) / 2 - x0, (v[3] - H) / 2 - y0
    labels = []
    for fp in b.GetFootprints():
        on = fp.IsFlipped() == (side == "bottom")
        p = fp.GetPosition()
        x, y = pcbnew.ToMM(p.x) + ox, pcbnew.ToMM(p.y) + oy
        if side == "bottom":
            x = v[2] - x
        col = "#c00" if on else "#999"
        labels.append(f"<div style='position:absolute;left:{x * S}px;top:{y * S}px;transform:translate(-50%,-50%);"
                      f"color:{col};background:rgba(255,255,255,{0.85 if on else 0.4});padding:0 2px;border-radius:3px;"
                      f"{'' if on else 'font-weight:normal;font-size:10px;'}'>{fp.GetReference()}</div>")
    html.append(f"<div><div style='font-size:16px'>{side}{' (seen from below)' if side == 'bottom' else ''}</div>"
                f"<div style='position:relative;width:{v[2] * S}px;height:{v[3] * S}px'>"
                f"<img src='file://{svg}' style='width:100%;height:100%'>{''.join(labels)}</div></div>")
html.append("</div></body></html>")
page = os.path.join(d, "review.html")
open(page, "w").write("".join(html))
w = int(2 * (W + 6) * S + 60)
h = int((H + 6) * S + 60)
subprocess.run(["google-chrome", "--headless=new", "--disable-gpu", "--no-sandbox", "--allow-file-access-from-files",
                f"--window-size={w},{h}", f"--screenshot={out}", "file://" + page], capture_output=True, timeout=60)
print(out)
