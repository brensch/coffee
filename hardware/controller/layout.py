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

import argparse
import json
import os
import re
import subprocess
import sys

import pcbnew
from pcbnew import FromMM as mm, VECTOR2I as V

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.path.join(HERE, "build")
FP_DIR = os.environ.get("KICAD7_FOOTPRINT_DIR", "/usr/share/kicad/footprints")
# Freerouting 2.1.0 jar (Java 21). The 2.5.0 native CLI hangs at 0% CPU after a serialisation crash.
FREEROUTING_JAR = os.environ.get("FREEROUTING_JAR", os.path.expanduser("~/opt/freerouting/fr-2.1.0.jar"))

# ---------------------------------------------------------------- geometry (mm)
W, H = 99.0, 71.0           # board
BAND = (31.5, 38.5)         # creepage band: no copper on any layer (7 mm > 5 mm reinforced, 250 V PD2)
SLOT_X = 35.0               # milled isolation slot (2 mm) through the band, under the optos and PSU
SLOT_W = 2.0
SLOTS = [(3.0, 25.5), (27.5, 67.5)]   # y ranges; a web of board is left between slots for stiffness

# ref: (x, y, rotation_deg, side) - side 'F' (top) or 'B'. Mains side is x < 42.5.
PLACE = {
    # ================= MAINS (x < 31.5) =================
    # Lanes, top to bottom: supply, zero cross, valve, pump, heater. Terminals on the left
    # edge (wires enter from the left); the heater triac sits on the left edge (tab to chassis).
    "J1": (5.2, 4.5, 270, "F"),     # L (5.2,4.5), N (5.2,9.58)
    "F1": (13.5, 4.5, 0, "F"),      # AC_L 13.5, L_F 18.58
    "RV1": (13.5, 15.5, 0, "F"),    # L_F 13.5, N 18.5
    "PS1": (26.0, 11.0, 0, "F"),    # AC (26,11) L_F, (26,17) N; DC (59.6,5) GND, (59.6,23) +5V
    "R1": (24.0, 29.0, 0, "F"), "R2": (24.0, 34.0, 0, "F"),
    "U1": (29.92, 30.0, 0, "F"),    # H11AA1: AC1 (29.92,30), AC2 (29.92,32.54)
    # valve lane: U4 gate (29.92,40.32), opto (29.92,45.4)
    "U4": (40.08, 45.4, 180, "F"),
    "J3": (5.2, 41.5, 270, "F"),    # valve (5.2,41.5), pump (5.2,46.58)
    "Q3": (15.0, 38.5, 90, "F"),    # pins y=41.65: MT1 12.7, MT2 15.0, G 17.3; tab (MT2) y=35.35
    "R10": (15.0, 45.0, 180, "F"),  # gate-MT1 (G pin3 -> pad1, MT1 pin1 -> pad2); MT2 is the tab
    "R9": (25.0, 45.4, 0, "F"),     # MT2 -> opto
    "R11": (15.5, 28.5, 0, "F"), "C22": (10.5, 22.0, 0, "F"),   # snubber
    # pump lane: U3 gate (29.92,49.52), opto (29.92,54.6)
    "U3": (40.08, 54.6, 180, "F"),
    "Q2": (14.0, 51.5, 0, "F"),     # TO-220 tab up: MT1 14, MT2 16.54, G 19.08 (y=51.5)
    "R7": (10.0, 54.5, 90, "F"),    # gate-MT1, left of the pins
    "R6": (27.5, 52.0, 90, "F"),
    "R8": (23.8, 50.5, 90, "F"), "C21": (23.0, 65.0, 90, "F"),   # snubber (C21 stands in the strip by U2)
    # heater: Q1 on the left edge, J2 on the bottom edge, copper pours between
    "U2": (40.08, 64.5, 180, "F"),  # gate (29.92,59.42), opto (29.92,64.5)
    "Q1": (3.6, 62.0, 90, "F"),     # pin1 MT1 (3.6,62) HTR_A, pin2 MT2 (3.6,59.46) HTR_B, pin3 G (3.6,56.92)
    "J2": (11.5, 65.0, 0, "F"),     # HTR_A (11.5,65), HTR_B (16.58,65)
    "R3": (23.6, 68.6, 0, "F"), "R4": (4.4, 68.3, 0, "F"),   # R3 pad 1 sits in the HTR_B pour
    # ================= SELV (x > 38.5) =================
    # power: PSU 5 V (59.6, 23) -> D1 -> +5V; USB VBUS -> D2; LDO
    "D1": (68.5, 22.0, 0, "F"), "D2": (68.5, 26.5, 0, "F"), "C1": (60.5, 33.5, 90, "F"),
    "U7": (69.0, 35.0, 0, "F"), "C2": (74.0, 28.5, 90, "F"), "C3": (75.0, 35.0, 90, "F"),
    "U8": (89.0, 12.5, 0, "F"),     # ESP32-S3, antenna at the top edge
    "C4": (76.0, 9.0, 90, "F"), "C5": (74.0, 9.0, 90, "F"),
    "R20": (76.0, 15.0, 90, "F"), "C6": (74.0, 15.0, 90, "F"),
    "R48": (76.0, 20.5, 90, "F"), "D9": (74.0, 20.5, 90, "F"),
    # zero-cross output
    "R24": (43.5, 31.0, 90, "F"), "C7": (45.5, 31.0, 90, "F"),
    # triac drivers, one row per opto (opto LED anode pin1 at y0, cathode pin2 at y0-2.54)
    "D5": (43.5, 45.4, 0, "F"), "R31": (47.0, 45.4, 180, "F"), "Q6": (44.0, 41.0, 0, "F"), "R32": (47.5, 40.0, 0, "F"), "R33": (50.0, 42.0, 90, "F"),
    "D4": (43.5, 54.6, 0, "F"), "R28": (47.0, 54.6, 180, "F"), "Q5": (44.0, 50.2, 0, "F"), "R29": (47.5, 49.2, 0, "F"), "R30": (50.0, 51.2, 90, "F"),
    "D3": (43.5, 64.5, 0, "F"), "R25": (47.0, 64.5, 180, "F"), "Q4": (44.0, 60.1, 0, "F"), "R26": (47.5, 59.1, 0, "F"), "R27": (50.0, 61.1, 90, "F"),
    # switches
    "J8": (56.0, 67.5, 0, "F"), "R44": (54.0, 57.5, 90, "F"), "R45": (56.0, 57.5, 90, "F"), "C14": (58.0, 57.5, 90, "F"), "D7": (53.5, 61.0, 0, "F"),
    "R46": (60.0, 57.5, 90, "F"), "R47": (62.0, 57.5, 90, "F"), "C15": (64.0, 57.5, 90, "F"), "D8": (60.5, 61.0, 0, "F"),
    # ADC + analog front end
    "U11": (60.0, 47.5, 0, "F"), "C10": (60.0, 43.5, 0, "F"),
    "R35": (55.0, 44.0, 90, "F"), "R36": (57.0, 44.0, 90, "F"), "R37": (64.5, 44.0, 90, "F"),
    "J6": (65.5, 67.5, 0, "F"), "D6": (68.5, 60.5, 0, "F"), "R38": (66.0, 52.5, 90, "F"), "R39": (68.0, 52.5, 90, "F"), "C11": (70.0, 52.5, 90, "F"),
    "R40": (53.0, 50.0, 90, "F"), "R41": (55.0, 50.0, 90, "F"), "C12": (54.5, 39.5, 90, "F"),
    "J7": (74.5, 67.5, 0, "F"), "R42": (72.0, 52.5, 90, "F"), "C13": (74.0, 52.5, 90, "F"), "R43": (66.5, 44.0, 90, "F"),
    "J9": (43.8, 67.8, 0, "F"),
    # thermocouple: right edge, away from the PSU, LDO and triacs
    "J5": (96.5, 36.0, 270, "F"), "C9": (91.5, 33.0, 90, "F"),
    "U10": (90.5, 43.5, 0, "F"), "C8": (85.0, 43.5, 90, "F"), "R34": (85.0, 48.5, 90, "F"),
    # USB (flashing), buttons
    "J4": (84.5, 67.0, 0, "F"), "U9": (84.5, 59.5, 0, "F"), "R22": (81.0, 60.5, 90, "F"), "R23": (88.0, 60.5, 90, "F"),
    "SW1": (94.0, 50.5, 0, "F"), "SW2": (94.0, 57.5, 0, "F"), "R21": (88.0, 52.0, 90, "F"),
    # mounting holes, clear of mains copper (the chassis standoffs are earthed)
    "H1": (4.0, 26.0, 0, "F"), "H2": (95.5, 29.5, 0, "F"), "H3": (79.5, 41.5, 0, "F"), "H4": (95.5, 67.0, 0, "F"),
}


def load_fp(spec):
    lib, name = spec.split(":", 1)
    fp = pcbnew.FootprintLoad(os.path.join(FP_DIR, lib + ".pretty"), name)
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


def pad_pos(board, ref, num):
    fp = board.FindFootprintByReference(ref)
    for p in fp.Pads():
        if p.GetNumber() == num:
            pos = p.GetPosition()
            return (pcbnew.ToMM(pos.x), pcbnew.ToMM(pos.y))
    raise KeyError(f"{ref}.{num}")


def build(spec, mains_v):
    board = pcbnew.BOARD()
    board.SetCopperLayerCount(2)
    ds = board.GetDesignSettings()
    ds.SetCopperLayerCount(2)
    ds.m_TrackMinWidth = mm(0.2)
    ds.m_MinClearance = mm(0.2)
    ds.m_ViasMinSize = mm(0.6)
    ds.m_MinThroughDrill = mm(0.2)   # the S3 module footprint has 0.2 mm thermal vias
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
        "Mains": netclass("Mains", 0.8, 1.0, via=1.2, drill=0.6),   # functional insulation between mains nets (coat the mains side)
        "Heater": netclass("Heater", 2.5, 1.0, via=1.6, drill=0.8),
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
        fp.SetPosition(V(mm(x), mm(y)))
        if side == "B":
            fp.Flip(fp.GetPosition(), False)
        fp.SetOrientationDegrees(rot)
        for pad in fp.Pads():
            num = pad.GetNumber()
            net = p["pins"].get(num)
            if net is None and num == "" and ref.startswith("U") and "ESP32" in p["footprint"]:
                net = "GND"  # thermal pad vias under the module
            if net:
                pad.SetNet(nets[net])
        board.Add(fp)
    if missing:
        raise SystemExit(f"no placement for: {' '.join(sorted(missing))}")

    # outline + slots (Edge.Cuts) and the creepage band keep-out
    seg(board, pcbnew.Edge_Cuts, rect(0, 0, W, H))
    for y0, y1 in SLOTS:
        seg(board, pcbnew.Edge_Cuts, rect(SLOT_X - SLOT_W / 2, y0, SLOT_X + SLOT_W / 2, y1))
    zone(board, None, pcbnew.F_Cu, rect(BAND[0], 0, BAND[1], H), keepout=True, name="ISOLATION BAND")
    # keep copper away from the M3 holes on the mains side (earthed chassis standoffs)
    zone(board, None, pcbnew.F_Cu, rect(0, 21.5, 8.5, 30.5), keepout=True, name="H1 clearance", pads_ok=True)

    # The ESP32-S3 footprint carries Espressif's antenna keep-out itself.
    # GND pours are added after routing (add_pours): exported as a plane, Freerouting assumes
    # the pour connects every GND pad, and the real fill leaves islands.

    # silkscreen: mark the barrier and the mains side
    for txt, x, y, size in (("MAINS", 16, 28.5, 1.5), ("SELV", 45, 27.5, 1.5),
                            (f"coffee v2 {mains_v} V", 62, 70.2, 1.0)):
        t = pcbnew.PCB_TEXT(board)
        t.SetText(txt)
        t.SetPosition(V(mm(x), mm(y)))
        t.SetLayer(pcbnew.F_SilkS)
        t.SetTextSize(V(mm(size), mm(size)))
        t.SetTextThickness(mm(size * 0.15))
        board.Add(t)
    seg(board, pcbnew.F_SilkS, [(BAND[0], 4), (BAND[0], H - 4)], width=0.3, closed=False)
    seg(board, pcbnew.F_SilkS, [(BAND[1], 4), (BAND[1], H - 4)], width=0.3, closed=False)

    # Heater current path as copper pours (solid pad connections). HTR_B leaves the triac's
    # middle pin on F.Cu only, as wide as the TO-220 pin pitch allows (~2 mm, about the pad
    # width) and widening to 4.5 mm once clear of the neighbouring pins; B.Cu under that band
    # stays free for the gate resistor's link into the HTR_A pocket.
    def pour(net, layer, pts, clearance):
        z = zone(board, nets[net], layer, pts, clearance=clearance, priority=2, name=net)
        z.SetPadConnection(pcbnew.ZONE_CONNECTION_FULL)
        z.SetMinThickness(mm(0.3))
    # Q1 pins: pin3 G y=56.92, pin2 MT2 (HTR_B) y=59.46, pin1 MT1 (HTR_A) y=62.0, all x=3.6.
    # The band out of the middle pin is bounded by pins 1 and 3 (pads +-1.0 mm, 0.5 mm
    # clearance): y 58.42..60.5, 2.1 mm, on both layers; it joins the 6 mm column to J2.2.
    band = [(2.0, 58.42), (14.4, 58.42), (14.4, 56.0), (20.5, 56.0), (20.5, 67.4), (23.0, 67.4), (23.0, 70.4), (14.4, 70.4), (14.4, 60.5), (2.0, 60.5)]
    band_b = [(5.2, 58.42), (20.5, 58.42), (20.5, 70.4), (14.4, 70.4), (14.4, 60.5), (5.2, 60.5)]   # B.Cu: band + column, below the gate track
    pocket = [(0.6, 61.5), (13.2, 61.5), (13.2, 70.4), (0.6, 70.4)]
    pour("HTR_B", pcbnew.F_Cu, band, 0.5)
    pour("HTR_B", pcbnew.B_Cu, band_b, 0.5)
    pour("HTR_A", pcbnew.F_Cu, pocket, 0.5)

    # Heater gate, hand-routed on B.Cu (B.Cu has no heater copper above the band):
    #   Q1.G -> along y=55.2 -> down x=26.5 -> U2 pin 4 (opto gate side);
    #   Q1.G -> left edge x=1.1 -> via -> R4 pad 1 (gate-MT1 resistor in the HTR_A pocket).
    g = nets["HEATER_GATE"]
    q1g, u2g, r4g = pad_pos(board, "Q1", "3"), pad_pos(board, "U2", "4"), pad_pos(board, "R4", "1")
    gw = 0.6
    for a_, b_ in ((q1g, (q1g[0] + 1.2, 55.2)), ((q1g[0] + 1.2, 55.2), (26.5, 55.2)), ((26.5, 55.2), (26.5, u2g[1])), ((26.5, u2g[1]), u2g)):
        track(board, g, pcbnew.B_Cu, a_, b_, gw)
    via_pt = (1.1, r4g[1] - 2.6)
    for a_, b_ in ((q1g, (1.1, q1g[1])), ((1.1, q1g[1]), via_pt)):
        track(board, g, pcbnew.B_Cu, a_, b_, gw)
    v = pcbnew.PCB_VIA(board)
    v.SetPosition(V(mm(via_pt[0]), mm(via_pt[1])))
    v.SetWidth(mm(1.0)); v.SetDrill(mm(0.5)); v.SetNet(g); v.SetLocked(True)
    board.Add(v)
    track(board, g, pcbnew.F_Cu, via_pt, r4g, gw)
    return board


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
        zone(board, gnd, layer, rect(BAND[1] + 0.5, 0.5, W - 0.5, H - 0.5), name="GND")


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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mains", type=int, default=230)
    ap.add_argument("--passes", type=int, default=40)
    ap.add_argument("--no-route", action="store_true")
    ap.add_argument("--attempts", type=int, default=6)
    ap.add_argument("--snap", choices=["90", "45"], default="45")
    a = ap.parse_args()
    spec = json.load(open(os.path.join(BUILD, f"parts-{a.mains}v.json")))
    stem = os.path.join(BUILD, f"coffee-controller-{a.mains}v")
    board = build(spec, a.mains)
    pcbnew.SaveBoard(stem + "-placed.kicad_pcb", board)
    if a.no_route:
        print("placed:", stem + "-placed.kicad_pcb")
        return
    dsn, ses = stem + ".dsn", stem + ".ses"
    # Fill the heater pours first, so the router sees them as copper to keep clear of.
    board = pcbnew.LoadBoard(stem + "-placed.kicad_pcb")
    board.BuildConnectivity()
    pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    pcbnew.SaveBoard(stem + "-placed.kicad_pcb", board)
    if not pcbnew.ExportSpecctraDSN(board, dsn):
        raise SystemExit("DSN export failed")
    # Manhattan routing (90-degree corners only) reads far neater than 45-degree spaghetti.
    if a.snap == "90":
        txt = open(dsn).read()
        txt = txt.replace("(structure", "(structure\n    (snap_angle ninety_degree)", 1)
        open(dsn, "w").write(txt)
    def freeroute(dsn_path, ses_path, passes, timeout):
        if os.path.exists(ses_path):
            os.remove(ses_path)
        try:
            r = subprocess.run(["java", "-Djava.awt.headless=true", "-jar", FREEROUTING_JAR, "--gui.enabled=false",
                                "-de", dsn_path, "-do", ses_path, "-mp", str(passes), "-mt", "4"],
                               capture_output=True, text=True, timeout=timeout)
            open(stem + "-freerouting.log", "a").write(r.stdout + r.stderr)
        except subprocess.TimeoutExpired:
            return False
        return os.path.exists(ses_path)

    def finish(board):
        board.BuildListOfNets()
        board.BuildConnectivity()
        return board.GetConnectivity().GetUnconnectedCount(False)

    # Stage 1: Manhattan (90 degree) for the bulk of the board. Stage 2: whatever is left
    # gets a 45-degree pass on top, with the stage-1 copper kept. Freerouting is
    # multi-threaded and not deterministic, so the whole thing retries.
    open(stem + "-freerouting.log", "w").close()
    best = None
    for attempt in range(1, a.attempts + 1):
        if not freeroute(dsn, ses, a.passes, 600):
            print(f"attempt {attempt}: stage 1 produced no session", flush=True)
            continue
        # Work on the board as KiCad loads it from disk: zone filling segfaults on a board
        # that was only ever built in memory (KiCad 7 standalone pcbnew).
        board = pcbnew.LoadBoard(stem + "-placed.kicad_pcb")
        n_t, n_v = import_ses(board, ses)
        stubs = remove_stubs(board)
        unrouted = finish(board)
        stage = "90"
        if unrouted:
            dsn2, ses2 = stem + "-stage2.dsn", stem + "-stage2.ses"
            pcbnew.ExportSpecctraDSN(board, dsn2)   # existing copper goes along as wiring
            if freeroute(dsn2, ses2, 20, 300):
                board = pcbnew.LoadBoard(stem + "-placed.kicad_pcb")
                n_t, n_v = import_ses(board, ses2)
                stubs = remove_stubs(board)
                unrouted = finish(board)
                stage = "90+45"
        add_pours(board)
        board.BuildConnectivity()
        pcbnew.ZONE_FILLER(board).Fill(board.Zones())
        unrouted = finish(board)
        print(f"attempt {attempt}: {n_t} segments, {n_v} vias, {stubs} stubs removed, unconnected {unrouted} ({stage})", flush=True)
        if best is None or unrouted < best[0]:
            best = (unrouted, board, n_t, n_v, stubs)
        if unrouted == 0:
            break
    if best is None:
        raise SystemExit("routing failed")
    unrouted, board, n_t, n_v, stubs = best
    pcb = stem + ".kicad_pcb"
    pcbnew.SaveBoard(pcb, board)
    rpt = stem + "-drc.rpt"
    pcbnew.WriteDRCReport(board, rpt, pcbnew.EDA_UNITS_MILLIMETRES, True)
    fab_outputs(pcb, stem)
    print(f"routed: {n_t} track segments ({stubs} stubs removed), {n_v} vias, unconnected: {unrouted}\nwrote {pcb}\nDRC: {rpt}")

if __name__ == "__main__":
    main()
