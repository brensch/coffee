#!/usr/bin/env python3
"""Place the low-voltage controller PCB from controller.py's output (KiCad's Python, pcbnew).

  1. place footprints from plan.json (all on the top side), draw the outline;
  2. ext_route.py exports Specctra DSN, routes it with fastroute, imports the session;
  3. finish_own(): GND pours, stitching, zone fill, DRC, gerbers/drill/renders.

JLC standard capabilities only (cheapest tier): 2 layers, 0.2 mm tracks/gaps (0.15 min), 0.3 mm
drills, 0.6 mm vias, 0.3 mm copper to edge.

    source ~/opt/kienv.sh            # KiCad 7 libraries + pcbnew on PYTHONPATH
    python3 layout.py                # placement only
"""

import json
import os
import re
import subprocess
import sys
import time

import pcbnew
from pcbnew import FromMM as mm, VECTOR2I as V

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.environ.get("BUILD_DIR", os.path.join(HERE, "build"))
FP_DIR = os.environ.get("KICAD7_FOOTPRINT_DIR", "/usr/share/kicad/footprints")
# Freerouting 2.1.0 jar (Java 21). The 2.5.0 native CLI hangs at 0% CPU after a serialisation crash.

# ---------------------------------------------------------------- geometry (mm)
PLAN = os.environ.get("PLACE_JSON", os.path.join(HERE, "plan.json"))
_plan = json.load(open(PLAN))
W, H = _plan["W"], _plan["H"]
PLACE = {r: tuple(v) for r, v in _plan["place"].items()}
for _k, _v in _plan.get("route", {}).items():      # routing settings that reproduce this plan's board
    os.environ.setdefault(_k, str(_v))
SWAP = set(_plan.get("swap", []))   # two-pin terminals whose pins are interchangeable (heater, valve/pump): swap 1<->2


def load_fp(spec):
    lib, name = spec.split(":", 1)
    local = os.path.join(HERE, "lib", lib + ".pretty")   # vendored libraries (Espressif)
    fp = pcbnew.FootprintLoad(local if os.path.isdir(local) else os.path.join(FP_DIR, lib + ".pretty"), name)
    if fp is None:
        raise SystemExit(f"footprint not found: {spec}")
    return fp


def seg(board, layer, pts, width=0.15, closed=True):
    n = len(pts)
    for i in range(n if closed else n - 1):
        s = pcbnew.PCB_SHAPE(board)
        s.SetShape(pcbnew.SHAPE_T_SEGMENT)
        s.SetLayer(layer)
        s.SetWidth(mm(width))
        s.SetStart(V(mm(pts[i][0]), mm(pts[i][1])))
        s.SetEnd(V(mm(pts[(i + 1) % n][0]), mm(pts[(i + 1) % n][1])))
        board.Add(s)


def rect(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


def guard(board, layer, pts, name):
    """No tracks or vias of any net (pours and pads allowed), on one layer."""
    z = pcbnew.ZONE(board)
    z.SetIsRuleArea(True)
    z.SetDoNotAllowTracks(True)
    z.SetDoNotAllowVias(True)
    z.SetDoNotAllowCopperPour(False)
    z.SetDoNotAllowPads(False)
    z.SetDoNotAllowFootprints(False)
    ls = pcbnew.LSET(); ls.AddLayer(layer)
    z.SetLayerSet(ls)
    z.SetZoneName(name)
    o = z.Outline(); o.NewOutline()
    for x, y in pts:
        o.Append(mm(x), mm(y))
    board.Add(z)


def zone(board, net, layer, pts, keepout=False, name="", clearance=0.3, priority=0, pads_ok=False):
    z = pcbnew.ZONE(board)
    z.SetLayer(layer)
    if keepout:
        z.SetIsRuleArea(True)
        z.SetDoNotAllowTracks(True)
        z.SetDoNotAllowVias(True)
        z.SetDoNotAllowCopperPour(True)
        z.SetDoNotAllowPads(not pads_ok)
        z.SetDoNotAllowFootprints(False)
        ls = pcbnew.LSET(); ls.AddLayer(pcbnew.F_Cu); ls.AddLayer(pcbnew.B_Cu)
        z.SetLayerSet(ls)
    else:
        z.SetNet(net)
        z.SetLocalClearance(mm(clearance))
        z.SetMinThickness(mm(0.25))
        z.SetPadConnection(pcbnew.ZONE_CONNECTION_THERMAL)
        z.SetThermalReliefGap(mm(0.4))
        z.SetThermalReliefSpokeWidth(mm(0.5))
        z.SetAssignedPriority(priority)
    if name:
        z.SetZoneName(name)
    o = z.Outline()
    o.NewOutline()
    for x, y in pts:
        o.Append(mm(x), mm(y))
    board.Add(z)
    return z


def track(board, net, layer, a, b, width):
    t = pcbnew.PCB_TRACK(board)
    t.SetStart(V(mm(a[0]), mm(a[1])))
    t.SetEnd(V(mm(b[0]), mm(b[1])))
    t.SetWidth(mm(width))
    t.SetLayer(layer)
    t.SetNet(net)
    t.SetLocked(True)
    board.Add(t)


def bean_logo(board, cx, cy, height=11.0, layer=pcbnew.F_SilkS):
    """The top-hat bean (logo/tophat.svg, option 2a) as silkscreen strokes, `height` mm tall."""
    import math
    s = height / 306.0                         # drawing units: hat top at -194, bean bottom at +112
    oy = cy - (112 - 194) / 2 * s              # centre the whole figure vertically on cy
    P = lambda x, y: (cx + x * s, oy + y * s)
    def line(pts, w):
        for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
            g = pcbnew.PCB_SHAPE(board); g.SetShape(pcbnew.SHAPE_T_SEGMENT); g.SetLayer(layer)
            g.SetWidth(mm(w * s)); g.SetStart(V(mm(x0), mm(y0))); g.SetEnd(V(mm(x1), mm(y1))); board.Add(g)
    def bez(p0, p1, p2, p3, n=16):
        return [P(*[(1 - t) ** 3 * a + 3 * (1 - t) ** 2 * t * b_ + 3 * (1 - t) * t ** 2 * c + t ** 3 * d
                    for a, b_, c, d in zip(p0, p1, p2, p3)]) for t in [i / n for i in range(n + 1)]]
    def quad(p0, p1, p2, n=12):
        return [P(*[(1 - t) ** 2 * a + 2 * (1 - t) * t * b_ + t ** 2 * c for a, b_, c in zip(p0, p1, p2)]) for t in [i / n for i in range(n + 1)]]
    # bean body (four cubic segments, as in the SVG)
    for seg_ in (((0, -112), (62, -112), (96, -58), (96, 6)), ((96, 6), (96, 72), (54, 112), (0, 112)),
                 ((0, 112), (-54, 112), (-96, 72), (-96, 6)), ((-96, 6), (-96, -58), (-62, -112), (0, -112))):
        line(bez(*seg_), 11)
    line(bez((8, -104), (-6, -88), (-6, -72), (4, -60)), 8)          # crease, top
    line(bez((-4, 62), (8, 74), (8, 90), (-2, 104)), 8)              # crease, bottom
    for ex in (-28, 28):                                             # eyes: filled dots
        g = pcbnew.PCB_SHAPE(board); g.SetShape(pcbnew.SHAPE_T_CIRCLE); g.SetLayer(layer)
        c = P(ex, -24); g.SetCenter(V(mm(c[0]), mm(c[1]))); g.SetEnd(V(mm(c[0] + 4.5 * s), mm(c[1])))
        g.SetWidth(mm(9 * s)); g.SetFilled(True); board.Add(g)
    line(quad((-24, 8), (0, 30), (24, 8)), 11)                       # smile
    line(quad((-62, -104), (0, -124), (62, -104)), 11)               # hat brim
    line([P(-38, -110), P(-36, -186)], 11); line([P(38, -110), P(36, -186)], 11)
    line(quad((-36, -186), (0, -194), (36, -186)), 11)               # hat top
    line(quad((-37, -132), (0, -140), (37, -132)), 7)                # hat band


def silkscreen(board, mains_v, footprints=None):
    """Labels on the bottom silkscreen (the bottom carries no parts): under every connector its name
    and pin order, plus the board name and the logo where the plan puts them. The top is too dense
    for text; it keeps only the connectors' own pin-1 marks. Optional top labels: plan "label_pos"."""
    LABELS = _plan.get("labels", {
        "J1": ("5V IN", ["+5V", "GND", "GND"]), "J2": ("HEATER", ["+5V", "OUT", "GND"]), "J3": ("PUMP", ["+5V", "OUT", "GND"]),
        "J8": ("VALVE", ["+5V", "OUT", "GND"]), "J5": ("NTC", ["", "", "NC"]), "J6": ("PRESS", ["+5V", "SIG", "GND"]),
        "J7": ("FRONT", ["PWR", "BREW", "STM", "GND", "LP", "LC", "LS"]),
        "J10": ("SCALE", ["+5V", "3V3", "GND", "SCK", "D1", "D2"]), "J4": ("USB", [])})
    def silk(txt, x, y, size=1.0, angle=0, layer=pcbnew.B_SilkS):
        size = max(size, 1.0)              # JLC's recommended minimum text height (1.0 mm, 0.16 mm stroke)
        t = pcbnew.PCB_TEXT(board)
        t.SetText(txt); t.SetPosition(V(mm(x), mm(y))); t.SetLayer(layer)
        t.SetTextSize(V(mm(size), mm(size))); t.SetTextThickness(mm(0.16))
        t.SetTextAngleDegrees(angle)
        if layer == pcbnew.B_SilkS:
            t.SetMirrored(True)
        board.Add(t)
    tlen = lambda txt, size: 0.85 * max(size, 1.0) * len(txt)      # KiCad stroke font advance (measured on the render)
    fps = {f.GetReference(): f for f in (footprints or board.GetFootprints())}
    skip_words = _plan.get("label_skip", {})      # plan: pins whose word is placed by hand (bottom_text) instead
    name_pos = _plan.get("label_name_pos", {})    # plan: [x, y, angle] for a connector name placed by hand
    for ref, (name, words) in LABELS.items():
        fp = fps.get(ref)
        if fp is None or ref in _plan.get("label_off", []):
            continue
        fp.BuildCourtyardCaches()
        cb = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd).BBox()
        x0, y0, x1, y1 = (pcbnew.ToMM(v) for v in (cb.GetLeft(), cb.GetTop(), cb.GetRight(), cb.GetBottom()))
        d = {"L": x0, "R": W - x1, "T": y0, "B": H - y1}
        edge = min(d, key=lambda e: (round(d[e], 1), e not in "TB"))   # a corner connector: its long edge
        inward = {"L": (1, 0), "R": (-1, 0), "T": (0, 1), "B": (0, -1)}[edge]
        pads = {p.GetNumber(): (pcbnew.ToMM(p.GetPosition().x), pcbnew.ToMM(p.GetPosition().y)) for p in fp.Pads() if p.GetNumber().isdigit()}
        if not words:                      # USB: just the name, centred on the part
            silk(name, (x0 + x1) / 2, (y0 + y1) / 2, 0.8)
            continue
        ang = 0 if edge in ("L", "R") else 90          # one word per pin, stacked at the 1.25 mm pitch
        reach = 0.0
        for num, word in zip([str(i) for i in range(1, len(words) + 1)], words):
            if not word or int(num) in skip_words.get(ref, []):
                continue
            px, py = pads[num]
            gap = 1.3 if fp.IsFlipped() else 0.5   # a bottom-side connector's own pads are on this layer: start past them
            off = gap + tlen(word, 0.8) / 2
            silk(word, px + inward[0] * off, py + inward[1] * off, 0.8, ang)
            reach = max(reach, gap + tlen(word, 0.8))
        # the name on the outer side of the pins, under the connector's own body (free on the bottom)
        cx = sum(p[0] for p in pads.values()) / len(pads); cy = sum(p[1] for p in pads.values()) / len(pads)
        nx, ny = _plan.get("name_shift", {}).get(ref, (0, 0))      # plan: move a name clear of a neighbour's words
        if ref in name_pos:
            silk(name, *name_pos[ref][:2], 0.8, name_pos[ref][2])
        elif fp.IsFlipped():   # the body covers the outer side: the name goes inside the pin words, facing the connector
            d = reach + 0.9
            silk(name, cx + inward[0] * d + nx, cy + inward[1] * d + ny, 0.8, 90 if edge in ("L", "R") else 0)
        else:
            silk(name, cx - inward[0] * 2.1 + nx, cy - inward[1] * 2.1 + ny, 0.8, 90 if edge in ("L", "R") else 0)
    for txt, x, y, size, *ang in _plan.get("bottom_text", []):
        silk(txt, x, y, size, ang[0] if ang else 0)
    for ref, (x, y, ang, txt) in _plan.get("label_pos", {}).items():
        silk(txt, x, y, 0.8, ang, pcbnew.F_SilkS)
    if "logo" in _plan:
        lx, ly, lh = _plan["logo"]
        bean_logo(board, lx, ly, lh, layer=pcbnew.B_SilkS)

def build(spec, mains_v):
    board = pcbnew.BOARD()
    board.SetCopperLayerCount(2)
    ds = board.GetDesignSettings()
    ds.SetCopperLayerCount(2)
    ds.m_TrackMinWidth = mm(0.15)     # JLC standard: 0.127 mm (5 mil) at 1 oz
    ds.m_MinClearance = mm(0.15)
    ds.m_ViasMinSize = mm(0.6)        # JLC standard via: 0.3 mm drill, 0.6 mm pad
    ds.m_MinThroughDrill = mm(0.3)
    ds.m_CopperEdgeClearance = mm(0.3)
    ds.m_HoleClearance = mm(0.2)      # JLC: hole to copper >= 0.2 mm (the USB-C peg holes are the tight spot)

    # nets
    nets = {}
    names = sorted({n for p in spec["parts"].values() for n in p["pins"].values() if n})
    for name in names:
        ni = pcbnew.NETINFO_ITEM(board, name)
        board.Add(ni)
        nets[name] = ni

    # net classes: signals (default), power
    ns = ds.m_NetSettings
    def netclass(name, width, clearance, via=0.8, drill=0.4):
        nc = pcbnew.NETCLASS(name)
        nc.SetTrackWidth(mm(width))
        nc.SetClearance(mm(clearance))
        nc.SetViaDiameter(mm(via))
        nc.SetViaDrill(mm(drill))
        ns.m_NetClasses[name] = nc
        return nc
    dflt = ns.m_DefaultNetClass
    dflt.SetTrackWidth(mm(0.25))
    dflt.SetClearance(mm(0.2))
    dflt.SetViaDiameter(mm(0.6))
    dflt.SetViaDrill(mm(0.3))
    dflt.SetTrackWidth(mm(0.2))
    classes = {"Power": netclass("Power", 0.4, 0.2, via=0.6, drill=0.3)}
    power = {"GND", "+5V", "+3V3", "VBUS", "+5V_IN"}
    for name, ni in nets.items():
        cls = "Power" if name in power else None
        if cls:
            ns.m_NetClassPatternAssignments.append(
                pcbnew.NETCLASS_PATTERN_ASSIGNMENT(f"^{re.escape(name)}$", cls)) if hasattr(pcbnew, "NETCLASS_PATTERN_ASSIGNMENT") else None
            ni.SetNetClass(classes[cls])

    # footprints
    missing = []
    for ref, p in spec["parts"].items():
        fp = load_fp(p["footprint"])
        fp.SetReference(ref)
        fp.SetValue(p["value"])
        if ref not in PLACE:
            missing.append(ref)
            continue
        x, y, rot, side = PLACE[ref]
        board.Add(fp)                      # Flip() needs the footprint on a board
        fp.SetPosition(V(mm(x), mm(y)))
        if side == "B":
            fp.Flip(fp.GetPosition(), False)
        fp.SetOrientationDegrees(rot)
        # silkscreen: only the parts you wire to or hand-solder keep their labels (small SMD refs would
        # sit on pads and on each other; the assembly drawing and BOM carry them)
        fp.Reference().SetVisible(False)   # connectors carry their wiring labels instead (bottom silkscreen)
        if ref in _plan.get("fp_silk_off", []):   # plan: an unfitted footprint (bare pads) needs no outline
            for g in list(fp.GraphicalItems()):
                if g.GetLayer() in (pcbnew.F_SilkS, pcbnew.B_SilkS):
                    fp.Remove(g)
        if ref == "J4" and "USB_C" in str(fp.GetFPID().GetLibItemName()):
            # USB-C (HRO TYPE-C-31-M-12), to HRO's recommended layout (drawing rev A, 2020-12-08): locating
            # holes 0.60 mm for the 0.50 mm posts (KiCad's hole positions match the drawing; JLC's EasyEDA
            # model draws its posts 0.18 mm elsewhere, but the physical part follows HRO), front shell slots
            # 0.60 x 1.40 mm (KiCad's library has 1.20). The shell legs get paste (paste-in-hole).
            ypeg = [p.GetPosition().y for p in fp.Pads() if p.GetAttribute() == pcbnew.PAD_ATTRIB_NPTH][0]
            for pad in fp.Pads():
                if pad.GetAttribute() == pcbnew.PAD_ATTRIB_NPTH:
                    pad.SetDrillSize(V(mm(0.6), mm(0.6))); pad.SetSize(V(mm(0.6), mm(0.6)))
                elif pad.GetNumber() == "S1":
                    if pad.GetPosition().y > ypeg + mm(2):          # the pair at the board edge
                        d, sz = pad.GetDrillSize(), pad.GetSize()
                        if d.x < d.y:
                            pad.SetDrillSize(V(d.x, mm(1.4))); pad.SetSize(V(sz.x, mm(1.8)))
                        else:
                            pad.SetDrillSize(V(mm(1.4), d.y)); pad.SetSize(V(mm(1.8), sz.y))
                    ls = pad.GetLayerSet(); ls.AddLayer(pcbnew.F_Paste); pad.SetLayerSet(ls)
        # footprint silkscreen lines at JLC's 0.153 mm minimum (KiCad's libraries draw 0.12 mm)
        for g in fp.GraphicalItems():
            if g.GetLayer() in (pcbnew.F_SilkS, pcbnew.B_SilkS) and hasattr(g, "GetShape") and g.GetWidth() < mm(0.16):
                g.SetWidth(mm(0.16))
        # footprint silkscreen that reaches the board edge (the USB-C's outline marks the edge): drop it
        for g in list(fp.GraphicalItems()):
            if g.GetLayer() in (pcbnew.F_SilkS, pcbnew.B_SilkS) and hasattr(g, "GetShape"):
                bb = g.GetBoundingBox()
                if pcbnew.ToMM(bb.GetBottom()) > H - 0.4 or pcbnew.ToMM(bb.GetTop()) < 0.4 or pcbnew.ToMM(bb.GetLeft()) < 0.4 or pcbnew.ToMM(bb.GetRight()) > W - 0.4:
                    fp.Remove(g)
        for pad in fp.Pads():
            num = pad.GetNumber()
            if ref in SWAP and num in ("1", "2"):
                num = "2" if num == "1" else "1"
            net = p["pins"].get(num)
            if net:
                pad.SetNet(nets[net])
    if missing:
        raise SystemExit(f"no placement for: {' '.join(sorted(missing))}")

    # outline (Edge.Cuts); copper keeps 0.3 mm off the edge (DRC rule, and these guards for the router)
    seg(board, pcbnew.Edge_Cuts, rect(0, 0, W, H))
    for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
        for i, r in enumerate((rect(0, 0, W, 0.3), rect(0, H - 0.3, W, H), rect(W - 0.3, 0, W, H), rect(0, 0, 0.3, H))):
            guard(board, layer, r, f"edge {i}")

    # GND pours are added after routing (add_pours): exported as a plane, Freerouting assumes
    # the pour connects every GND pad, and the real fill leaves islands. The module's exposed
    # pad (9 sub-pads) gets its own small GND patch with two vias down to the B.Cu plane.
    u4 = board.FindFootprintByReference("U4")
    ep = [p for p in u4.Pads() if p.GetNumber() == "49"]
    exs = sorted({round(pcbnew.ToMM(p.GetPosition().x), 3) for p in ep}); eys = sorted({round(pcbnew.ToMM(p.GetPosition().y), 3) for p in ep})
    ex0, ex1, ey0, ey1 = exs[0] - 0.75, exs[-1] + 0.75, eys[0] - 0.75, eys[-1] + 0.75
    elayer = pcbnew.B_Cu if u4.IsFlipped() else pcbnew.F_Cu
    if not u4.IsFlipped():   # module on top: its own GND patch (the pours are added after routing)
        zone(board, nets["GND"], elayer, rect(ex0, ey0, ex1, ey1), clearance=0.2, priority=3, name="EPAD")
    for vx in ((exs[0] + exs[1]) / 2, (exs[1] + exs[2]) / 2):
        for vy in ((eys[0] + eys[1]) / 2, (eys[1] + eys[2]) / 2):
            v = pcbnew.PCB_VIA(board)
            v.SetPosition(V(mm(vx), mm(vy)))
            v.SetWidth(mm(0.6)); v.SetDrill(mm(0.3)); v.SetNet(nets["GND"]); v.SetLocked(True)
            board.Add(v)

    silkscreen(board, mains_v)

    return board              # all copper is routed by fastroute (ext_route.py)


def sexpr(text):
    """Parse an S-expression (Specctra DSN/SES) into nested lists of strings."""
    tok = re.findall(r'"[^"]*"|\(|\)|[^\s()"]+', text)
    stack, cur = [], []
    for t in tok:
        if t == "(":
            stack.append(cur)
            cur = []
        elif t == ")":
            done, cur = cur, stack.pop()
            cur.append(done)
        else:
            cur.append(t.strip('"'))
    return cur[0]


def find(node, key):
    return [c for c in node if isinstance(c, list) and c and c[0] == key]


def remove_stubs(board):
    """Delete track segments with a free end (Freerouting leaves a few), repeatedly."""
    removed = 0
    while True:
        tracks = [t for t in board.GetTracks() if t.Type() == pcbnew.PCB_TRACE_T and not t.IsLocked()]
        ends = {}
        for t in board.GetTracks():
            if t.Type() == pcbnew.PCB_TRACE_T:
                for p in (t.GetStart(), t.GetEnd()):
                    ends.setdefault((t.GetNetCode(), t.GetLayer(), p.x, p.y), []).append(t)
            else:  # via: joins both layers
                p = t.GetPosition()
                for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
                    ends.setdefault((t.GetNetCode(), layer, p.x, p.y), []).append(t)
        pads = [pd for fp in board.GetFootprints() for pd in fp.Pads()]
        dead = []
        for t in tracks:
            for p in (t.GetStart(), t.GetEnd()):
                others = [o for o in ends.get((t.GetNetCode(), t.GetLayer(), p.x, p.y), []) if o is not t]
                on_pad = any(pd.GetNetCode() == t.GetNetCode() and pd.HitTest(p) and pd.IsOnLayer(t.GetLayer()) for pd in pads)
                if not others and not on_pad:
                    dead.append(t)
                    break
        if not dead:
            return removed
        for t in dead:
            board.Remove(t)
        removed += len(dead)


def add_pours(board):
    gnd = board.GetNetsByName()["GND"]
    for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
        z = zone(board, gnd, layer, rect(0.3, 0.3, W - 0.3, H - 0.3), name="GND")
        z.SetPadConnection(pcbnew.ZONE_CONNECTION_FULL)
        z.SetIslandRemovalMode(pcbnew.ISLAND_REMOVAL_MODE_ALWAYS)


def write_rules(path, spec):
    """KiCad custom rules: none beyond the board's design settings (a low-voltage board)."""
    open(path, "w").write("(version 1)\n")


def parse_ses(path):
    """Specctra session -> (net, layer, width_mm, [pts]) wires and (net, padstack, x, y) vias (mm, KiCad y-down)."""
    root = sexpr(open(path).read())
    routes = find(root, "routes")[0]
    res = find(routes, "resolution") or find(find(root, "placement")[0], "resolution")
    unit, per = res[0][1], float(res[0][2])
    scale = {"um": 1e-3, "mm": 1.0, "mil": 0.0254, "inch": 25.4}[unit] / per
    wires, vias = [], []
    for net in find(find(routes, "network_out")[0], "net"):
        name = net[1]
        for w in find(net, "wire"):
            for pth in find(w, "path"):
                nums = [float(v) for v in pth[3:]]
                pts = [(nums[i] * scale, -nums[i + 1] * scale) for i in range(0, len(nums) - 1, 2)]
                wires.append((name, pth[1], float(pth[2]) * scale, pts))
        for v in find(net, "via"):
            vias.append((name, v[1], float(v[2]) * scale, -float(v[3]) * scale))
    return wires, vias


def import_ses(board, path):
    wires, vias = parse_ses(path)
    layers = {"F.Cu": pcbnew.F_Cu, "B.Cu": pcbnew.B_Cu}
    nets = board.GetNetsByName()
    n_t = n_v = 0
    for net, layer, width, pts in wires:
        ni = nets[net] if nets.has_key(net) else None
        for a, b in zip(pts, pts[1:]):
            t = pcbnew.PCB_TRACK(board)
            t.SetStart(V(mm(a[0]), mm(a[1])))
            t.SetEnd(V(mm(b[0]), mm(b[1])))
            t.SetWidth(mm(width))
            t.SetLayer(layers[layer])
            if ni:
                t.SetNet(ni)
            board.Add(t)
            n_t += 1
    for net, stack, x, y in vias:
        ni = nets[net] if nets.has_key(net) else None
        m = re.search(r"(\d+(?:\.\d+)?):(\d+(?:\.\d+)?)_um", stack)
        dia, drill = (float(m.group(1)) / 1000, float(m.group(2)) / 1000) if m else (0.8, 0.4)
        v = pcbnew.PCB_VIA(board)
        v.SetPosition(V(mm(x), mm(y)))
        v.SetWidth(mm(dia))
        v.SetDrill(mm(drill))
        v.SetViaType(pcbnew.VIATYPE_THROUGH)
        v.SetLayerPair(pcbnew.F_Cu, pcbnew.B_Cu)
        if ni:
            v.SetNet(ni)
        board.Add(v)
        n_v += 1
    return n_t, n_v


def fix_dsn(path, spec):
    """Nothing to add for a low-voltage board (kept so ext_route's flow stays the same)."""
    return []


def fab_outputs(pcb, stem):
    """Gerbers, drill, pick-and-place, PDF + SVG renders (kicad-cli)."""
    fab = stem + "-fab"
    os.makedirs(fab, exist_ok=True)
    cli = "kicad-cli"
    run = lambda *a: subprocess.run([cli, "pcb", "export", *a], capture_output=True, text=True)
    run("gerbers", pcb, "-o", fab + "/", "-l",
        "F.Cu,B.Cu,F.Paste,B.Paste,F.SilkS,B.SilkS,F.Mask,B.Mask,Edge.Cuts")
    run("drill", pcb, "-o", fab + "/", "--excellon-separate-th")   # separate PTH and NPTH files (no plating ambiguity)
    run("pos", pcb, "-o", fab + "/positions.csv", "--format", "csv", "--units", "mm", "--side", "both")
    for name, layers, mirror in (("top", "F.Cu,F.SilkS,F.Mask,Edge.Cuts", False),
                                 ("bottom", "B.Cu,B.SilkS,Edge.Cuts", True),
                                 ("all", "F.Cu,B.Cu,F.SilkS,Edge.Cuts", False)):
        args = ["svg", pcb, "-o", f"{stem}-{name}.svg", "-l", layers, "--page-size-mode", "2", "--exclude-drawing-sheet"]
        run(*(args + (["--mirror"] if mirror else [])))
    run("pdf", pcb, "-o", stem + ".pdf", "-l", "F.Cu,B.Cu,F.SilkS,Edge.Cuts")




def obstacles(board, net="GND"):
    """Other-net copper (and every hole) as simple shapes, for placing GND vias/tracks."""
    items = []
    for fp in board.GetFootprints():
        for p in fp.Pads():   # every pad, GND ones too: a via in an SMD pad wicks solder away from the joint
            bb = p.GetBoundingBox()
            items.append(("r", [pcbnew.ToMM(v) for v in (bb.GetLeft(), bb.GetTop(), bb.GetRight(), bb.GetBottom())]))
    for t in board.GetTracks():
        if t.GetNetname() == "GND" and t.Type() != pcbnew.PCB_VIA_T:
            continue
        if t.Type() == pcbnew.PCB_VIA_T:          # every via, GND too: hole-to-hole spacing
            c = t.GetPosition(); items.append(("c", (pcbnew.ToMM(c.x), pcbnew.ToMM(c.y), pcbnew.ToMM(t.GetWidth()) / 2)))
        else:
            a, b = t.GetStart(), t.GetEnd()
            items.append(("s", (pcbnew.ToMM(a.x), pcbnew.ToMM(a.y), pcbnew.ToMM(b.x), pcbnew.ToMM(b.y), pcbnew.ToMM(t.GetWidth()) / 2)))
    return items


def is_free(items, x, y, r):
    """True if a disc of radius r at (x, y) clears every obstacle."""
    import math
    for k, g in items:
        if k == "r":
            dx = max(g[0] - x, 0, x - g[2]); dy = max(g[1] - y, 0, y - g[3])
            if math.hypot(dx, dy) < r:
                return False
        elif k == "c":
            if math.hypot(x - g[0], y - g[1]) < r + g[2]:
                return False
        else:
            ax, ay, bx, by, h = g
            vx, vy = bx - ax, by - ay
            L2 = vx * vx + vy * vy
            t_ = 0 if L2 == 0 else max(0, min(1, ((x - ax) * vx + (y - ay) * vy) / L2))
            if math.hypot(x - ax - t_ * vx, y - ay - t_ * vy) < r + h:
                return False
    return True


def stitch(board, pitch=2.0, d=0.6, drill=0.3, clr=0.3):
    """GND stitching vias on a grid over the pours, wherever both layers are free of other nets."""
    gnd = board.GetNetsByName()["GND"]
    items = obstacles(board)
    n = 0
    x = 1.2
    while x < W - 1.2:
        y = 1.5
        while y < H - 1.2:
            if is_free(items, x, y, d / 2 + clr):
                v = pcbnew.PCB_VIA(board)
                v.SetPosition(V(mm(x), mm(y))); v.SetWidth(mm(d)); v.SetDrill(mm(drill)); v.SetNet(gnd)
                board.Add(v); n += 1
                items.append(("c", (x, y, d / 2)))
            y += pitch
        x += pitch
    return n


def finish_own(spec, stem):
    placed = stem + "-placed.kicad_pcb"
    b = pcbnew.LoadBoard(placed)
    if b.FindFootprintByReference("U4").IsFlipped():      # bottom-side module: the GND pour covers its pad array
        for z in list(b.Zones()):
            if z.GetZoneName() == "EPAD":
                b.Remove(z)
    add_pours(b)
    n = stitch(b, pitch=_plan.get("stitch_pitch", 2.0))
    pcb = stem + ".kicad_pcb"
    # fastroute routes at 0.2 mm; the DRC checks against 0.15 mm (JLC standard minimum is 0.127 mm), so
    # the few router neck-downs widened to 0.2 mm by ext_route still pass with margin.
    ns = b.GetDesignSettings().m_NetSettings
    ns.m_DefaultNetClass.SetClearance(mm(0.15))
    for name, nc in ns.m_NetClasses.items():
        nc.SetClearance(mm(0.15))
    pcbnew.SaveBoard(pcb, b)
    write_rules(stem + ".kicad_dru", spec)
    b = pcbnew.LoadBoard(pcb)
    b.BuildConnectivity()
    pcbnew.ZONE_FILLER(b).Fill(b.Zones())
    # drop GND vias that ended up in pour on only one layer (dangling), then refill
    zs = [z for z in b.Zones() if z.GetNetname() == "GND" and not z.GetIsRuleArea()]
    dead = []
    for t in b.GetTracks():
        if t.Type() == pcbnew.PCB_VIA_T and t.GetNetname() == "GND" and not t.IsLocked():
            on = [L for L in (pcbnew.F_Cu, pcbnew.B_Cu) if any(z.IsOnLayer(L) and z.GetFilledPolysList(L).Contains(t.GetPosition()) for z in zs)]
            if len(on) < 2:
                dead.append(t)
    for t in dead:
        b.Remove(t)
    pcbnew.ZONE_FILLER(b).Fill(b.Zones())
    pcbnew.SaveBoard(pcb, b)
    rpt = stem + "-drc.rpt"
    pcbnew.WriteDRCReport(b, rpt, pcbnew.EDA_UNITS_MILLIMETRES, True)
    b.BuildConnectivity()
    un = b.GetConnectivity().GetUnconnectedCount(False)
    vias = sum(1 for t in b.GetTracks() if t.Type() == pcbnew.PCB_VIA_T)
    fab_outputs(pcb, stem)
    print(f"finished: {n} GND stitching vias, {vias} vias total, unconnected {un}\nwrote {pcb}\nDRC: {rpt}")


def main():
    """Placement only (routing: ext_route.py, fastroute end to end)."""
    spec = json.load(open(os.path.join(BUILD, "parts-120v.json")))
    stem = os.path.join(BUILD, "coffee-controller-120v")
    board = build(spec, 120)
    pcbnew.SaveBoard(stem + "-placed.kicad_pcb", board)
    write_rules(stem + "-placed.kicad_dru", spec)
    print("placed:", stem + "-placed.kicad_pcb")


if __name__ == "__main__":
    main()
