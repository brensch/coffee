#!/usr/bin/env python3
"""Slide-in holder for the controller board (v38): a flat 3D-printed cradle that sticks to the machine's
steel side panel with magnets. The board slides down into two edge grooves, plugs up, and stops on the
floor; gravity keeps it there. Everything on the board is on its front face, so the back of the holder is
a plain plate.

    ~/opt/holder-venv/bin/python holder.py            # -> out/holder.stl, out/holder.step, fit report

Print it back-down (the magnet pockets on the bed side are fine: they're open on the back face), in PETG or
ASA (not PLA: the case runs 60-70 C), 0.2 mm layers, 4 walls. Press six 10 x 3 mm disc magnets into the
pockets with a drop of CA glue, all the same pole outward.

Frame (mm): X along the board's width (board x), Y up (the board's bottom edge at Y = 0 when seated, so
Y = BOARD_H - board y), Z out of the steel panel (Z = 0 is the panel). The fit check sweeps every part and
every pin tail from far above down to its seated place (the path it takes as the board slides in) and
reports any overlap with the holder.
"""
import json, os, sys
from build123d import Box, Cylinder, Pos, export_stl, export_step

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out"); os.makedirs(OUT, exist_ok=True)
VER = "v38"

# ---- board (hardware/controller/versions/v38-slide-in) ----
BOARD_W, BOARD_H, BOARD_T = 80.0, 77.0, 1.6
RAIL = 2.5                    # the board keeps this strip clear of parts on both side edges
# ---- holder ----
BACK_T = 4.0                  # back plate: insulation from the steel, and the magnet pockets
STANDOFF = 4.0                # back plate to the board's back face: room for through-hole pin tails (<= 3 mm)
ZB = BACK_T + STANDOFF        # board's back face
ZF = ZB + BOARD_T             # board's front face
CLR = 0.25                    # groove clearance, each side of the board and at its edge
GROOVE = 1.5                  # how far the grooves grip the board's edge (inside RAIL, 1 mm to spare)
LIP_T = 1.6                   # lip over the board's front face
SIDE_T = 3.0                  # side wall outside the groove
FLOOR_T = 3.0                 # floor under the board's bottom edge
LEDGE = 2.4                   # support behind the board's bottom strip, below the spade pins (Y < 3.1)
TOP_GAP = 6.0                 # side walls stop this far below the board's top edge: the plugs' row
LEAD_IN = 4.0                 # groove mouth: lips start this far down, so the edge finds the groove first
RIB_W = 2.4
RIB_X = [6.0, 20.5, 37.0, 50.0, 53.5, 67.0, 74.0]   # rib lanes (board x); each rib stops below any pin's path
PIN_TAIL = 3.0                # through-hole pin tails below the board's back face (trimmed: ~1.5-2.5)
PIN_MARGIN = 0.6              # rib to a pin tail's path, each side
MAG_D, MAG_T = 10.0, 3.0      # magnets: 10 x 3 mm N52 (or N42SH for heat) discs
MAG_POCKET_D, MAG_POCKET_T = MAG_D + 0.25, MAG_T + 0.15
MAGNETS = [(10.0, 12.0), (40.0, 12.0), (70.0, 12.0), (10.0, 58.0), (40.0, 58.0), (70.0, 58.0)]   # (X, Y)

X0, X1 = -CLR - SIDE_T, BOARD_W + CLR + SIDE_T
Y0, Y1 = -FLOOR_T, BOARD_H - TOP_GAP
ZTOP = ZF + CLR + LIP_T


def box(x0, y0, z0, x1, y1, z1):
    return Pos((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2) * Box(x1 - x0, y1 - y0, z1 - z0)


def load(name):
    return json.load(open(os.path.join(HERE, name)))


def pins():
    """Through-hole pads as (ref, X, Y, diameter) in holder coordinates."""
    return [(r, x, BOARD_H - y, d) for r, x, y, d in load(f"_tht_{VER}.json")]


def rib_top(xc):
    """A rib in lane xc may rise until just below the lowest path of any pin tail that crosses its lane."""
    top = Y1
    for _, x, y, d in pins():
        if abs(x - xc) < RIB_W / 2 + d / 2 + PIN_MARGIN:
            top = min(top, y - d / 2 - PIN_MARGIN)
    return top


def holder():
    part = box(X0, Y0, 0, X1, Y1, BACK_T)                                   # back plate
    part += box(X0, Y0, 0, GROOVE, Y1, ZTOP)                               # left wall
    part += box(BOARD_W - GROOVE, Y0, 0, X1, Y1, ZTOP)                     # right wall
    part += box(X0, Y0, 0, X1, 0.0, ZF)                                    # floor: the board's edge stands on it
    part += box(0, 0.0, BACK_T, BOARD_W, LEDGE, ZB)                        # ledge behind the spade row
    ribs = []
    for xc in RIB_X:
        top = rib_top(xc)
        if top > LEDGE + 3:
            part += box(xc - RIB_W / 2, LEDGE, BACK_T, xc + RIB_W / 2, top, ZB)
            ribs.append((xc, round(top, 1)))
    # grooves: the board's edge runs in them, open at the top
    gz0, gz1 = ZB - CLR, ZF + CLR
    part -= box(-CLR, 0.0, gz0, GROOVE, Y1 + 1, gz1)
    part -= box(BOARD_W - GROOVE, 0.0, gz0, BOARD_W + CLR, Y1 + 1, gz1)
    # lead-in: no front lip for the top LEAD_IN mm, with a 45-degree-ish step so the edge drops in
    for xa, xb in ((-CLR - 0.01, GROOVE + 0.01), (BOARD_W - GROOVE - 0.01, BOARD_W + CLR + 0.01)):
        part -= box(xa, Y1 - LEAD_IN, gz1, xb, Y1 + 1, ZTOP + 1)
    # magnet pockets, open on the back face
    for mx, my in MAGNETS:
        part -= Pos(mx, my, MAG_POCKET_T / 2 - 0.001) * Cylinder(MAG_POCKET_D / 2, MAG_POCKET_T)
    # cable-tie slots through the floor, under the spade row (the mains bundle leaves downward)
    for sx in (12.0, 31.5, 42.0, 60.0):
        part -= box(sx - 2.0, Y0 + 0.6, -1, sx + 2.0, Y0 + 2.2, ZF + 1)
    return part, ribs


# Heights the 3D model's stand-in boxes get wrong (from datasheets / evidence store).
HEIGHT_FIX = {"J12": 14.02, "J13": 14.02, "J14": 14.02, "J15": 14.02,   # TE 63824-1 tab: 14.02 mm
              "U8": 4.5, "U9": 4.5, "F1": 4.5}                          # SMDIP-6 ~3.9, Nano2 443 ~3.4: rounded up


def part_boxes():
    out = []
    for x0, y0, z0, x1, y1, z1, name in load(f"_parts_{VER}.json"):   # board x, y down, z up from its back face
        if name in HEIGHT_FIX:
            z1 = BOARD_T + HEIGHT_FIX[name]
        # every part is on the front face; a model's own pins (in the same mesh) would stretch its box through
        # the board, so clip at the front face -- the pin tails are checked one by one below
        out.append((name, x0, BOARD_H - y1, max(ZB + z0, ZF), x1, BOARD_H - y0, ZB + z1))
    for r, x, y, d in pins():
        out.append((f"{r}-pin", x - d / 2, y - d / 2, ZB - PIN_TAIL, x + d / 2, y + d / 2, ZB))
    return out


def fit_check(h):
    bad = []
    for name, x0, y0, z0, x1, y1, z1 in part_boxes():
        for label, top in (("seated", y1), ("sliding in", y1 + 300)):
            v = (h & box(x0, y0, z0, x1, top, z1)).volume
            if v > 1e-3:
                bad.append(f"{name} ({label}): {v:.2f} mm3 at X {x0:.1f}..{x1:.1f}")
                break
    v = (h & box(0, 0, ZB, BOARD_W, BOARD_H + 300, ZF)).volume
    if v > 1e-3:
        bad.append(f"board itself: {v:.2f} mm3")
    return bad


if __name__ == "__main__":
    h, ribs = holder()
    export_stl(h, os.path.join(OUT, "holder.stl"))
    export_step(h, os.path.join(OUT, "holder.step"))
    bb = h.bounding_box()
    print(f"holder {bb.size.X:.1f} x {bb.size.Y:.1f} x {bb.size.Z:.1f} mm, {h.volume / 1000:.1f} cm3; ribs (x, top Y): {ribs}")
    tall = max(p[6] for p in part_boxes())
    print(f"board's back face {ZB:.1f} mm off the panel; tallest part reaches {tall:.1f} mm")
    bad = fit_check(h)
    print("fit: OK, nothing touches seated or on the way in" if not bad else "fit problems:\n  " + "\n  ".join(bad))
    sys.exit(1 if bad else 0)
