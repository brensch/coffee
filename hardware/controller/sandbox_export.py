#!/usr/bin/env python3
"""Export part geometry for the layout sandbox (out/view/sandbox.html).

For every part: courtyard box and pad offsets relative to its anchor at rotation 0 (top side),
pad nets, and whether each net is mains. Also the current placement from layout.py, so the
sandbox starts from the latest board.

    source ~/opt/kienv.sh && python3 sandbox_export.py   ->  ../../out/view/sandbox/parts.json
"""
import json, os, sys
import pcbnew

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
os.environ.setdefault("PLACE_ONLY", "1")
import layout  # noqa: E402

spec = json.load(open(os.path.join(HERE, "build", "parts-120v.json")))
mains = set(spec["mains_nets"])
board = pcbnew.BOARD()
parts = {}
for ref, p in spec["parts"].items():
    fp = layout.load_fp(p["footprint"])
    board.Add(fp)
    fp.SetPosition(pcbnew.VECTOR2I(0, 0))
    c = fp.GetCourtyard(pcbnew.F_CrtYd).BBox()
    if c.GetWidth() == 0:
        c = fp.GetBoundingBox(False, False)
    mm = pcbnew.ToMM
    pads = []
    for pad in fp.Pads():
        net = p["pins"].get(pad.GetNumber())
        pos, size = pad.GetPosition(), pad.GetBoundingBox()
        pads.append(dict(n=pad.GetNumber(), x=round(mm(pos.x), 3), y=round(mm(pos.y), 3),
                         w=round(mm(size.GetWidth()), 2), h=round(mm(size.GetHeight()), 2),
                         net=net, mains=bool(net in mains), tht=pad.GetAttribute() == pcbnew.PAD_ATTRIB_PTH))
    kind = "mains" if any(q["mains"] for q in pads) else "selv"
    if any(q["mains"] for q in pads) and any(q["net"] and not q["mains"] for q in pads):
        kind = "straddle"
    parts[ref] = dict(box=[round(mm(v), 3) for v in (c.GetLeft(), c.GetTop(), c.GetRight(), c.GetBottom())],
                      pads=pads, kind=kind, value=p["value"], desc=p["desc"], footprint=p["footprint"].split(":")[1])
place = {r: list(v) for r, v in layout.PLACE.items() if r in parts}
band = [[layout.BAND[0], 0], [layout.BAND[1], 0], [layout.BAND[1], layout.H], [layout.BAND[0], layout.H]]
out = dict(W=layout.W, H=layout.H, band=band, parts=parts, place=place)
dst = os.path.join(HERE, "..", "..", "out", "view", "sandbox")
os.makedirs(dst, exist_ok=True)
json.dump(out, open(os.path.join(dst, "parts.json"), "w"))
print(f"wrote {dst}/parts.json: {len(parts)} parts")
