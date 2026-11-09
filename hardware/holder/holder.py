#!/usr/bin/env python3
"""Holder for the controller board (v42 holder, for the v39 board): a FRAME on the steel side panel, a COVER that
drops in over the board, and two BARS that slide across the full width to lock it (user's design, 2026-11-08).

Frame: back plate (six magnets), side walls with the board's grooves (open at the bottom), a channel gripping the
board's top edge, a solid roof just above the board, and an ear on each side wall at the top and at the bottom with
a hole running across (X). The board slides up into the frame from below.
Receptacles: ordinary straight fully insulated 6.3 mm receptacles push straight onto the spade tabs; they stick out
of the board above the cover's top edge, under the roof, and their wires leave straight out.
Cover: shaped to the board: ~17 mm off the panel over the low parts, deeper over the IRM supply and the thermocouple
terminal (45-degree steps, prints without supports), with a band along its top edge as deep as the supply bump so
the top bar bears on it everywhere. Two fingers at its bottom corners tuck under the board's bottom edge: the board
stands on the cover.
Bars: slide across through the frame's ears, one in front of the cover's top band, one in front of its bottom edge
(through the terminal pocket's walls), and lock it. A head at one end, a snap bump near the other.

    ~/opt/holder-venv/bin/python holder.py   # -> out/holder_frame.stl, holder_cover.stl, holder_bars.stl (+ .step), checks

Frame (mm): X along the board (board x), Y up (Y = BOARD_H - board y; board bottom edge at Y = 0), Z off the panel.
Board inputs: _court/_tht/_parts/_mains_pads_v39.json from hardware/controller/versions/v39-front-plate.
"""
import json, os, sys
from build123d import Box, Cylinder, Pos, Plane, Polygon, extrude, export_stl, export_step

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "out"); os.makedirs(OUT, exist_ok=True)
VER = "v39"

BOARD_W, BOARD_H, BOARD_T = 80.0, 77.0, 1.6
BACK_T, STANDOFF = 4.0, 4.0
ZB = BACK_T + STANDOFF
ZF = ZB + BOARD_T
CLR = 0.25
GROOVE, SIDE_T = 1.5, 3.0
X0, X1 = -CLR - SIDE_T, BOARD_W + CLR + SIDE_T      # frame side walls / cover skirts, outer
EAR_T = 3.5                                         # ears sit outboard of the cover's skirts
XE0, XE1 = X0 - EAR_T, X1 + EAR_T
FRAME_Z = 12.0                                      # frame side walls stop here; the cover's skirts sit in front
TOP_LIP = 1.2
BOTTOM = -6.0
WALL = 2.0
Z_LOW = ZF + 5.5                                    # inner, over the low parts
Z_PSU = ZB + 21.5 + 1.6 + 0.7                       # inner, IRM-10-5
Z_TC = ZB + 15.45 + 0.7                             # inner, MKDS terminal with its screws
BAND = 6.0                                          # the cover's top band, as deep as the supply bump
TAB_H = 14.02
RECEPT = (10.4, 5.0, 30.0)                          # straight receptacle, TE C-520140 maxima, ~30 mm off the board
ROOF_T = 2.5
BAR = 3.0                                           # bar section (square); holes BAR + 0.3
MAG_D, MAG_T = 10.0, 3.0
MAGNETS = [(12.0, 12.0), (40.0, 12.0), (68.0, 8.0), (12.0, 60.0), (40.0, 60.0), (40.0, 36.0)]


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
Y_PSU0 = PSU[1] - 0.6                                # where the supply bump's cavity is full depth
COVER_TOP = min(ty for _, ty in TABS.values()) - RECEPT[1] / 2 - 1.5     # below the receptacles
Z_DEEP = Z_PSU + WALL                                # supply bump and top band, outer
ROOF_Y = BOARD_H + 2.5                               # roof underside: just above the board's top edge and its channel
ROOF_Z = ZF + RECEPT[2] + 5.0                        # roof reaches 5 mm past the receptacles' ends
TOP_BAR = (COVER_TOP - BAND / 2, Z_DEEP + BAR / 2 + 0.2)       # (Y, Z) of the bar centres
BOT_BAR = (BOTTOM + 0.5 + BAR / 2, Z_LOW + WALL + BAR / 2 + 0.2)


def step(y_low, z_low, z_high):
    return [(y_low - (z_high - z_low), z_low), (y_low, z_high)]


def profile(z_bottom, y_step, z_top, inner=False):
    """Cover side profile: z_bottom from the bottom edge, a 45-degree step reaching z_top at y_step (cavity full depth
    there; the outer step sits WALL*sqrt(2) lower), z_top up to the cover's top edge."""
    s = 2 ** 0.5 * WALL
    if not inner:
        return [(BOTTOM, FRAME_Z), (BOTTOM, z_bottom)] + step(y_step - s, z_bottom, z_top) + [(COVER_TOP, z_top), (COVER_TOP, FRAME_Z)]
    zb, zt = z_bottom - WALL, z_top - WALL
    return [(BOTTOM - 1, FRAME_Z - 1), (BOTTOM - 1, zb)] + step(y_step, zb, zt) + [(COVER_TOP + 1, zt), (COVER_TOP + 1, FRAME_Z - 1)]


def cover():
    segs = [  # (x0, x1, z_bottom outer, y where the deep part starts, z_top outer)
        (X0, X_PSU, Z_LOW + WALL, COVER_TOP - BAND, Z_DEEP),     # left: low, then the top band
        (X_PSU, X_TC, Z_LOW + WALL, Y_PSU0, Z_DEEP),             # supply column
        (X_TC, X1, Z_TC + WALL, Y_PSU0, Z_DEEP),                 # terminal, then the supply column
    ]
    outer = inner = None
    for a, b, zb, ys, zt in segs:
        o = prism(profile(zb, ys, zt), a, b)
        inn = prism(profile(zb, ys, zt, inner=True), a + WALL, b - WALL)
        outer = o if outer is None else outer + o
        inner = inn if inner is None else inner + inn
    for (a0, b0, zb0, ys0, zt0), (a1, b1, zb1, ys1, zt1) in zip(segs, segs[1:]):   # open only where both sides are open
        x = b0
        inner = inner + (prism(profile(zb0, ys0, zt0, inner=True), x - WALL - 0.01, x + WALL + 0.01) &
                         prism(profile(zb1, ys1, zt1, inner=True), x - WALL - 0.01, x + WALL + 0.01))
    c = outer - inner
    for fx0, fx1 in ((2.0, 7.5), (76.0, 78.3)):                  # fingers under the board's bottom edge (inside the side walls)
        c += box(fx0, -3.0, ZB - 0.1, fx1, 0.0, FRAME_Z + 0.5)
    by, bz = BOT_BAR                                             # bottom bar passes through the terminal pocket's walls
    s = BAR + 0.3
    c -= box(X_TC - 1, by - s / 2, bz - s / 2, X1 + 1, by + s / 2, bz + s / 2)
    return c


def frame():
    f = box(XE0, BOTTOM, 0, XE1, ROOF_Y + ROOF_T, BACK_T)                       # back plate up to the roof
    f += box(XE0, ROOF_Y, 0, XE1, ROOF_Y + ROOF_T, ROOF_Z)                      # roof: solid
    f += box(X0, BOTTOM, 0, GROOVE, ROOF_Y, FRAME_Z)                            # side walls
    f += box(BOARD_W - GROOVE, BOTTOM, 0, X1, ROOF_Y, FRAME_Z)
    f += box(0, BOARD_H - TOP_LIP, BACK_T, BOARD_W, ROOF_Y, ZF + CLR + 1.6)     # top channel block
    gz0, gz1 = ZB - CLR, ZF + CLR
    f -= box(-CLR, BOTTOM - 1, gz0, GROOVE, BOARD_H + CLR, gz1)
    f -= box(BOARD_W - GROOVE, BOTTOM - 1, gz0, BOARD_W + CLR, BOARD_H + CLR, gz1)
    f -= box(-CLR, BOARD_H - TOP_LIP - 0.01, gz0, BOARD_W + CLR, BOARD_H + CLR, gz1)
    s = BAR + 0.3
    for by, bz in (TOP_BAR, BOT_BAR):                                           # ears, outboard of the cover
        for xa, xb in ((XE0, X0), (X1, XE1)):
            f += box(xa, by - BAR, 0, xb, by + BAR, bz + BAR)
        f -= box(XE0 - 1, by - s / 2, bz - s / 2, XE1 + 1, by + s / 2, bz + s / 2)
    for mx, my in MAGNETS:
        f -= Pos(mx, my, (MAG_T + 0.15) / 2 - 0.001) * Cylinder((MAG_D + 0.25) / 2, MAG_T + 0.15)
    return f


def bars():
    out = None
    for by, bz in (TOP_BAR, BOT_BAR):
        b = box(XE0 - 0.5, by - BAR / 2, bz - BAR / 2, XE1 + 1.5, by + BAR / 2, bz + BAR / 2)
        b += box(XE0 - 3.0, by - BAR, bz - BAR, XE0 - 0.5, by + BAR, bz + BAR)            # head
        b += box(XE1 + 0.3, by + BAR / 2 - 0.01, bz - BAR / 2, XE1 + 1.3, by + BAR / 2 + 0.35, bz + BAR / 2)  # snap bump
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
    for r, (tx, ty) in TABS.items():               # v39 tabs: blade along X, base 7.9 x 3.1
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
            out.append(("thermocouple wire", x - 0.8, -60.0, ZF + 1.5, x + 0.8, y, ZF + 3.5))
    return out


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
    for name, *b in part_boxes() + plugs():                       # 1. board slides up into the frame
        x0, y0, z0, x1, y1, z1 = b
        for label, bot in (("seated", y0), ("sliding in", y0 - 300)):
            if hit(fr, (x0, bot, z0, x1, y1, z1)):
                bad.append(f"frame: {name} touches it ({label})"); break
    if hit(fr, (0, -300, ZB, BOARD_W, BOARD_H, ZF)):
        bad.append("frame: the board itself touches it")
    for name, *b in receptacles():                                # 2. receptacles pushed straight on, wires straight out
        if hit(fr, tuple(b)):
            bad.append(f"frame: {name} touches it")
    no_fingers = cv - box(X0 - 1, -3.5, 0, X1 + 1, 0.5, FRAME_Z + 1)
    for name, *b in part_boxes() + plugs() + receptacles():       # 3. cover seated, and coming straight in
        x0, y0, z0, x1, y1, z1 = b
        if hit(cv, (x0, y0, z0, x1, y1, z1)):
            bad.append(f"cover: {name} touches it (seated)"); continue
        if hit(no_fingers, (x0, y0, z0 - 80, x1, y1, z1)):
            bad.append(f"cover: {name} is in its way as it goes on")
    if touches(fr, cv):
        bad.append("frame and cover overlap")
    for name, *b in part_boxes() + plugs() + receptacles():       # 4. bars
        if hit(br, tuple(b)):
            bad.append(f"bars: touch {name}")
    if touches(br, fr) or touches(br, cv):
        bad.append("bars: don't fit their holes")
    for i, (by, bz) in enumerate((TOP_BAR, BOT_BAR)):             # bars actually bear on the cover
        if not hit(cv, (20.0, by - BAR / 2, bz - BAR / 2 - 0.6, 30.0, by + BAR / 2, bz - BAR / 2)):
            bad.append(f"bars: the {'top' if i == 0 else 'bottom'} bar doesn't bear on the cover")
    if not hit(cv, (0, -0.2, ZB, BOARD_W, 0.0, ZF)):              # board stands on the cover's fingers
        bad.append("cover: nothing holds the board up")
    if hit(cv, (0, 0.001, ZB, BOARD_W, BOARD_H, ZF)):
        bad.append("cover: fingers cut into the board")
    if not hit(fr, (0, BOARD_H + CLR + 0.01, ZB, BOARD_W, BOARD_H + 300, ZF)):
        bad.append("frame: nothing stops the board at the top")
    if not hit(fr, (0, BOARD_H - TOP_LIP + 0.1, ZF + CLR + 0.05, BOARD_W, BOARD_H - 0.1, ZF + CLR + 1.0)):
        bad.append("frame: no lip in front of the board's top edge")
    return bad


def coverage(cover_stl, step_=2.0):
    """Sight-lines onto the board from the front, below the cover's top edge: each must hit the cover."""
    import numpy as np, trimesh
    m = trimesh.load(cover_stl)
    xs = np.arange(1.0, BOARD_W - 0.99, step_); ys = np.arange(0.5, COVER_TOP - 0.5, step_)
    pts = np.array([(x, y, 200.0) for x in xs for y in ys])
    hit = m.ray.intersects_any(pts, np.tile([0.0, 0.0, -1.0], (len(pts), 1)))
    return [(float(p[0]), float(p[1])) for p, h in zip(pts, hit) if not h]


def overhangs(path, up, min_width=6.0):
    """Down-facing faces steeper than 45 degrees in the print orientation, grouped into patches; width ~ 2 x area /
    perimeter; patches narrower than min_width (holes, groove ceilings, short bridges) print fine."""
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
    """Mains pads seen from the front above the cover's top edge (under the roof)."""
    return sorted({f"{r}.{n} ({net})" for r, n, net, x0, y0, x1, y1 in load("mains_pads") if r != "PS1" and BOARD_H - y0 > COVER_TOP})


if __name__ == "__main__":
    fr, cv, br = frame(), cover(), bars()
    for name, s in (("holder_frame", fr), ("holder_cover", cv), ("holder_bars", br)):
        export_stl(s, os.path.join(OUT, f"{name}.stl")); export_step(s, os.path.join(OUT, f"{name}.step"))
    for f in os.listdir(OUT):
        if f.startswith(("holder_rods", "holder_peg", "holder_bar.", "holder.")):
            os.remove(os.path.join(OUT, f))
    for name, s in (("frame", fr), ("cover", cv)):
        bb = s.bounding_box()
        print(f"{name}: {bb.size.X:.1f} x {bb.size.Y:.1f} x {bb.size.Z:.1f} mm, {s.volume / 1000:.1f} cm3")
    print(f"off the panel: cover {Z_LOW + WALL:.1f} (low parts), {Z_TC + WALL:.1f} (terminal), {Z_DEEP:.1f} (supply, top band); "
          f"receptacles to {ZF + RECEPT[2]:.1f}; roof edge {ROOF_Z:.1f}. Height {ROOF_Y + ROOF_T - BOTTOM:.1f} mm")
    print("mains pads seen above the cover's top edge (under the roof):", exposed_mains() or "none")
    holes = coverage(os.path.join(OUT, "holder_cover.stl"))
    print("board visible through the cover:", f"{len(holes)} points, e.g. {holes[:6]}" if holes else "none")
    print("overhangs, frame printed on its back:", overhangs(os.path.join(OUT, "holder_frame.stl"), (0, 0, 1)) or "none")
    print("overhangs, cover printed on its bottom edge:", overhangs(os.path.join(OUT, "holder_cover.stl"), (0, 1, 0)) or "none")
    bad = fit_check(fr, cv, br)
    if holes:
        bad.append(f"cover: the board shows through it at {len(holes)} grid points")
    print("fit: OK" if not bad else "fit problems:\n  " + "\n  ".join(bad))
    sys.exit(1 if bad else 0)
