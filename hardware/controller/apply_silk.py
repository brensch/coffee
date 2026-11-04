#!/usr/bin/env python3
"""Redraw the board-level silkscreen (labels, band marks, logo) on a finished board, no reroute.

The old board-level F.SilkS graphics are cut from the file text (the KiCad 7 Python bindings return
untyped objects after BOARD.Remove()), then the board is loaded and silkscreen() draws afresh.

    source ~/opt/kienv.sh && PLACE_JSON=plan.json python3 apply_silk.py build/coffee-controller-120v.kicad_pcb
"""
import os, sys
os.environ.setdefault("PLACE_ONLY", "1")
import layout, pcbnew

pcb = sys.argv[1]
txt = open(pcb).read()
out, i, gone = [], 0, 0
# walk the top-level items of (kicad_pcb ...): drop gr_* graphics on F.SilkS
depth, start = 0, None
for j, ch in enumerate(txt):
    if ch == "(":
        depth += 1
        if depth == 2:
            start = j
    elif ch == ")":
        if depth == 2 and start is not None:
            item = txt[start:j + 1]
            if item.startswith(("(gr_text", "(gr_line", "(gr_circle", "(gr_arc", "(gr_poly", "(gr_rect")) and '(layer "F.SilkS")' in item:
                out.append(txt[i:start]); i = j + 1; gone += 1
            start = None
        depth -= 1
out.append(txt[i:])
open(pcb, "w").write("".join(out))
b = pcbnew.LoadBoard(pcb)
layout.silkscreen(b, 120)
pcbnew.SaveBoard(pcb, b)
b = pcbnew.LoadBoard(pcb)                  # reload so the rules apply to fill + DRC
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
pcbnew.SaveBoard(pcb, b)
stem = pcb[:-len(".kicad_pcb")]
pcbnew.WriteDRCReport(b, stem + "-drc.rpt", pcbnew.EDA_UNITS_MILLIMETRES, True)
layout.fab_outputs(pcb, stem)
print(f"replaced {gone} silkscreen drawings; DRC {stem}-drc.rpt")
