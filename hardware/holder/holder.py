#!/usr/bin/env python3
"""Holder for the controller board (v39): one printed part, a back plate and a front plate joined by the two side
walls, open at the top. The board slides down between them in two edge grooves and stands on the floor.
The mains spades along the top poke out through a comb in the front plate, the IRM supply through a channel,
and the low-voltage plugs along the bottom hang out of the open bottom. A printed bar through both side walls,
just above the board's top edge, stops the board being pulled up. Six magnets in the back hold it to the
machine's steel side panel.

    ~/opt/holder-venv/bin/python holder.py      # -> out/holder.stl, out/holder_bar.stl, .step files, fit report

Frame (mm): X along the board (board x), Y up (Y = BOARD_H - board y; the board's bottom edge at Y = 0),
Z out of the steel panel (Z = 0 is the panel).

Every opening in the front plate is computed from the board (courtyards and pads exported from
hardware/controller/versions/v39-front-plate). The fit check then confirms, by solid intersection:
  - nothing on the board, and no pin tail on its back, touches the holder on the way in or when seated;
  - the low-voltage plugs (plugged in before sliding) and the thermocouple wires pass and end clear;
  - each spade receptacle sits in its comb slot without touching;
  - a screwdriver reaches the terminal's screws;
  - the retaining bar clears everything;
and lists every mains pad the front plate leaves uncovered (only the four spade tabs should be).
"""
import json, os, sys
from build123d import Box, Cylinder, Pos, export_stl, export_step

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out"); os.makedirs(OUT, exist_ok=True)
VER = "v39"

# ---- board ----
BOARD_W, BOARD_H, BOARD_T = 80.0, 77.0, 1.6
# ---- holder ----
BACK_T = 4.0                  # back plate: insulation from the steel, and the magnet pockets
STANDOFF = 4.0                # back plate to the board's back face: room for through-hole pin tails (<= 3 mm)
ZB = BACK_T + STANDOFF        # board's back face
ZF = ZB + BOARD_T             # board's front face
PLATE_GAP = 5.5               # board's front face to the front plate (tallest covered part: optos, ~4.5)
PLATE_T = 2.0
ZP0, ZP1 = ZF + PLATE_GAP, ZF + PLATE_GAP + PLATE_T
CLR = 0.25                    # groove clearance, each side of the board and at its edge
GROOVE = 1.5                  # how far the grooves grip the board's edge (the board keeps 2.5 mm clear)
SIDE_T = 3.0                  # side wall outside the groove
FLOOR_T = 3.0                 # floor under the board's bottom edge (behind the board only: front is open)
TOP_EXTRA = 5.0               # side walls rise this far above the board's top edge, for the retaining bar
BAR = 3.2                     # retaining bar section (square); holes BAR + 0.15: a snug push fit, so pump vibration can't walk it out
BAR_Y = BOARD_H + 0.4 + BAR / 2
BAR_Z = (ZB + ZF) / 2
RIB_W = 2.4
RIB_X = [6.0, 14.5, 22.0, 27.0, 35.0, 41.0, 47.5, 54.5, 61.0, 63.0]   # lanes clear of the USB-C shell pins at the bottom
PIN_TAIL = 3.0
PIN_MARGIN = 0.6
OPEN_MARGIN = 0.6             # front-plate openings: courtyard plus this
MAG_D, MAG_T = 10.0, 3.0      # magnets: 10 x 3 mm discs
MAG_POCKET_D, MAG_POCKET_T = MAG_D + 0.25, MAG_T + 0.15
MAGNETS = [(10.0, 12.0), (40.0, 12.0), (70.0, 12.0), (10.0, 62.0), (40.0, 62.0), (72.0, 52.0)]   # top-right one kept clear of the IRM's AC pins
TAB_H = 14.02                 # TE 63824-1 tab height
RECEPT = (10.4, 5.0, 30.0)    # fully insulated 6.3 mm receptacle on a tab, TE C-520140 maxima: width .409 in, body .195 in;
                              # 23.7 mm long, so with ~8 mm engaged on the 14 mm tab it reaches ~30 mm off the board
SLOT_W = 11.6                 # comb slot width: the receptacle plus ~0.6 mm a side for print shrinkage

X0, X1 = -CLR - SIDE_T, BOARD_W + CLR + SIDE_T
Y0, Y1 = -FLOOR_T, BOARD_H + TOP_EXTRA


def box(x0, y0, z0, x1, y1, z1):
    return Pos((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2) * Box(x1 - x0, y1 - y0, z1 - z0)


def load(name):
    return json.load(open(os.path.join(HERE, f"_{name}_{VER}.json")))


COURT = {r: (x0, BOARD_H - y1, x1, BOARD_H - y0) for r, (x0, y0, x1, y1) in load("court").items()}   # holder X/Y
PINS = [(r, x, BOARD_H - y, d) for r, x, y, d in load("tht")]
TABS = {r: ((COURT[r][0] + COURT[r][2]) / 2, (COURT[r][1] + COURT[r][3]) / 2) for r in ("J12", "J13", "J14", "J15")}
GH = ["J2", "J6", "J7", "J10"]          # low-voltage plugs on the bottom edge (USB J4 is only for recovery)


def openings():
    """Front-plate openings as (name, x0, y0, x1, y1), holder coordinates; all run up to the top edge or the floor."""
    m = OPEN_MARGIN
    ps, j5 = COURT["PS1"], COURT["J5"]
    ops = [("supply channel", ps[0] - m, ps[1] - m, BOARD_W - GROOVE, Y1 + 1),
           ("terminal column", j5[0] - m, -0.3, BOARD_W - GROOVE, Y1 + 1)]   # it slides the full height
    for r, (tx, ty) in TABS.items():
        ops.append((f"{r} comb slot", tx - SLOT_W / 2, ty - RECEPT[1] / 2 - 0.5, tx + SLOT_W / 2, Y1 + 1))
    xs = [COURT[f"D{i}"][0] for i in (1, 3, 4, 5, 6, 7, 8)] + [COURT[f"D{i}"][2] for i in (1, 3, 4, 5, 6, 7, 8)]
    ys = [COURT[f"D{i}"][1] for i in (1, 8)] + [COURT[f"D{i}"][3] for i in (1, 8)]
    ops.append(("LED window", min(xs) - 0.3, min(ys) - 0.3, max(xs) + 0.3, max(ys) + 0.3))
    return ops


def rib_top(xc):
    top = BOARD_H - 0.5
    for _, x, y, d in PINS:
        if abs(x - xc) < RIB_W / 2 + d / 2 + PIN_MARGIN:
            top = min(top, y - d / 2 - PIN_MARGIN)
    return top


def holder():
    part = box(X0, Y0, 0, X1, Y1, BACK_T)                                   # back plate
    part += box(X0, Y0, 0, GROOVE, Y1, ZP1)                                # left wall
    part += box(BOARD_W - GROOVE, Y0, 0, X1, Y1, ZP1)                      # right wall
    part += box(X0, Y0, 0, X1, 0.0, ZF)                                    # floor behind the board: it stands on it
    part += box(X0, Y0, ZP0, X1, BOARD_H, ZP1)                             # front plate, down to the floor: printed
    # floor-down it starts on the bed, and its strip below the board's edge ties it to the right wall (the terminal
    # column and the supply channel cut it off from that wall everywhere above)
    ribs = []
    for xc in RIB_X:
        top = rib_top(xc)
        if top > 6:
            part += box(xc - RIB_W / 2, 0.0, BACK_T, xc + RIB_W / 2, top, ZB)
            ribs.append((xc, round(top, 1)))
    gz0, gz1 = ZB - CLR, ZF + CLR                                          # grooves, open at the top
    part -= box(-CLR, 0.0, gz0, GROOVE, Y1 + 1, gz1)
    part -= box(BOARD_W - GROOVE, 0.0, gz0, BOARD_W + CLR, Y1 + 1, gz1)
    part -= box(0, 0.0, ZF, BOARD_W, Y1 + 1, ZP0)                          # the space in front of the board
    part -= box(0, BOARD_H, ZB - CLR, BOARD_W, Y1 + 1, ZP0)                # above the board (receptacle wires)
    for _, x0, y0, x1, y1 in openings():
        part -= box(x0, y0, ZP0 - 0.01, x1, y1, ZP1 + 0.01)
    for dx in [8.0 + 9.0 * i for i in range(8)]:                          # drain slots: the floor behind the board
        part -= box(dx - 2.5, Y0 - 1, BACK_T + 0.6, dx + 2.5, 0.01, ZB - CLR - 0.6)   # would trap water otherwise
    for mx, my in MAGNETS:                                                 # magnet pockets, open on the back face
        part -= Pos(mx, my, MAG_POCKET_T / 2 - 0.001) * Cylinder(MAG_POCKET_D / 2, MAG_POCKET_T)
    h = BAR + 0.15                                                         # retaining-bar holes through both walls
    part -= box(X0 - 1, BAR_Y - h / 2, BAR_Z - h / 2, X1 + 1, BAR_Y + h / 2, BAR_Z + h / 2)
    return part, ribs


def bar():
    """The retaining bar: pushes through both walls above the board's top edge; a head stops it at one side."""
    b = box(X0 - 0.5, BAR_Y - BAR / 2, BAR_Z - BAR / 2, X1 + 0.5, BAR_Y + BAR / 2, BAR_Z + BAR / 2)
    b += box(X1 + 0.5, BAR_Y - BAR, BAR_Z - BAR, X1 + 3.0, BAR_Y + BAR, BAR_Z + BAR)
    return b


# Stand-in boxes the 3D model gets wrong: real heights from datasheets (GH side-entry header 4.25 mm; SMDIP-6 ~3.9
# and Nano2 443 ~3.4, rounded up). The spade tabs are replaced by their real shape below.
HEIGHT_FIX = {"U8": 4.5, "U9": 4.5, "F1": 4.5, "J2": 4.25, "J10": 4.25}


def part_boxes():
    out = []
    for x0, y0, z0, x1, y1, z1, name in json.load(open(os.path.join(HERE, f"_parts_{VER}.json"))):
        if name in TABS:
            continue                                   # stand-in block; the real tab follows
        if name in HEIGHT_FIX:
            z1 = BOARD_T + HEIGHT_FIX[name]
        out.append((name, x0, BOARD_H - y1, max(ZB + z0, ZF), x1, BOARD_H - y0, ZB + z1))
    for r, (tx, ty) in TABS.items():                   # TE 63824-1: a 6.35 x 0.8 blade 14 mm tall on a 7.9 x 3.1 base
        out.append((f"{r} tab", tx - 3.25, ty - 0.45, ZF, tx + 3.25, ty + 0.45, ZF + TAB_H))
        out.append((f"{r} tab base", tx - 3.95, ty - 1.55, ZF, tx + 3.95, ty + 1.55, ZF + 2.0))
    for r, x, y, d in PINS:
        out.append((f"{r} pin tail", x - d / 2, y - d / 2, ZB - PIN_TAIL, x + d / 2, y + d / 2, ZB))
    return out


def plugs():
    """Mated GH plugs and the thermocouple wires: past the bottom edge, in front of the floor, leads hanging down."""
    out = []
    for r in GH:
        x0, _, x1, _ = COURT[r]
        out.append((f"{r} plug", x0 + 0.6, -8.0, ZF + 0.05, x1 - 0.6, 0.0, ZF + 4.6))
        out.append((f"{r} lead", x0 + 1.5, -60.0, ZF + 1.0, x1 - 1.5, -8.0, ZF + 3.5))
    for r, x, y, d in PINS:
        if r == "J5":
            out.append(("thermocouple wire", x - 0.8, -60.0, ZF + 1.5, x + 0.8, y, ZF + 3.5))
    return out


def fit_check(h, b):
    bad = []
    hit = lambda solid, bx: (solid & box(*bx)).volume > 1e-3
    for name, x0, y0, z0, x1, y1, z1 in part_boxes() + plugs():
        for label, top in (("seated", y1), ("sliding in", y1 + 300)):
            if hit(h, (x0, y0, z0, x1, top, z1)):
                bad.append(f"{name} touches the holder ({label}) at X {x0:.1f}..{x1:.1f}")
                break
    if hit(h, (0, 0, ZB, BOARD_W, BOARD_H + 300, ZF)):
        bad.append("the board itself touches the holder")
    for r, (tx, ty) in TABS.items():
        rb = (tx - RECEPT[0] / 2, ty - RECEPT[1] / 2, ZF + 3.0, tx + RECEPT[0] / 2, ty + RECEPT[1] / 2, ZF + RECEPT[2])
        for label, up in (("seated", 0.0), ("sliding in with the board", 300.0)):   # fitted on the bench, before sliding
            if hit(h, (rb[0], rb[1], rb[2], rb[3], rb[4] + up, rb[5])):
                bad.append(f"{r} receptacle touches the holder ({label})")
                break
    j5 = COURT["J5"]                                                      # screwdriver straight down onto the screws
    if hit(h, (j5[0] + 1.5, j5[1] + 1.0, ZF + 11.0, j5[2] - 1.5, j5[3] - 1.0, ZF + 80)):
        bad.append("no screwdriver access to the thermocouple terminal")
    for name, x0, y0, z0, x1, y1, z1 in part_boxes():
        if hit(b, (x0, y0, z0, x1, y1, z1)):
            bad.append(f"retaining bar touches {name}")
    if hit(b, (0, 0, ZB, BOARD_W, BOARD_H, ZF)):
        bad.append("retaining bar touches the board")
    return bad


def exposed_mains():
    """Mains pads not under the front plate (and not under the IRM's own body)."""
    ops = openings()
    ps = COURT["PS1"]
    out = []
    for r, num, net, x0, y0, x1, y1 in load("mains_pads"):
        X0p, X1p, Y0p, Y1p = x0, x1, BOARD_H - y1, BOARD_H - y0
        cx, cy = (X0p + X1p) / 2, (Y0p + Y1p) / 2
        if r == "PS1":
            continue                                           # under the supply's own case
        for name, ox0, oy0, ox1, oy1 in ops:
            if ox0 < cx < ox1 and oy0 < cy < oy1:
                out.append(f"{r}.{num} ({net}) in the {name}")
                break
    return out


if __name__ == "__main__":
    h, ribs = holder()
    b = bar()
    export_stl(h, os.path.join(OUT, "holder.stl")); export_step(h, os.path.join(OUT, "holder.step"))
    export_stl(b, os.path.join(OUT, "holder_bar.stl")); export_step(b, os.path.join(OUT, "holder_bar.step"))
    bb = h.bounding_box()
    print(f"holder {bb.size.X:.1f} x {bb.size.Y:.1f} x {bb.size.Z:.1f} mm, {h.volume / 1000:.1f} cm3; front plate {ZP0:.1f}-{ZP1:.1f} mm off the panel")
    print("ribs (x, top Y):", ribs)
    print("openings:", [(n, round(x0, 1), round(y0, 1), round(x1, 1), round(min(y1, Y1), 1)) for n, x0, y0, x1, y1 in openings()])
    print("mains pads left uncovered:", exposed_mains() or "none")
    bad = fit_check(h, b)
    print("fit: OK" if not bad else "fit problems:\n  " + "\n  ".join(bad))
    sys.exit(1 if bad else 0)
