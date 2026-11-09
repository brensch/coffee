#!/usr/bin/env python3
"""Holder for the controller board (v40 holder, v39 board): the user's roof design (2026-11-08).

One printed part: a back plate that rises above the board and turns into a roof overhanging outward, two side
walls, and a deep solid front wall that stops below the spade row. The four mains wires leave straight out of
their receptacles through the forward-facing slot between the front wall's top and the roof; drips from above
hit the roof and run off its front edge, outside the front wall. The front wall has no holes. Everything else
leaves through the open bottom.

The board slides in from the BOTTOM, up the two side grooves, until its top edge enters a channel under the roof
that grips it front and back. Two printed pegs, perpendicular to the steel wall, push through slots in the front
wall, under the board's bottom edge, into blind pockets in the back plate: the board stands on them. The spade
receptacles go on after, through the slot; the top channel, 3-5 mm above the tabs, takes their push and pull.

    ~/opt/holder-venv/bin/python holder.py   # -> out/holder.stl, out/holder_peg.stl (two pegs), .step files, fit report

Frame (mm): X along the board (board x), Y up (Y = BOARD_H - board y; board bottom edge at Y = 0), Z out of the
steel panel (Z = 0 is the panel). Board inputs: _court/_tht/_parts/_mains_pads_v39.json from
hardware/controller/versions/v39-front-plate.
"""
import json, os, sys
from build123d import Box, Cylinder, Pos, export_stl, export_step

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out"); os.makedirs(OUT, exist_ok=True)
VER = "v39"                    # the board this holder is for

# ---- board ----
BOARD_W, BOARD_H, BOARD_T = 80.0, 77.0, 1.6
# ---- holder ----
BACK_T = 4.0                   # back plate: insulation from the steel, magnet pockets
STANDOFF = 4.0                 # room for through-hole pin tails (<= 3 mm) behind the board
ZB = BACK_T + STANDOFF         # board's back face
ZF = ZB + BOARD_T              # board's front face
FRONT_Z0, FRONT_T = 42.0, 2.5  # front wall: clears TE's longest receptacle (~30 mm off the board -> Z 39.6)
FRONT_TOP = 66.0               # front wall's top edge: below the receptacles (Y 70-75)
ROOF_Y, ROOF_T = 88.0, 2.5     # roof underside and thickness
ROOF_OVER = 7.5                # roof overhang past the front wall, so its drips fall outside
CLR = 0.25                     # groove clearance each side of the board and at its edge
GROOVE = 1.5                   # grooves grip the board's edges (the board keeps 2.5 mm side strips clear)
SIDE_T = 3.0
TOP_LIP = 1.5                  # top channel: lips over the board's top strip, front and back (tab bases start 2.95 mm down)
BOTTOM = -6.0                  # walls reach this far below the board's bottom edge (the pegs live here)
PEG = 2.6                      # peg section (square); slots PEG + 0.2
PEG_X = (4.0, 77.2)            # under the board's bottom edge, clear of the USB-C, the GH plugs and the thermocouple wires
PEG_Y = -PEG / 2               # peg top flush with the board's bottom edge
PEG_POCKET = 2.5               # blind pocket depth in the back plate (1.5 mm of plate left toward the steel)
RIB_W = 2.0
RIB_X = [5.0, 12.0, 26.0, 31.0, 43.2, 55.25, 59.0, 74.0]
PIN_TAIL, PIN_MARGIN = 3.0, 0.6
MAG_D, MAG_T = 10.0, 3.0       # magnets: 10 x 3 mm discs, H/SH grade
MAG_POCKET_D, MAG_POCKET_T = MAG_D + 0.25, MAG_T + 0.15
MAGNETS = [(10.0, 10.0), (40.0, 10.0), (68.0, 6.0), (10.0, 60.0), (40.0, 60.0), (40.0, 35.0)]
TAB_H = 14.02
RECEPT = (10.4, 5.0, 30.0)     # TE C-520140 maxima: width .409 in, body .195 in; ~30 mm off the board on the tab

X0, X1 = -CLR - SIDE_T, BOARD_W + CLR + SIDE_T
ZW = FRONT_Z0 + FRONT_T
ROOF_TOP = ROOF_Y + ROOF_T


def box(x0, y0, z0, x1, y1, z1):
    return Pos((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2) * Box(x1 - x0, y1 - y0, z1 - z0)


def load(name):
    return json.load(open(os.path.join(HERE, f"_{name}_{VER}.json")))


COURT = {r: (x0, BOARD_H - y1, x1, BOARD_H - y0) for r, (x0, y0, x1, y1) in load("court").items()}
PINS = [(r, x, BOARD_H - y, d) for r, x, y, d in load("tht")]
TABS = {r: ((COURT[r][0] + COURT[r][2]) / 2, (COURT[r][1] + COURT[r][3]) / 2) for r in ("J12", "J13", "J14", "J15")}
GH = ["J2", "J6", "J7", "J10"]


def rib_span(xc):
    """The board slides UP, so a pin tail sweeps everything below its seated place: a rib in its lane must start above it."""
    lo = 1.0
    for _, x, y, d in PINS:
        if abs(x - xc) < RIB_W / 2 + d / 2 + PIN_MARGIN:
            lo = max(lo, y + d / 2 + PIN_MARGIN)
    return lo, BOARD_H - TOP_LIP - 0.3


def holder():
    part = box(X0, BOTTOM, 0, X1, ROOF_TOP, BACK_T)                          # back plate, up to the roof
    part += box(X0, ROOF_Y, 0, X1, ROOF_TOP, ZW + ROOF_OVER)                 # roof
    part += box(X0, BOTTOM, 0, GROOVE, ROOF_TOP, ZW)                         # left wall, full depth up to the roof
    part += box(BOARD_W - GROOVE, BOTTOM, 0, X1, ROOF_TOP, ZW)               # right wall
    part += box(X0, BOTTOM, FRONT_Z0, X1, FRONT_TOP, ZW)                     # front wall, solid
    part += box(0, BOARD_H - TOP_LIP, BACK_T, BOARD_W, ROOF_Y, ZF + CLR + 1.6)   # top channel block under the roof
    ribs = []
    for xc in RIB_X:
        lo, hi = rib_span(xc)
        if hi - lo > 4:
            part += box(xc - RIB_W / 2, lo, BACK_T, xc + RIB_W / 2, hi, ZB)
            ribs.append((xc, round(lo, 1), round(hi, 1)))
    gz0, gz1 = ZB - CLR, ZF + CLR
    part -= box(-CLR, BOTTOM - 1, gz0, GROOVE, BOARD_H + CLR, gz1)                       # side grooves, open at the bottom
    part -= box(BOARD_W - GROOVE, BOTTOM - 1, gz0, BOARD_W + CLR, BOARD_H + CLR, gz1)
    part -= box(-CLR, BOARD_H - TOP_LIP - 0.01, gz0, BOARD_W + CLR, BOARD_H + CLR, gz1)  # top channel: the board's top edge
    part -= box(0, BOTTOM - 1, ZF, BOARD_W, BOARD_H - TOP_LIP, FRONT_Z0)                 # the cavity in front of the board
    part -= box(0, BOARD_H - TOP_LIP, ZF + CLR + 1.6, BOARD_W, ROOF_Y, FRONT_Z0 + 0.01)  # above the channel, under the roof
    part -= box(0, FRONT_TOP, FRONT_Z0 - 0.01, BOARD_W, ROOF_Y, ZW + 0.01)               # the wire slot (forward facing)
    for mx, my in MAGNETS:
        part -= Pos(mx, my, MAG_POCKET_T / 2 - 0.001) * Cylinder(MAG_POCKET_D / 2, MAG_POCKET_T)
    s = PEG + 0.2
    for px in PEG_X:                                                         # peg slots: front wall through, back plate blind
        part -= box(px - s / 2, PEG_Y - s / 2, FRONT_Z0 - 0.01, px + s / 2, PEG_Y + s / 2, ZW + 0.01)
        part -= box(px - s / 2, PEG_Y - s / 2, BACK_T - PEG_POCKET, px + s / 2, PEG_Y + s / 2, BACK_T + 0.01)
    return part, ribs


def pegs():
    out = None
    for px in PEG_X:
        p = box(px - PEG / 2, PEG_Y - PEG / 2, BACK_T - PEG_POCKET + 0.2, px + PEG / 2, PEG_Y + PEG / 2, ZW + 0.5)
        p += box(px - PEG, PEG_Y - PEG, ZW + 0.5, px + PEG, PEG_Y + PEG, ZW + 2.5)   # head
        out = p if out is None else out + p
    return out


HEIGHT_FIX = {"U8": 4.5, "U9": 4.5, "F1": 4.5, "J2": 4.25, "J10": 4.25}


def part_boxes():
    out = []
    for x0, y0, z0, x1, y1, z1, name in json.load(open(os.path.join(HERE, f"_parts_{VER}.json"))):
        if name in TABS:
            continue
        if name in HEIGHT_FIX:
            z1 = BOARD_T + HEIGHT_FIX[name]
        out.append((name, x0, BOARD_H - y1, max(ZB + z0, ZF), x1, BOARD_H - y0, ZB + z1))
    for r, (tx, ty) in TABS.items():
        out.append((f"{r} tab", tx - 3.25, ty - 0.45, ZF, tx + 3.25, ty + 0.45, ZF + TAB_H))
        out.append((f"{r} tab base", tx - 3.95, ty - 1.55, ZF, tx + 3.95, ty + 1.55, ZF + 2.0))
    for r, x, y, d in PINS:
        out.append((f"{r} pin tail", x - d / 2, y - d / 2, ZB - PIN_TAIL, x + d / 2, y + d / 2, ZB))
    return out


def plugs():
    out = []
    for r in GH:
        x0, _, x1, _ = COURT[r]
        out.append((f"{r} plug", x0 + 0.6, -8.0, ZF + 0.05, x1 - 0.6, 0.0, ZF + 4.6))
        out.append((f"{r} lead", x0 + 1.5, -60.0, ZF + 1.0, x1 - 1.5, -8.0, ZF + 3.5))
    for r, x, y, d in PINS:
        if r == "J5":
            out.append(("thermocouple wire", x - 0.8, -60.0, ZF + 1.5, x + 0.8, y, ZF + 3.5))
    return out


def fit_check(h, pg):
    bad = []
    hit = lambda solid, bx: (solid & box(*bx)).volume > 1e-3
    for name, x0, y0, z0, x1, y1, z1 in part_boxes() + plugs():          # the board comes in from below
        for label, bot in (("seated", y0), ("sliding in", y0 - 300)):
            if hit(h, (x0, bot, z0, x1, y1, z1)):
                bad.append(f"{name} touches the holder ({label}) at X {x0:.1f}..{x1:.1f}")
                break
    if hit(h, (0, -300, ZB, BOARD_W, BOARD_H, ZF)):
        bad.append("the board itself touches the holder")
    for r, (tx, ty) in TABS.items():                                      # pushed on through the slot, straight in
        rb = (tx - RECEPT[0] / 2, ty - RECEPT[1] / 2, ZF + 3.0, tx + RECEPT[0] / 2, ty + RECEPT[1] / 2, ZF + RECEPT[2] + 60)
        if hit(h, rb):
            bad.append(f"{r} receptacle can't go on through the slot")
    for name, x0, y0, z0, x1, y1, z1 in part_boxes() + plugs():
        if hit(pg, (x0, y0, z0, x1, y1, z1)):
            bad.append(f"a peg touches {name}")
    if hit(pg, (0, 0.001, ZB, BOARD_W, BOARD_H, ZF)):
        bad.append("a peg cuts into the board")
    if not hit(pg, (0, -0.2, ZB, BOARD_W, 0.0, ZF)):
        bad.append("the pegs don't reach under the board")
    if not hit(h, (0, BOARD_H + CLR + 0.01, ZB, BOARD_W, BOARD_H + 300, ZF)):
        bad.append("nothing stops the board at the top")
    if not hit(h, (0, BOARD_H - TOP_LIP + 0.1, ZF + CLR + 0.05, BOARD_W, BOARD_H - 0.1, ZF + CLR + 1.0)):
        bad.append("no lip in front of the board's top edge (pull-off support)")
    return bad


def exposed_mains():
    """Mains pads visible straight through the wire slot (everything else is behind the solid front wall)."""
    out = []
    for r, num, net, x0, y0, x1, y1 in load("mains_pads"):
        cy = BOARD_H - (y0 + y1) / 2
        if r != "PS1" and cy > FRONT_TOP:
            out.append(f"{r}.{num} ({net})")
    return sorted(set(out))


if __name__ == "__main__":
    h, ribs = holder()
    pg = pegs()
    export_stl(h, os.path.join(OUT, "holder.stl")); export_step(h, os.path.join(OUT, "holder.step"))
    export_stl(pg, os.path.join(OUT, "holder_peg.stl")); export_step(pg, os.path.join(OUT, "holder_peg.step"))
    for f in ("holder_bar.stl", "holder_bar.step"):
        if os.path.exists(os.path.join(OUT, f)):
            os.remove(os.path.join(OUT, f))
    bb = h.bounding_box()
    print(f"holder {bb.size.X:.1f} x {bb.size.Y:.1f} x {bb.size.Z:.1f} mm, {h.volume / 1000:.1f} cm3 "
          f"(front wall to {ZW} mm off the panel, roof to {ZW + ROOF_OVER} mm, wire slot Y {FRONT_TOP}-{ROOF_Y})")
    print("ribs (x, from Y, to Y):", ribs)
    print("mains pads seen through the wire slot (under the roof):", exposed_mains() or "none")
    bad = fit_check(h, pg)
    print("fit: OK" if not bad else "fit problems:\n  " + "\n  ".join(bad))
    sys.exit(1 if bad else 0)
