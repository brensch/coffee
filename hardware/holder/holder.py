#!/usr/bin/env python3
"""Holder for the controller board (v45): a FRAME on the steel side panel, a COVER, and two BARS (user's design).

Frame: the board lies flat on the back plate in a recess that fits it closely, with small pockets for the through-hole
pin tails. Its top edge tucks under a lip in the recess; its bottom edge rests on the recess wall. A solid roof sits just
above the board. Struts come straight out of the frame's sides at the top and the bottom, with a hole running across.
Receptacles: straight fully insulated 6.3 mm ones push straight onto the spades, under the roof; wires leave straight out.
Cover: goes on from the front between the struts, closed across its whole top edge, including over the IRM supply. 13 mm
off the panel over the low parts, deeper over the thermocouple terminal and the supply (45-degree steps). A spring finger
inside it presses on the board. Bars: slide across through the struts, in front of the cover; their ramped ends push the
cover back against the finger, which holds the board flat on the plate and the cover hard against the bars. A head at one
end, a groove at the other for a small cable tie.

    ~/opt/holder-venv/bin/python holder.py   # -> out/holder_frame.stl, holder_cover.stl, holder_bars.stl (+ .step), checks

Frame (mm): X = board x, Y up (Y = BOARD_H - board y; board bottom edge at Y = 0), Z off the panel (board back face at ZB).
Board inputs: _court/_tht/_parts/_mains_pads_v45.json from hardware/controller/versions/v45-closed-top.
"""
import json, os, sys
import numpy as np
from build123d import Box, Cylinder, Pos, Rot, Plane, Polygon, extrude, export_stl, export_step

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out"); os.makedirs(OUT, exist_ok=True)
VER = "v45"

BOARD_W, BOARD_H, BOARD_T = 71.2, 77.0, 1.6
BACK_T = 4.5                                 # back plate; the pin-tail pockets leave 1.2 mm of it toward the steel
ZB = BACK_T
ZF = ZB + BOARD_T
CLR = 0.2                                    # recess around the board
POCKET_D = 3.3                               # pin-tail pockets (tails <= ~2.9 mm)
STRUT = 2.0                                  # strut thickness (X)
FIT = 0.3                                    # cover to strut
WALL = 1.6                                   # cover walls
XC0, XC1 = -(WALL - 0.3), BOARD_W + (WALL - 0.3)      # cover outer sides (its walls overlap the board's edge strip by 0.3)
X0, X1 = XC0 - FIT - STRUT, XC1 + FIT + STRUT         # frame outer sides = strut outer faces
ZC = ZF + 0.2                                # the cover's back edge
Z_LOW = ZF + 5.5                             # inner, over the low parts
Z_PSU = ZF + 21.5 + 0.7                      # inner, IRM-10-5
Z_TC = ZB + 15.45 + 0.7                      # inner, MKDS terminal with its screws
RECEPT = (10.4, 8.3, 30.0)                   # straight receptacle, TE C-520140 maxima incl. the 8.26 mm funnel
TAB_H = 14.02
ROOF_Y, ROOF_T = BOARD_H + 2.5, 2.5
ROOF_Z = ZF + RECEPT[2] + 5.0
LIP_LAND, LIP_Z = 0.7, ZF + 0.1              # lip over the board's top strip: a 0.7 mm land 0.1 above the board, then 45 degrees
LIP_TOP = ZF + 1.3
TAIL = 3.0                                   # pin tails below the board, at most
SLIDE = 2.0                                  # the top row's pockets are this much longer: the board hooks under the lip by sliding up
BAR = 3.0
HOLE = BAR + 0.3
BAR_GAP = 0.05                               # bar to cover, as drawn (the bars' ramped ends take up the rest)
RAMP = (6.0, 1.2)                            # bar's leading end: long, rise on its bearing face
SPRING_PRELOAD = 0.8                         # free, the finger reaches this far past the board's face: the cover's spring travel
FINGER_X = (33.0, 39.5)
FINGER_Y = (36.0, 64.0)                      # tip end, root end
FINGER_T, FINGER_PAD = 1.5, 3.0
BOTTOM = -5.4                                # cover's bottom edge
MAG_D, MAG_T = 10.0, 3.0
MAG_X, MAG_Y = (8.0, 35.6, 63.2), (14.5, 37.25, 60.0)   # a 3 x 3 grid, centred on the board's width
MAGNETS = [(x, y) for y in MAG_Y for x in MAG_X if not (x > 60 and y > 20)]   # less the two that would sit behind the supply


def box(x0, y0, z0, x1, y1, z1):
    return Pos((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2) * Box(x1 - x0, y1 - y0, z1 - z0)


def prism(pts_yz, x0, x1):
    return extrude(Plane.YZ.offset(x0) * Polygon(*pts_yz, align=None), amount=x1 - x0, dir=(1, 0, 0))


def load(name):
    return json.load(open(os.path.join(HERE, f"_{name}_{VER}.json")))


COURT = {r: (x0, BOARD_H - y1, x1, BOARD_H - y0) for r, (x0, y0, x1, y1) in load("court").items()}
PINS = [(r, x, BOARD_H - y, d) for r, x, y, d in load("tht")]
TABS = {r: ((COURT[r][0] + COURT[r][2]) / 2, (COURT[r][1] + COURT[r][3]) / 2) for r in ("J12", "J13", "J14", "J15")}
GH = ["J2", "J6", "J7", "J10"]
PSU, J5 = COURT["PS1"], COURT["J5"]
X_PSU = PSU[0] - 0.6 - WALL
X_TC = J5[0] - 0.6 - WALL
Y_PSU0 = PSU[1] - 0.6
COVER_TOP = min(ty for _, ty in TABS.values()) - RECEPT[1] / 2 - 1.0     # 1 mm below the receptacles' funnels
Z_LO, Z_DEEP = Z_LOW + WALL, Z_PSU + WALL    # outer depths
TOP_BAR = (COVER_TOP - 3.0, Z_DEEP + BAR_GAP + BAR / 2)
BOT_BAR = (-2.4, Z_LO + BAR_GAP + BAR / 2)
Y_F0 = BOT_BAR[0] - 3.2                      # frame bottom
HEIGHT = ROOF_Y + ROOF_T - Y_F0
RIB_X = (8.0, 24.0, 38.0)                    # the top bar bears on these over the low part


def step(y_low, z_low, z_high):
    return [(y_low - (z_high - z_low), z_low), (y_low, z_high)]


def outer_prof(zb, ys, zt):
    s = 2 ** 0.5 * WALL
    return [(BOTTOM, ZC), (BOTTOM, zb)] + step(ys - s, zb, zt) + [(COVER_TOP, zt), (COVER_TOP, ZC)]


def inner_prof(zb, ys, zt):
    zb -= WALL; zt -= WALL
    return [(BOTTOM - 1, ZC - 1), (BOTTOM - 1, zb)] + step(ys, zb, zt) + [(COVER_TOP - WALL, zt), (COVER_TOP - WALL, ZC - 1)]


def left_outer():
    return [(BOTTOM, ZC), (BOTTOM, Z_LO), (COVER_TOP, Z_LO), (COVER_TOP, ZC)]


def left_inner():
    return [(BOTTOM - 1, ZC - 1), (BOTTOM - 1, Z_LOW), (COVER_TOP - WALL, Z_LOW), (COVER_TOP - WALL, ZC - 1)]


def finger():
    """The spring: a leaf hanging off the cover's inside, printed with the cover. Root near the top edge, flat pad at the tip.
    Drawn in its free shape, so the pad is SPRING_PRELOAD past the board's face; seated, the board bends it back."""
    y0, y1 = FINGER_Y
    zt, zr = ZF - SPRING_PRELOAD, Z_LOW + 0.3
    yp = y0 + FINGER_PAD
    slope = (y1 - yp) / (zr - zt)
    return prism([(y1, zr), (yp, zt), (y0, zt), (y0, zt + FINGER_T), (yp, zt + FINGER_T), (y1 - FINGER_T * slope, zr)], *FINGER_X)


def cover():
    segs = [(XC0, X_PSU, left_outer(), left_inner()),
            (X_PSU, X_TC, outer_prof(Z_LO, Y_PSU0, Z_DEEP), inner_prof(Z_LO, Y_PSU0, Z_DEEP)),
            (X_TC, XC1, outer_prof(Z_TC + WALL, Y_PSU0, Z_DEEP), inner_prof(Z_TC + WALL, Y_PSU0, Z_DEEP))]
    outer = inner = None
    for a, b, o, i in segs:
        po = prism(o, a, b); pi = prism(i, a + WALL, b - WALL)
        outer = po if outer is None else outer + po
        inner = pi if inner is None else inner + pi
    for (a0, b0, o0, i0), (a1, b1, o1, i1) in zip(segs, segs[1:]):
        x = b0
        inner = inner + (prism(i0, x - WALL - 0.01, x + WALL + 0.01) & prism(i1, x - WALL - 0.01, x + WALL + 0.01))
    c = outer - inner
    for rx in RIB_X:                                                # ribs carry the top bar's load to the low front
        c += prism([(COVER_TOP - 6 - (Z_DEEP - Z_LO), Z_LO - 0.1), (COVER_TOP - 6, Z_DEEP), (COVER_TOP, Z_DEEP), (COVER_TOP, Z_LO - 0.1)], rx - 1.0, rx + 1.0)
    c += finger()
    by, bz = BOT_BAR
    c -= box(X_TC - 1, by - HOLE / 2, bz - HOLE / 2, XC1 + 1, by + HOLE / 2, bz + HOLE / 2)   # bottom bar through the terminal pocket
    return c


def lip_poly():
    r = BOARD_H + CLR
    a = r - LIP_LAND
    return [(r + 0.01, LIP_Z), (a, LIP_Z), (a - (LIP_TOP - LIP_Z), LIP_TOP), (r + 0.01, LIP_TOP)]


def tilt_pose(l, t, deg, py=BOARD_H + CLR, pz=ZB):
    """Board point (l mm down from its top edge, t mm in front of its back face) with the board swung about its top edge."""
    a = np.radians(deg)
    return py - l * np.cos(a) + t * np.sin(a), pz + l * np.sin(a) + t * np.cos(a)


def pocket_y(y, d):
    """A pin tail's pocket: its seated size, stretched along Y as far as the tail swings while the board tilts in."""
    s = (d + 1.2) / 2
    y0, y1 = y - s, y + s
    L = BOARD_H - y
    for deg in np.arange(0, 45.0, 0.5):
        py = BOARD_H + CLR - BOARD_T * np.sin(np.radians(deg))
        for l in (L - d / 2, L + d / 2):
            Y, Z = tilt_pose(l, -TAIL, deg, py)
            if Z < BACK_T:
                y0, y1 = min(y0, Y - 0.15), max(y1, Y + 0.15)
    if L < 8.0:
        y0 -= SLIDE
    return float(y0), float(y1)


def frame():
    f = box(X0, Y_F0, 0, X1, ROOF_Y + ROOF_T, BACK_T)                       # back plate
    f += box(X0, ROOF_Y, 0, X1, ROOF_Y + ROOF_T, ROOF_Z)                    # roof, solid
    f += box(X0, Y_F0, 0, -CLR, ROOF_Y, ZF)                                 # recess sides
    f += box(BOARD_W + CLR, Y_F0, 0, X1, ROOF_Y, ZF)
    f += box(X0, Y_F0, 0, X1, -CLR, ZF)                                     # recess bottom: the board rests on it
    f += box(X0, BOARD_H + CLR, 0, X1, ROOF_Y, LIP_TOP)                     # recess top
    f += prism(lip_poly(), 0.0, BOARD_W)                                    # the lip
    for by, bz in (TOP_BAR, BOT_BAR):                                       # struts straight out of the sides
        f += box(X0, by - 3.2, 0, X0 + STRUT, by + 3.2, bz + 3.2)
        f += box(X1 - STRUT, by - 3.2, 0, X1, by + 3.2, bz + 3.2)
        f -= box(X0 - 1, by - HOLE / 2, bz - HOLE / 2, X1 + 1, by + HOLE / 2, bz + HOLE / 2)
    for r_, x, y, d in PINS:                                                # pin-tail pockets
        s = (d + 1.2) / 2; y0, y1 = pocket_y(y, d)
        f -= box(x - s, y0, BACK_T - POCKET_D, x + s, y1, BACK_T + 0.01)
    for mx, my in MAGNETS:
        f -= Pos(mx, my, (MAG_T + 0.15) / 2 - 0.001) * Cylinder((MAG_D + 0.25) / 2, MAG_T + 0.15)
    return f


def one_bar(by, bz):
    h, x0, x1 = BAR / 2, X0 - 0.4, X1 + 6.0
    b = box(x0, by - h, bz - h, x1, by + h, bz + h)
    b += box(x0 - 2.5, by - h, bz - h, x0, by + h, bz + 3.0)                # head: flush on the back face
    b -= box(X1 + 1.0, by - h - 1, bz - h - 1, X1 + 3.8, by - h + 0.6, bz + h + 1)   # groove for a small cable tie
    b -= box(X1 + 1.0, by + h - 0.6, bz - h - 1, X1 + 3.8, by + h + 1, bz + h + 1)
    ln, rise = RAMP                                                         # the leading end's back face ramps: it pushes the cover back as it slides in
    ramp = Polygon(*[(x1 - ln, bz - h), (x1 + 0.01, bz - h + rise), (x1 + 0.01, bz - h - 0.5), (x1 - ln, bz - h - 0.5)], align=None)
    b -= extrude(Plane.XZ.offset(-(by + h + 1)) * ramp, amount=2 * h + 2, dir=(0, -1, 0))
    return b


def bars():
    return one_bar(*TOP_BAR) + one_bar(*BOT_BAR)


def bars_for_print():
    """Both bars lying on their side on Z = 0 (so the ramp and head are in the bed's plane), side by side."""
    out = None
    for i, (by, bz) in enumerate((TOP_BAR, BOT_BAR)):
        b = Pos(0, i * 12.0, BAR / 2) * Rot(90, 0, 0) * Pos(0, -by, -bz) * one_bar(by, bz)
        out = b if out is None else out + b
    return out


HEIGHT_FIX = {"U8": 4.5, "U9": 4.5, "F1": 4.5, "J2": 4.25, "J10": 4.25}


def part_boxes():
    out = []
    for x0, y0, z0, x1, y1, z1, name in load("parts"):
        if name in TABS:
            continue
        if name in HEIGHT_FIX:
            z1 = BOARD_T + HEIGHT_FIX[name]
        out.append((name, x0, BOARD_H - y1, max(ZB + z0, ZF), x1, BOARD_H - y0, ZB + z1))
    for r, (tx, ty) in TABS.items():
        out.append((f"{r} tab", tx - 3.25, ty - 0.45, ZF, tx + 3.25, ty + 0.45, ZF + TAB_H))
        out.append((f"{r} tab base", tx - 3.95, ty - 1.55, ZF, tx + 3.95, ty + 1.55, ZF + 2.0))
    for r, x, y, d in PINS:
        out.append((f"{r} pin tail", x - d / 2, y - d / 2, ZB - 3.0, x + d / 2, y + d / 2, ZB))
    return out


def receptacles():
    out = []
    for r, (tx, ty) in TABS.items():
        out.append((f"{r} receptacle", tx - RECEPT[0] / 2, ty - RECEPT[1] / 2, ZF + 3.0, tx + RECEPT[0] / 2, ty + RECEPT[1] / 2, ZF + RECEPT[2]))
        out.append((f"{r} wire", tx - 1.4, ty - 1.4, ZF + RECEPT[2], tx + 1.4, ty + 1.4, 120.0))
    return out


def plugs():
    out = []
    for r in GH:
        x0, _, x1, _ = COURT[r]
        out.append((f"{r} plug", x0 + 0.6, -8.0, ZF + 0.05, x1 - 0.6, 0.0, ZF + 4.6))
        out.append((f"{r} lead", x0 + 1.5, -60.0, ZF + 1.0, x1 - 1.5, -8.0, ZF + 3.5))
    for r, x, y, d in PINS:
        if r == "J5":
            out.append(("thermocouple wire", x - 0.8, -60.0, ZF + 1.5, x + 0.8, y, ZF + 5.5))
    return out


def spring_figures():
    """Beam estimate of the finger, ASA E about 2 GPa. Free length runs from where its front face leaves the cover to the pad."""
    y0, y1 = FINGER_Y
    zt, zr = ZF - SPRING_PRELOAD, Z_LOW + 0.3
    yp = y0 + FINGER_PAD
    L = y1 - FINGER_T * (y1 - yp) / (zr - zt) - yp
    w = FINGER_X[1] - FINGER_X[0]
    I = w * FINGER_T ** 3 / 12
    k = 3 * 2000 * I / L ** 3
    return f"free length {L:.1f}, {k:.2f} N/mm, {k * SPRING_PRELOAD:.2f} N at {SPRING_PRELOAD} mm, peak bending stress {3 * 2000 * FINGER_T * SPRING_PRELOAD / (2 * L * L):.0f} MPa"


def fit_check(fr, cv, br):
    bad = []

    def hit(solid, bx):
        try:
            return (solid & box(*bx)).volume > 1e-3
        except ValueError:
            return False

    def touches(a, b):
        try:
            return (a & b).volume > 1e-3
        except ValueError:
            return False
    for name, *b in part_boxes() + plugs() + receptacles():       # board and everything on it, against the frame
        if hit(fr, tuple(b)):
            bad.append(f"frame: {name} touches it")
    if hit(fr, (0, 0, ZB, BOARD_W, BOARD_H, ZF)):
        bad.append("frame: the board itself touches it")
    for _, x, y, d in PINS:                                        # tails in their pockets, with plastic left to the steel
        if hit(fr, (x - d / 2, y - d / 2, ZB - TAIL, x + d / 2, y + d / 2, ZB)):
            bad.append(f"frame: a pin tail at ({x:.1f}, {y:.1f}) hits it")
    for name, x0, y0, z0, x1, y1, z1 in part_boxes():              # board comes in from the front (lip only at its top edge)
        if "pin tail" not in name and hit(fr, (x0, y0, z0, x1, y1, z1 + 200)):
            bad.append(f"frame: {name} is in the way as the board goes in")
    for name, *b in receptacles():                                 # pushed straight on from the front
        x0, y0, z0, x1, y1, z1 = b
        if hit(fr, (x0, y0, z0, x1, y1, z1 + 200)):
            bad.append(f"frame: {name} can't be pushed on")
    for name, *b in part_boxes() + plugs() + receptacles():        # cover seated, and coming straight on
        x0, y0, z0, x1, y1, z1 = b
        if hit(cv, (x0, y0, z0, x1, y1, z1)):
            bad.append(f"cover: {name} touches it (seated)"); continue
        if hit(cv, (x0, y0, z0 - 200, x1, y1, z1)):
            bad.append(f"cover: {name} is in its way as it goes on")
    if touches(fr, cv):
        bad.append("frame and cover overlap")
    for name, *b in part_boxes() + plugs() + receptacles():
        if hit(br, tuple(b)):
            bad.append(f"bars: touch {name}")
    if touches(br, fr) or touches(br, cv):
        bad.append("bars: don't fit their holes")
    if not hit(cv, (RIB_X[0] - 0.9, TOP_BAR[0] - 1, TOP_BAR[1] - BAR / 2 - 0.5, RIB_X[0] + 0.9, TOP_BAR[0] + 1, TOP_BAR[1] - BAR / 2)):
        bad.append("bars: the top bar doesn't bear on the cover")
    if not hit(cv, (20.0, BOT_BAR[0] - 1, BOT_BAR[1] - BAR / 2 - 0.5, 30.0, BOT_BAR[0] + 1, BOT_BAR[1] - BAR / 2)):
        bad.append("bars: the bottom bar doesn't bear on the cover")
    y0, y1 = FINGER_Y
    xm = sum(FINGER_X) / 2
    if not hit(cv, (xm - 1, y0 + 0.5, ZF - SPRING_PRELOAD + 0.05, xm + 1, y0 + FINGER_PAD - 0.5, ZF)):
        bad.append("cover: the spring finger doesn't reach the board")
    if SPRING_PRELOAD <= HOLE - BAR + BAR_GAP:
        bad.append("cover: the bars' slack is more than the spring's travel")
    if RAMP[1] < SPRING_PRELOAD + BAR_GAP:
        bad.append("bars: the ramp is too short to push the cover back")
    if PSU[3] > COVER_TOP - WALL - 0.3:
        bad.append("cover: the supply's top doesn't fit under the cover's closed top")
    if not hit(fr, (5.0, BOARD_H - 0.5, ZF + 0.1, 60.0, BOARD_H + CLR, LIP_TOP)):
        bad.append("frame: no lip over the board's top edge")
    if not hit(fr, (5.0, -0.5, ZB, 60.0, -CLR, ZF)):
        bad.append("frame: nothing under the board's bottom edge")
    for mx, my in MAGNETS:
        for r, x, y, d in PINS:
            y0, y1 = pocket_y(y, d)
            dx = max(abs(mx - x) - (d + 1.2) / 2, 0.0); dy = max(y0 - my, my - y1, 0.0)
            if (dx * dx + dy * dy) ** 0.5 < (MAG_D + 0.25) / 2 + 0.8:
                bad.append(f"magnet at ({mx}, {my}) is too close to {r}'s pin pocket")
    return bad


def tilt_in():
    """Can the board really get from free of the frame to seated? Tilted section through the board (Y, Z) with its pin tails:
    search every pose (tilt, Y, Z of the top back corner) for a connected path. Returns (ok, largest tilt on the way)."""
    from collections import deque
    degs = np.arange(0.0, 40.01, 0.5); ys = np.arange(70.0, BOARD_H + CLR + 0.001, 0.05); zs = np.arange(ZB, 10.0, 0.05)
    l = np.concatenate([np.arange(0, 4, 0.05), np.arange(4, BOARD_H, 1.0), [BOARD_H]])
    edge = np.array([(a, t) for a in l for t in (0.0, BOARD_T)] + [(0.0, t) for t in np.arange(0, BOARD_T + 0.01, 0.1)] + [(BOARD_H, t) for t in np.arange(0, BOARD_T + 0.01, 0.1)])
    poly = np.array(lip_poly())
    area2 = np.sum(poly[:, 0] * np.roll(poly[:, 1], -1) - np.roll(poly[:, 0], -1) * poly[:, 1])

    def in_lip(Y, Z):                                                       # convex polygon, 0.01 mm shrunk
        inside = np.ones(Y.shape, bool)
        for (ax, ay), (bx, by) in zip(poly, np.roll(poly, -1, axis=0)):
            ex, ey = bx - ax, by - ay
            inside &= np.sign(area2) * (ex * (Z - ay) - ey * (Y - ax)) / np.hypot(ex, ey) > 0.01
        return inside
    tails = [(BOARD_H - y + sgn * d / 2, -t, *pocket_y(y, d)) for _, x, y, d in PINS for sgn in (-1, 1) for t in np.arange(0, TAIL + 0.01, 0.5)]
    tl = np.array([t[:2] for t in tails]); ty0 = np.array([t[2] for t in tails]); ty1 = np.array([t[3] for t in tails])
    free = np.zeros((len(degs), len(ys), len(zs)), bool); clear = free.copy()
    for i, deg in enumerate(degs):
        yy, zz = np.meshgrid(ys, zs, indexing="ij")
        for j in range(0, len(ys), 200):
            Y = yy[j:j + 200, :, None]; Z = zz[j:j + 200, :, None]
            py, pz = Y, Z
            Ye, Ze = tilt_pose(edge[:, 0], edge[:, 1], deg, py, pz)
            bad = in_lip(Ye, Ze) | (Ye > BOARD_H + CLR + 0.01) | (Ze < ZB - 0.01) | ((Ye < -CLR - 0.01) & (Ze < ZF - 0.01))
            Yt, Zt = tilt_pose(tl[:, 0], tl[:, 1], deg, py, pz)
            tb = (Zt < BACK_T - 0.01) & ((Yt < ty0) | (Yt > ty1) | (Zt < BACK_T - POCKET_D))
            free[i, j:j + 200] = ~(bad.any(axis=2) | tb.any(axis=2))
            clear[i, j:j + 200] = free[i, j:j + 200] & (Zt.min(axis=2) > BACK_T + 0.05) & (Ye.max(axis=2) < BOARD_H - 1.5)
    seated = (0, int(round((BOARD_H - ys[0]) / 0.05)), 0)
    if not free[seated]:
        return False, None
    seen = {seated}; q = deque([seated]); top = 0.0
    while q:
        c = q.popleft()
        if clear[c]:
            return True, top
        top = max(top, degs[c[0]])
        for ax in range(3):
            for s_ in (-1, 1):
                n = list(c); n[ax] += s_; n = tuple(n)
                if n in seen or not (0 <= n[0] < free.shape[0] and 0 <= n[1] < free.shape[1] and 0 <= n[2] < free.shape[2]) or not free[n]:
                    continue
                seen.add(n); q.append(n)
    return False, top


def coverage(cover_stl, step_=2.0):
    import numpy as np, trimesh
    m = trimesh.load(cover_stl)
    xs = np.arange(0.6, BOARD_W - 0.59, step_); ys = np.arange(0.5, COVER_TOP - 0.5, step_)
    pts = np.array([(x, y, 200.0) for x in xs for y in ys])
    hit = m.ray.intersects_any(pts, np.tile([0.0, 0.0, -1.0], (len(pts), 1)))
    return [(float(p[0]), float(p[1])) for p, h in zip(pts, hit) if not h]


def overhangs(path, up, min_width=6.0):
    import numpy as np, trimesh, networkx as nx
    from collections import Counter
    m = trimesh.load(path); up = np.array(up, float)
    h = m.vertices @ up
    down = np.where((m.face_normals @ up < -0.7072) & (m.triangles_center @ up > h.min() + 0.3))[0]
    if len(down) == 0:
        return []
    keep = set(down.tolist())
    g = nx.Graph(); g.add_nodes_from(keep); g.add_edges_from(tuple(e) for e in m.face_adjacency if e[0] in keep and e[1] in keep)
    out = []
    for comp in nx.connected_components(g):
        comp = list(comp)
        area = float(m.area_faces[comp].sum())
        edges = Counter(tuple(sorted(e)) for f in m.faces[comp] for e in ((f[0], f[1]), (f[1], f[2]), (f[2], f[0])))
        perim = sum(float(np.linalg.norm(m.vertices[a] - m.vertices[b])) for (a, b), n in edges.items() if n == 1)
        width = 2 * area / perim if perim else 0.0
        if width > min_width:
            c = m.triangles[comp].reshape(-1, 3).mean(axis=0)
            out.append(f"~{width:.1f} mm wide, {area:.0f} mm2 at X {c[0]:.0f} Y {c[1]:.0f} Z {c[2]:.0f}")
    return out


def exposed_mains():
    return sorted({f"{r}.{n} ({net})" for r, n, net, x0, y0, x1, y1 in load("mains_pads") if r != "PS1" and BOARD_H - y0 > COVER_TOP})


if __name__ == "__main__":
    fr, cv, br = frame(), cover(), bars()
    for name, s in (("holder_frame", fr), ("holder_cover", cv), ("holder_bars", bars_for_print()), ("holder_bars_assembled", br)):
        export_stl(s, os.path.join(OUT, f"{name}.stl"))
        if name != "holder_bars_assembled":
            export_step(s, os.path.join(OUT, f"{name}.step"))
    for name, s in (("frame", fr), ("cover", cv)):
        bb = s.bounding_box()
        print(f"{name}: {bb.size.X:.1f} x {bb.size.Y:.1f} x {bb.size.Z:.1f} mm, {s.volume / 1000:.1f} cm3")
    print(f"off the panel: cover {Z_LO:.1f} (low parts), {Z_TC + WALL:.1f} (terminal), {Z_DEEP:.1f} (supply); receptacles to {ZF + RECEPT[2]:.1f}; roof edge {ROOF_Z:.1f}. Height {HEIGHT:.1f}. Supply top {PSU[3]:.2f}, cover closes at {COVER_TOP - WALL:.2f}. Finger: {spring_figures()}")
    print("mains pads seen above the cover's top edge (under the roof):", exposed_mains() or "none")
    holes = coverage(os.path.join(OUT, "holder_cover.stl"))
    print("board visible through the cover:", f"{len(holes)} points, e.g. {holes[:6]}" if holes else "none")
    for label, f, up in (("frame on its back", "holder_frame", (0, 0, 1)), ("cover on its top edge", "holder_cover", (0, -1, 0)), ("bars on their sides", "holder_bars", (0, 0, 1))):
        print(f"overhangs, {label}:", overhangs(os.path.join(OUT, f + ".stl"), up) or "none")
    bad = fit_check(fr, cv, br)
    ok, deg = tilt_in()
    print(f"board tilts in under the lip with its pins: {'yes' if ok else 'NO'} (search reached {deg} degrees)")
    if not ok:
        bad.append("frame: the board can't tilt in under the lip")
    for name, s, want in (("frame", fr, 1), ("cover", cv, 1), ("bars", br, 2)):
        n = len(s.solids())
        print(f"{name}: {n} solid bod{'y' if n == 1 else 'ies'}")
        if n != want:
            bad.append(f"{name}: {n} separate bodies, expected {want}")
    if holes:
        bad.append(f"cover: the board shows through it at {len(holes)} grid points")
    print("fit: OK" if not bad else "fit problems:\n  " + "\n  ".join(bad))
    sys.exit(1 if bad else 0)
