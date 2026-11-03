#!/usr/bin/env python3
"""Large-parts-first floorplanning: place only the parts that set the board's size (and the ones
you plug wires into), draw the isolation boundary, and check overlaps and band rules in seconds.
No routing. Used to choose the topology before layout.py does the real board.

    source ~/opt/kienv.sh && python3 floorplan.py plan.json out.png
plan.json: {"W": .., "H": .., "band": [[x, y], ...] (polygon, the 7 mm copper-free barrier),
            "parts": {"REF": [x, y, rot, "F"|"B"], ...}}
"""
import json, os, subprocess, sys, tempfile
import pcbnew
from pcbnew import FromMM as mm, VECTOR2I as V

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from layout import load_fp, seg, rect  # noqa: E402

plan = json.load(open(sys.argv[1]))
spec = json.load(open(os.path.join(HERE, "build", "parts-120v.json")))
mains_nets = set(spec["mains_nets"])
b = pcbnew.BOARD()
W, H = plan["W"], plan["H"]
seg(b, pcbnew.Edge_Cuts, rect(0, 0, W, H))
band = plan["band"]
seg(b, pcbnew.Dwgs_User, band, width=0.3)
boxes, problems = {}, []
for ref, (x, y, rot, side) in plan["parts"].items():
    fp = load_fp(spec["parts"][ref]["footprint"])
    fp.SetReference(ref)
    b.Add(fp)
    fp.SetPosition(V(mm(x), mm(y)))
    if side == "B":
        fp.Flip(fp.GetPosition(), False)
    fp.SetOrientationDegrees(rot)
    c = fp.GetCourtyard(pcbnew.B_CrtYd if side == "B" else pcbnew.F_CrtYd).BBox()
    boxes[ref] = (side, [pcbnew.ToMM(v) for v in (c.GetLeft(), c.GetTop(), c.GetRight(), c.GetBottom())])
    # which side of the barrier each pad is on: mains pads must be outside the band polygon on the mains side
    for pad in fp.Pads():
        p = (pcbnew.ToMM(pad.GetPosition().x), pcbnew.ToMM(pad.GetPosition().y))
        if not (0.5 <= p[0] <= W - 0.5 and 0.5 <= p[1] <= H - 0.5):
            problems.append(f"{ref}.{pad.GetNumber()} off the board at {p}")


def inside(poly, p):
    n, c = len(poly), False
    for i in range(n):
        (x1, y1), (x2, y2) = poly[i], poly[(i + 1) % n]
        if (y1 > p[1]) != (y2 > p[1]) and p[0] < (x2 - x1) * (p[1] - y1) / (y2 - y1) + x1:
            c = not c
    return c


for fp in b.GetFootprints():
    for pad in fp.Pads():
        p = (pcbnew.ToMM(pad.GetPosition().x), pcbnew.ToMM(pad.GetPosition().y))
        if inside(band, p):
            problems.append(f"{fp.GetReference()}.{pad.GetNumber()} pad inside the isolation band")
refs = list(boxes)
for i, a in enumerate(refs):
    for bb in refs[i + 1:]:
        sa, A = boxes[a]
        sb, Bx = boxes[bb]
        if sa != sb:
            continue
        if A[0] < Bx[2] and Bx[0] < A[2] and A[1] < Bx[3] and Bx[1] < A[3]:
            problems.append(f"overlap {a} / {bb}")
out = sys.argv[2]
with tempfile.TemporaryDirectory() as d:
    pcb = os.path.join(d, "fp.kicad_pcb")
    pcbnew.SaveBoard(pcb, b)
    subprocess.run([os.path.join(HERE, "render.sh"), pcb, out, "F.Cu,B.Cu,F.SilkS,Edge.Cuts,F.CrtYd,Dwgs.User", "1200"], check=False,
                   env=dict(os.environ, TMPDIR=os.environ.get("TMPDIR", d)))
area = W * H
print(f"board {W} x {H} = {area:.0f} mm2; problems: {len(problems)}")
for p in problems:
    print("  " + p)
