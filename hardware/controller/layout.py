#!/usr/bin/env python3
"""Build, route and check the controller PCB from controller.py's output.

Runs in KiCad's Python (pcbnew), so it reads build/parts-<V>v.json rather than
importing SKiDL. Pipeline:

  1. place footprints (positions below), draw the outline, the isolation slots
     and the creepage keep-out band, add GND pours on the SELV side;
  2. hand-route the heater current path (3 mm, short, both layers);
  3. export Specctra DSN -> Freerouting -> import the session (.ses);
  4. fill zones, run DRC, write the .kicad_pcb plus gerbers/drill/renders.

    source ~/opt/kienv.sh            # KiCad 7 libraries + pcbnew on PYTHONPATH
    python3 layout.py [--mains 230] [--passes 40]
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
# The floorplan comes from the layout sandbox (sb.py / opt.py): board size, the barrier and every
# part's (x, y, rotation, side). Mains zone is x < BAND[0], SELV x > BAND[1].
PLAN = os.environ.get("PLACE_JSON", os.path.join(HERE, "plan.json"))
_plan = json.load(open(PLAN))
W, H = _plan["W"], _plan["H"]
BAND = (max(x for x, _ in _plan["mains"]), min(x for x, _ in _plan["selv"]))   # 7 mm, no copper on any layer
SLOT_X = (BAND[0] + BAND[1]) / 2                 # milled isolation slot through the band
SLOT_W = 2.0
SLOTS = [(2.0, H / 2 - 0.75), (H / 2 + 0.75, H - 2.0)]   # a web of board in the middle for stiffness
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
    """Wiring labels at every connector, the barrier marks, the bean, and the board name."""
    # silkscreen: what plugs in where. Labels sit just outside each connector's body on the inward
    # side (anything under a screw terminal is hidden once it's fitted), one per pad, then the title.
    LABELS = {"J1": ("MAINS IN", {"1": "N", "2": "L"}), "J2": ("HEATER", {}), "J3": ("", {"1": "VALVE", "2": "PUMP"}),
              "J5": ("NTC", {}), "J6": ("PRESSURE", {"1": "+5V", "2": "SIG", "3": "GND"}),
              "J7": ("SWITCHES", {"1": "BREW", "2": "STEAM", "3": "GND"})}
    def silk(txt, x, y, size=1.0, angle=0, just=0):
        t = pcbnew.PCB_TEXT(board)
        t.SetText(txt); t.SetPosition(V(mm(x), mm(y))); t.SetLayer(pcbnew.F_SilkS)
        t.SetTextSize(V(mm(size), mm(size))); t.SetTextThickness(mm(max(0.15, size * 0.15)))
        t.SetTextAngleDegrees(angle)
        t.SetHorizJustify({-1: pcbnew.GR_TEXT_H_ALIGN_LEFT, 0: pcbnew.GR_TEXT_H_ALIGN_CENTER, 1: pcbnew.GR_TEXT_H_ALIGN_RIGHT}[just])
        board.Add(t)
    fps = {f.GetReference(): f for f in (footprints or board.GetFootprints())}
    for ref, (title, pads) in LABELS.items():
        fp = fps.get(ref)
        if fp is None:
            continue
        cb = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd).BBox()
        x0, y0, x1, y1 = (pcbnew.ToMM(v) for v in (cb.GetLeft(), cb.GetTop(), cb.GetRight(), cb.GetBottom()))
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        # inward = away from the nearest board edge
        d = {"L": x0, "R": W - x1, "T": y0, "B": H - y1}
        edge = min(d, key=d.get)
        pp = {p.GetNumber(): (pcbnew.ToMM(p.GetPosition().x), pcbnew.ToMM(p.GetPosition().y)) for p in fp.Pads()}
        size = 1.0
        for num, txt in pads.items():
            px, py = pp[num]
            if edge == "L":
                silk(txt, x1 + 0.6, py, size, 0, -1)
            elif edge == "R":
                silk(txt, x0 - 0.6, py, size, 0, 1)
            elif edge == "B":
                silk(txt, px, y0 - 0.9, size, 0, 0)
            else:
                silk(txt, px, y1 + 0.9, size, 0, 0)
        if title:
            if edge in ("L", "R") and pads:
                # a heading directly above the label column, aligned with it
                ytop = min(pp[n][1] for n in pads) - 1.7
                silk(title, (x1 + 0.6) if edge == "L" else (x0 - 0.6), ytop, size, 0, -1 if edge == "L" else 1)
            elif edge in ("L", "R"):
                silk(title, (x1 + 0.6) if edge == "L" else (x0 - 0.6), cy, size, 0, -1 if edge == "L" else 1)
            else:
                silk(title, cx, (y0 - 2.4) if edge == "B" else (y1 + 2.4), size, 0, 0)

    # free strips: between the fuse and the PSU (mains), and the open top-side area right of the optos (SELV)
    for txt, x, y, size in ((f"{mains_v}V MAINS", 13.0, 17.0, 1.0), ("SELV", BAND[1] + 6.0, 26.5, 1.0),
                            ):
        t = pcbnew.PCB_TEXT(board)
        t.SetText(txt)
        t.SetPosition(V(mm(x), mm(y)))
        t.SetLayer(pcbnew.F_SilkS)
        t.SetTextSize(V(mm(size), mm(size)))
        t.SetTextThickness(mm(size * 0.15))
        if txt.endswith("MAINS"):
            t.SetTextAngleDegrees(90)
        board.Add(t)
    seg(board, pcbnew.F_SilkS, [(BAND[0], 4), (BAND[0], H - 4)], width=0.3, closed=False)
    seg(board, pcbnew.F_SilkS, [(BAND[1], 4), (BAND[1], H - 4)], width=0.3, closed=False)
    bean_logo(board, BAND[1] + 6.2, 34.0, 11.0)


def build(spec, mains_v):
    board = pcbnew.BOARD()
    board.SetCopperLayerCount(2)
    ds = board.GetDesignSettings()
    ds.SetCopperLayerCount(2)
    ds.m_TrackMinWidth = mm(0.2)
    ds.m_MinClearance = mm(0.2)
    ds.m_ViasMinSize = mm(0.6)
    ds.m_MinThroughDrill = mm(0.3)
    ds.m_CopperEdgeClearance = mm(0.5)
    ds.m_HoleClearance = mm(0.15)    # USB4105 footprint: shell holes sit 0.2 mm from pads

    # nets
    nets = {}
    names = sorted({n for p in spec["parts"].values() for n in p["pins"].values() if n})
    for name in names:
        ni = pcbnew.NETINFO_ITEM(board, name)
        board.Add(ni)
        nets[name] = ni

    # net classes: SELV signals, SELV power, mains, heater
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
    classes = {
        "Power": netclass("Power", 0.6, 0.25),
        "Mains": netclass("Mains", 0.8, 2.0, via=1.2, drill=0.6),   # functional insulation between mains nets (IEC 60664: ~1.6 mm at 120 V)
        "Heater": netclass("Heater", 2.5, 2.0, via=1.6, drill=0.8),
    }
    power = {"GND", "+5V", "+3V3", "VBUS", "5V_PSU"}
    for name, ni in nets.items():
        cls = "Heater" if name in spec["heater_nets"] else "Mains" if name in spec["mains_nets"] else "Power" if name in power else None
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
        if not ref.startswith(("Q", "U1", "U2", "U3", "F", "PS", "TP")):
            fp.Reference().SetVisible(False)      # connectors carry their wiring labels instead
        for pad in fp.Pads():
            num = pad.GetNumber()
            if ref in SWAP and num in ("1", "2"):
                num = "2" if num == "1" else "1"
            net = p["pins"].get(num)
            if net:
                pad.SetNet(nets[net])
    if missing:
        raise SystemExit(f"no placement for: {' '.join(sorted(missing))}")

    # outline + slots (Edge.Cuts) and the creepage band keep-out
    seg(board, pcbnew.Edge_Cuts, rect(0, 0, W, H))
    for y0, y1 in SLOTS:
        seg(board, pcbnew.Edge_Cuts, rect(SLOT_X - SLOT_W / 2, y0, SLOT_X + SLOT_W / 2, y1))
    zone(board, None, pcbnew.F_Cu, rect(BAND[0], 0, BAND[1], H), keepout=True, name="ISOLATION BAND")
    for layer in (pcbnew.F_Cu, pcbnew.B_Cu):
        for i, r in enumerate((rect(0, 0, W, 0.8), rect(0, H - 0.8, W, H), rect(W - 0.8, 0, W, H), rect(0, 0, 0.8, H))):
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

    if os.environ.get("PLACE_ONLY"):   # floorplanning: footprints, outline and band only
        return board
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
        z = zone(board, gnd, layer, rect(BAND[1] + 0.5, 0.8, W - 0.8, H - 0.8), name="GND")
        z.SetPadConnection(pcbnew.ZONE_CONNECTION_FULL)
        z.SetIslandRemovalMode(pcbnew.ISLAND_REMOVAL_MODE_ALWAYS)


# Mains nets within a few volts of each other in normal operation: 0.5 mm between them is enough.
# (Not the fuse: if F1 blows, AC_L and AC_L_FUSED have full line voltage between them.)
SAME_POTENTIAL_GROUPS = [
    # L and the nets hanging off it through a resistor that carries ~nothing: the opto feeds
    # (360 R, no current while off) and the snubber mid-points (100 R carrying the snubber
    # capacitor's ~0.5 mA: 0.05 V; at turn-on the zero-cross opto fires below ~20 V).
    ("AC_L", "PUMP_OPTO", "VALVE_OPTO", "PUMP_SNUB", "VALVE_SNUB"),
    ("PUMP_L", "PUMP_GATE"), ("VALVE_L", "VALVE_GATE"), ("HTR_A", "HEATER_GATE"),   # gate <-> MT1
    ("HTR_B", "HEATER_OPTO"),
]
SAME_POTENTIAL = [(g[i], g[j]) for g in SAME_POTENTIAL_GROUPS for i in range(len(g)) for j in range(i + 1, len(g))]
MAINS_PARTS = ["Q1", "Q2", "Q3", "U1", "U2", "U3", "PS1"]   # parts whose own pin pitch governs inside their courtyard
F_, B_ = pcbnew.F_Cu, pcbnew.B_Cu
# (net, width, layers): None = router picks layers (vias allowed); heater copper on both layers
MAINS_ORDER = [("HTR_A", 2.0, [None, F_, B_]), ("HTR_B", 2.0, [None, F_, B_]),
               ("AC_L_FUSED", 0.8, [None]), ("AC_N", 0.8, [None]), ("AC_L", 0.8, [None]),
               ("PUMP_L", 0.8, [None]), ("VALVE_L", 0.8, [None]),
               ("HEATER_GATE", 0.5, [None]), ("PUMP_GATE", 0.5, [None]), ("VALVE_GATE", 0.5, [None]),
               ("HEATER_OPTO", 0.5, [None]), ("PUMP_OPTO", 0.5, [None]), ("VALVE_OPTO", 0.5, [None]),
               ("PUMP_SNUB", 0.8, [None]), ("VALVE_SNUB", 0.8, [None])]


def write_rules(path, spec):
    """KiCad design rules = the mains lint. 2.0 mm between mains nets everywhere, except
    (a) inside one part's footprint, where the manufacturer's pin spacing governs, and
    (b) net pairs at (nearly) the same potential. Reinforced isolation is the 7 mm band."""
    nets = spec["mains_nets"]
    isin = lambda side: "(" + " || ".join(f"{side}.NetName == '{n}'" for n in nets) + ")"
    # Both items inside the same part's courtyard: that part's own pins and the copper leaving them.
    # ("A or B inside" would let any neighbour's pad sit 0.5 mm from an opto or triac pin.)
    inside = " || ".join(f"(A.insideCourtyard('{r}') && B.insideCourtyard('{r}'))" for r in MAINS_PARTS)
    pairs = " || ".join(f"(A.NetName == '{a}' && B.NetName == '{b}') || (A.NetName == '{b}' && B.NetName == '{a}')" for a, b in SAME_POTENTIAL)
    open(path, "w").write(f"""(version 1)
(rule "MAINS: 2.0 mm between mains nets (IEC 60664 functional, 120 V: ~1.6 mm)"
  (condition "{isin('A')} && {isin('B')}")
  (constraint clearance (min 2.0mm)))
(rule "MAINS: pins of one part (manufacturer's pitch)"
  (condition "{isin('A')} && {isin('B')} && ({inside})")
  (constraint clearance (min 0.5mm)))
(rule "MAINS: same-potential pairs"
  (condition "{pairs}")
  (constraint clearance (min 0.5mm)))
""")


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
    """Give Freerouting the same mains rules as the DRC. KiCad 7's Python API doesn't carry netclass
    membership into the Specctra export (every net lands in kicad_default), so: one DSN class per
    same-potential group of mains nets (0.5 mm inside it) and 2.0 mm between every pair of
    mains classes (class_class rules). SELV nets stay in the default class."""
    txt = open(path).read()
    m = re.search(r'\(class kicad_default ""(.*?)\n      \(circuit', txt, re.S)
    names = re.findall(r'"[^"]*"|\S+', m.group(1))
    plain = lambda n: n.strip('"')
    present = {plain(n): n for n in names}
    mains = [n for n in spec["mains_nets"] if n in present]
    groups = [[n for n in g if n in present] for g in SAME_POTENTIAL_GROUPS]
    grouped = {n for g in groups for n in g}
    groups = [g for g in groups if g] + [[n] for n in mains if n not in grouped]
    rest = [n for n in names if plain(n) not in mains]
    txt = txt[:m.start(1)] + " " + " ".join(rest) + txt[m.end(1):]
    um = lambda v: f"{v * 1000:.1f}"
    blocks = []
    for i, g in enumerate(groups):
        heater = any(n in spec["heater_nets"] for n in g)
        width, via = (2.5, "Via[0-1]_1600:800_um") if heater else (0.8, "Via[0-1]_1200:600_um")
        blocks.append(f"    (class M{i} {' '.join(present[n] for n in g)}\n      (circuit\n        (use_via {via})\n      )\n"
                      f"      (rule\n        (width {um(width)})\n        (clearance {um(0.5)})\n      )\n    )\n")
    for i in range(len(groups)):
        for j in range(i + 1, len(groups)):
            c = 2.0
            blocks.append(f"    (class_class (classes M{i} M{j})\n      (rule (clearance {um(c)}))\n    )\n")
    k = txt.index("    (class Heater")
    txt = txt[:k] + "".join(blocks) + txt[k:]
    open(path, "w").write(txt)


def fab_outputs(pcb, stem):
    """Gerbers, drill, pick-and-place, PDF + SVG renders (kicad-cli)."""
    fab = stem + "-fab"
    os.makedirs(fab, exist_ok=True)
    cli = "kicad-cli"
    run = lambda *a: subprocess.run([cli, "pcb", "export", *a], capture_output=True, text=True)
    run("gerbers", pcb, "-o", fab + "/", "-l",
        "F.Cu,B.Cu,F.Paste,B.Paste,F.SilkS,B.SilkS,F.Mask,B.Mask,Edge.Cuts")
    run("drill", pcb, "-o", fab + "/")
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
        for p in fp.Pads():
            if p.GetNetname() != "GND" or p.GetDrillSize().x > 0:     # holes: keep vias clear of them too
                bb = p.GetBoundingBox()
                g = 0.25 if p.GetNetname() == "GND" else 0.0
                items.append(("r", [pcbnew.ToMM(v) for v in (bb.GetLeft(), bb.GetTop(), bb.GetRight(), bb.GetBottom())]))
                if g:
                    items[-1] = ("r", [items[-1][1][0] - g, items[-1][1][1] - g, items[-1][1][2] + g, items[-1][1][3] + g])
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


def stitch(board, pitch=2.5, d=0.6, drill=0.3, clr=0.3):
    """GND stitching vias on a grid over the SELV pours, wherever both layers are free of other nets."""
    gnd = board.GetNetsByName()["GND"]
    items = obstacles(board)
    n = 0
    x = BAND[1] + 1.5
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
    n = stitch(b)
    pcb = stem + ".kicad_pcb"
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
