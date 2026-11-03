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
import time

import pcbnew
from pcbnew import FromMM as mm, VECTOR2I as V

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.environ.get("BUILD_DIR", os.path.join(HERE, "build"))
FP_DIR = os.environ.get("KICAD7_FOOTPRINT_DIR", "/usr/share/kicad/footprints")
# Freerouting 2.1.0 jar (Java 21). The 2.5.0 native CLI hangs at 0% CPU after a serialisation crash.
FREEROUTING_JAR = os.environ.get("FREEROUTING_JAR", os.path.expanduser("~/opt/freerouting/fr-2.1.0.jar"))

# ---------------------------------------------------------------- geometry (mm)
W, H = 72.0, 60.5           # board
BAND = (34.5, 41.5)         # creepage band: no copper on any layer (7 mm > 5 mm reinforced, 250 V PD2)
SLOT_X = 38.0               # milled isolation slot (2 mm) through the band, under the optos and PSU
SLOT_W = 2.0
SLOTS = [(2.5, 22.5), (24.0, 57.5)]   # y ranges; a web of board is left between slots for stiffness

# ref: (x, y, rotation_deg, side) - side 'F' (top) or 'B'. Mains x < 34.5, SELV x > 41.5.
PLACE = {
    # ================= MAINS =================
    # supply: J1 -> F1 -> RV1 -> PS1
    "J1": (5.05, 4.0, 270, "F"),    # L (5.05,4), N (5.05,9.08)
    "F1": (12.75, 4.7, 0, "F"),     # AC_L 12.75, AC_L_FUSED 17.83
    "RV1": (23.2, 4.4, 270, "F"),   # between the fuse and the PSU, standing up
    "PS1": (29.0, 9.5, 0, "F"),     # AC (29,9.5) L_F, (29,14.5) N
    # valve lane: U3 gate pin 4 (32.92,26.92), opto pin 6 (32.92,32)
    "U3": (43.08, 32.0, 180, "F"),
    "C2": (15.2, 14.4, 180, "F"), "R6": (22.8, 16.4, 90, "F"),     # valve snubber 100 R + 10 nF (SMD X2)
    "J3": (5.2, 25.0, 270, "F"),    # valve (5.2,25), pump (5.2,30.08)
    "Q3": (15.0, 23.5, 90, "F"),    # SOT-223 pins y=26.65, tab (MT2) y=20.35
    "R5": (15.0, 30.6, 180, "F"),   # valve gate-MT1 (2 mm below Q3's MT2 pin)
    "R4": (29.5, 30.5, 0, "F"),     # valve opto series
    # pump lane: U2 gate (32.92,38.92), opto (32.92,44.0)
    "U2": (43.08, 44.0, 180, "F"),
    "Q2": (14.0, 39.5, 0, "F"),     # TO-220 tab up: MT1 14, MT2 16.54, G 19.08
    "R2": (29.2, 44.0, 270, "F"),   # AC_L at the top, PUMP_OPTO beside U2 pin 6
    "R3": (24.0, 38.7, 270, "F"), "C1": (24.2, 48.0, 180, "F"),   # pump snubber 100 R + 10 nF (SMD X2)
    # heater: Q1 on the left edge (bolts to the chassis), J2 on the bottom edge, pours between
    "U1": (43.08, 55.6, 180, "F"),  # gate (32.92,50.52), opto (32.92,55.6)
    "Q1": (3.6, 51.0, 90, "F"),     # MT1 (3.6,51) HTR_A, MT2 (3.6,48.46) HTR_B, G (3.6,45.92)
    "J2": (10.5, 54.5, 0, "F"),
    "R1": (22.6, 57.8, 0, "F"),     # heater opto series; pad 1 sits in the HTR_B pour
    # ================= SELV =================
    # ESP32-C6-MINI-1U under the PSU, turned 180: IO18/19/20 at its top-left corner face the optos,
    # USB in the middle of the top row, analog at the top right, 3V3/EN on the right side.
    "U4": (53.5, 32.5, 180, "F"),
    "R9": (45.0, 32.0, 90, "F"), "R8": (45.0, 44.0, 90, "F"), "R7": (45.0, 52.6, 90, "F"),   # opto LED resistors
    # right of the module: sensor dividers by the analog pins, EN RC, strap pull-up, 3V3 decoupling
    "R15": (61.6, 25.4, 90, "F"), "C8": (63.4, 25.4, 90, "F"), "R16": (65.2, 25.4, 90, "F"), "R17": (67.0, 25.4, 90, "F"),
    "C9": (68.8, 25.4, 90, "F"),
    "R10": (61.6, 29.0, 90, "F"), "C5": (63.4, 29.0, 90, "F"), "R11": (65.2, 29.0, 90, "F"),
    "C4": (61.6, 32.6, 90, "F"), "C3": (63.6, 33.1, 90, "F"),
    # power: LDO and bulk caps below the module
    "U5": (56.0, 43.3, 0, "F"), "C6": (47.6, 46.6, 90, "F"), "C7": (50.0, 42.9, 90, "F"),
    "D1": (48.4, 38.6, 0, "F"), "R12": (51.6, 38.6, 180, "F"),   # status LED
    # top right: USB-C on the right edge, CC resistors, VBUS diode
    "J4": (67.8, 13.0, 90, "F"), "R13": (63.8, 2.6, 90, "F"), "R14": (63.8, 5.9, 90, "F"),
    "D2": (67.0, 21.2, 0, "F"),
    # bottom edge: switches, pressure, thermistor
    "J6": (63.4, 39.4, 0, "F"), "J5": (63.4, 45.0, 0, "F"), "J7": (63.4, 50.6, 0, "F"),   # all on the right, beside the divider pins
    "TP1": (50.5, 30.0, 0, "B"), "TP2": (53.5, 30.0, 0, "B"),    # BOOT recovery pads, underneath
    # holes (the heater triac's tab bolt is the fourth fixing)
    "H1": (4.0, 16.4, 0, "F"), "H2": (68.6, 3.4, 0, "F"), "H3": (68.6, 57.4, 0, "F"),
}


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
        for pad in fp.Pads():
            net = p["pins"].get(pad.GetNumber())
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
        for i, r in enumerate((rect(0, 0, W, 0.8), rect(0, H - 0.8, W, H), rect(W - 0.8, 0, W, H))):
            guard(board, layer, r, f"edge {i}")
    # keep copper away from the M3 hole on the mains side (earthed chassis standoff)
    zone(board, None, pcbnew.F_Cu, rect(0, 12.6, 8.2, 20.2), keepout=True, name="H1 clearance", pads_ok=True)

    # GND pours are added after routing (add_pours): exported as a plane, Freerouting assumes
    # the pour connects every GND pad, and the real fill leaves islands. The module's exposed
    # pad (9 sub-pads) gets its own small GND patch with two vias down to the B.Cu plane.
    u4 = board.FindFootprintByReference("U4")
    epad = [pcbnew.ToMM(p.GetPosition().x) for p in u4.Pads() if p.GetNumber() == "49"], \
           [pcbnew.ToMM(p.GetPosition().y) for p in u4.Pads() if p.GetNumber() == "49"]
    ex0, ex1, ey0, ey1 = min(epad[0]) - 0.5, max(epad[0]) + 0.5, min(epad[1]) - 0.5, max(epad[1]) + 0.5
    zone(board, nets["GND"], pcbnew.F_Cu, rect(ex0, ey0, ex1, ey1 + 1.6), clearance=0.2, priority=3, name="EPAD")
    for vx in (ex0 + 0.6, ex1 - 0.6):
        v = pcbnew.PCB_VIA(board)
        v.SetPosition(V(mm(vx), mm(ey1 + 0.9)))
        v.SetWidth(mm(0.6)); v.SetDrill(mm(0.3)); v.SetNet(nets["GND"]); v.SetLocked(True)
        board.Add(v)

    # silkscreen: mark the barrier and the mains side
    for txt, x, y, size in (("MAINS", 29.5, 58.8, 1.2), ("SELV", 62.6, 37.6, 1.0),
                            (f"coffee v6 {mains_v} V", 60.0, 50.0, 0.7)):
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
    # middle pin as wide as the TO-220 pin pitch allows (~2 mm, the pad width) and widens to
    # 4.5 mm once clear of the neighbouring pins.
    def pour(net, layer, pts, clearance):
        z = zone(board, nets[net], layer, pts, clearance=clearance, priority=2, name=net)
        z.SetPadConnection(pcbnew.ZONE_CONNECTION_FULL)
        z.SetMinThickness(mm(0.3))
    dy = -2.0   # v5 heater geometry, moved with the lanes
    sh = lambda pts: [(x, y + dy) for x, y in pts]
    band = sh([(2.0, 49.42), (14.4, 49.42), (20.5, 49.42), (20.5, 57.8), (23.0, 57.8), (23.0, 61.9), (14.4, 61.9), (14.4, 51.5), (2.0, 51.5)])
    band_b = sh([(5.2, 49.42), (20.5, 49.42), (20.5, 61.9), (14.4, 61.9), (14.4, 51.5), (5.2, 51.5)])
    pocket = sh([(0.6, 52.5), (13.2, 52.5), (13.2, 61.9), (0.6, 61.9)])
    pour("HTR_B", pcbnew.F_Cu, band, 0.5)
    pour("HTR_B", pcbnew.B_Cu, band_b, 0.5)
    pour("HTR_A", pcbnew.F_Cu, pocket, 0.5)
    pour("HTR_A", pcbnew.B_Cu, sh([(0.6, 52.5), (13.2, 52.5), (13.2, 61.9), (0.6, 61.9)]), 0.5)
    # Keep other nets' tracks and vias ~1 mm off the heater copper so nothing nibbles the band.
    guard(board, pcbnew.F_Cu, sh([(1.0, 48.4), (21.5, 48.4), (21.5, 56.8), (23.5, 56.8), (23.5, 62.5), (0.0, 62.5), (0.0, 52.4), (1.0, 52.4)]), "heater guard F")
    guard(board, pcbnew.B_Cu, sh([(4.2, 48.4), (21.5, 48.4), (21.5, 62.5), (0.0, 62.5), (0.0, 52.4), (4.2, 52.4)]), "heater guard B")

    # Heater gate, hand-routed on B.Cu: Q1.G -> along the top of the heater band -> U1 pin 4.
    g = nets["HEATER_GATE"]
    q1g, u1g = pad_pos(board, "Q1", "3"), pad_pos(board, "U1", "4")
    gw, yb, yc = 0.4, q1g[1], 53.0 + dy
    for a_, b_ in ((q1g, (22.0, yb)), ((22.0, yb), (22.0, yc)),
                   ((22.0, yc), (31.6, yc)), ((31.6, yc), (31.6, u1g[1])), ((31.6, u1g[1]), u1g)):
        track(board, g, pcbnew.B_Cu, a_, b_, gw)
    # Heater opto feed (HTR_B via R1 -> U1 pin 6), hand-routed along the bottom edge: the pour,
    # the pump snubber and the 2 mm mains spacing leave the router no way in (v5's last unrouted net).
    o = nets["HEATER_OPTO"]
    r1o, u1o = pad_pos(board, "R1", "2"), pad_pos(board, "U1", "6")
    yo = 58.6
    for a_, b_ in ((r1o, (r1o[0] + 0.8, yo)), ((r1o[0] + 0.8, yo), (u1o[0] - 1.9, yo)),
                   ((u1o[0] - 1.9, yo), (u1o[0], yo - 1.9)), ((u1o[0], yo - 1.9), u1o)):
        track(board, o, pcbnew.F_Cu, a_, b_, 0.8)

    # Fan-out stubs (F.Cu, 0.8 mm) out of tight mains pins; the router joins at the stub ends.
    def stub(ref, pin, end):
        a_ = pad_pos(board, ref, pin)
        net = board.FindFootprintByReference(ref).FindPadByNumber(pin).GetNetname()
        track(board, nets[net], pcbnew.F_Cu, a_, end, 0.8)

    # ---- Mains side, hand-routed (0/45/90 only). Freerouting 2.1 throws inside its maze search
    # on these nets (NullPointerException in find_connection) and leaves the same few unrouted,
    # so the mains copper is drawn here and checked by the DRC rules; the router only does SELV.
    def run(net, layer, *pts, w=0.8):
        for a_, b_ in zip(pts, pts[1:]):
            dx, dy = b_[0] - a_[0], b_[1] - a_[1]
            assert abs(dx) < 1e-6 or abs(dy) < 1e-6 or abs(abs(dx) - abs(dy)) < 0.02, f"{net}: {a_}->{b_} is not 0/45/90"
            track(board, nets[net], layer, a_, b_, w)
    def via(net, xy, d=1.2, drill=0.6):
        v = pcbnew.PCB_VIA(board)
        v.SetPosition(V(mm(xy[0]), mm(xy[1]))); v.SetWidth(mm(d)); v.SetDrill(mm(drill))
        v.SetNet(nets[net]); v.SetLocked(True); board.Add(v)
    P = lambda r, n: pad_pos(board, r, n)
    F, B = pcbnew.F_Cu, pcbnew.B_Cu
    d45 = lambda p, dx, dy: (p[0] + dx, p[1] + dy)

    # Supply. L: terminal -> fuse (B). Fused L: fuse -> MOV -> PSU (B). N: terminal -> MOV (F), MOV -> PSU (B).
    j11, f11, f12, rv1, rv2, ps1, ps2, j12 = P("J1", "1"), P("F1", "1"), P("F1", "2"), P("RV1", "1"), P("RV1", "2"), P("PS1", "1"), P("PS1", "2"), P("J1", "2")
    run("AC_L", B, j11, (f11[0] - (f11[1] - j11[1]), j11[1]), f11)
    run("AC_L_FUSED", B, f12, (rv1[0] - (f12[1] - rv1[1]), f12[1]), rv1)
    run("AC_L_FUSED", B, rv1, (ps1[0] - (ps1[1] - rv1[1]), rv1[1]), ps1)
    run("AC_N", F, j12, (rv2[0] - (rv2[1] - j12[1]), j12[1]), rv2)
    run("AC_N", B, rv2, (rv2[0] + ps2[1] - rv2[1], ps2[1]), ps2)

    # L trunk on B: fuse -> down the middle (under C2) -> via at the valve triac -> pump triac MT2.
    q22 = P("Q2", "2")
    run("AC_L", B, f11, (15.0, f11[1] + 15.0 - f11[0]), (15.0, 22.3), (15.0, 33.0), (q22[0], 33.0 + q22[0] - 15.0), q22)
    via("AC_L", (15.0, 22.3))
    tab, q32, r61 = (15.0, 20.35), P("Q3", "2"), P("R6", "1")
    run("AC_L", F, (15.0, 22.3), tab)                     # valve triac tab (MT2)
    run("AC_L", F, (15.0, 22.3), q32)                     # and its middle pin (same terminal)
    run("AC_L", F, tab, (r61[0] - (tab[1] - r61[1]), tab[1]), r61)   # valve snubber resistor

    # Pump MT2 -> R3 (pump snubber) -> R4 (valve opto feed) and R2 (pump opto feed).
    r31, r41, r21, r42, u36, r22, u26 = P("R3", "1"), P("R4", "1"), P("R2", "1"), P("R4", "2"), P("U3", "6"), P("R2", "2"), P("U2", "6")
    run("AC_L", F, q22, (q22[0], r31[1] + 1.16), (q22[0] + 1.16, r31[1]), r31)
    run("AC_L", F, r31, (r41[0], r31[1] - (r41[0] - r31[0])), r41)
    run("AC_L", F, r31, (r31[0] + 1.2, r31[1]), (r21[0], r31[1] + r21[0] - r31[0] - 1.2), r21)
    run("VALVE_OPTO", F, r42, (r42[0] + u36[1] - r42[1], u36[1]), u36)
    run("PUMP_OPTO", F, r22, (r22[0] + r22[1] - u26[1], u26[1]), u26)

    # Valve snubber: C2 (snubber side, right pad) straight across to R6.
    c21, r62 = P("C2", "1"), P("R6", "2")
    run("VALVE_SNUB", F, (c21[0], r62[1]), r62)
    # Valve load: C2 (load side, left pad) -> down the left -> J3.1 and Q3.1 -> R5.
    c22, j31, q31, r52 = P("C2", "2"), P("J3", "1"), P("Q3", "1"), P("R5", "2")
    y0 = 16.6
    run("VALVE_L", F, (c22[0] - 0.45, y0), (10.5, y0 + c22[0] - 0.45 - 10.5), (10.5, 24.5), (j31[0] + 0.5, 24.5), j31)
    run("VALVE_L", F, (10.5, 24.5), (10.5 + q31[1] - 24.5, q31[1]), q31)
    run("VALVE_L", F, q31, (q31[0], r52[1] - (r52[0] - q31[0])), r52)
    # Valve gate: Q3.3 -> y 25.35 -> U3.4; Q3.3 -> R5.1 (gate-MT1 resistor).
    q33, u34, r51 = P("Q3", "3"), P("U3", "4"), P("R5", "1")
    yg = q33[1] - 1.3
    run("VALVE_GATE", F, q33, (q33[0] + 1.3, yg), (u34[0] - (u34[1] - yg), yg), u34)
    run("VALVE_GATE", F, q33, (q33[0], r51[1] - (q33[0] - r51[0])), r51)

    # Pump gate on B: Q2.3 -> U2.4.
    q23, u24 = P("Q2", "3"), P("U2", "4")
    run("PUMP_GATE", B, q23, (q23[0] + q23[1] - u24[1], u24[1]), u24)
    # Pump load: Q2.1 -> J3.2 (45 deg up-left); Q2.1 -> y 43.6 -> C1 (load side, left pad).
    q21, j32, c1l = P("Q2", "1"), P("J3", "2"), P("C1", "2")
    run("PUMP_L", F, q21, (j32[0], q21[1] - (q21[0] - j32[0])), j32)
    run("PUMP_L", F, q21, (q21[0] - 1.0, q21[1] + 1.0), (13.0, 43.6), (c1l[0] - 1.8, 43.6), (c1l[0], 45.4), (c1l[0], 45.8))
    # Pump snubber: R3.2 -> C1 (snubber side, right pad).
    r32, c1r = P("R3", "2"), P("C1", "1")
    run("PUMP_SNUB", F, r32, (c1r[0] - (45.5 - r32[1]), r32[1]), (c1r[0], 45.5), (c1r[0], 46.0))

    # Lint: no foreign pad inside a heater guard (the router could never reach it: v5's last
    # unconnected net was exactly this).
    bad = []
    for z in board.Zones():
        if not z.GetZoneName().startswith("heater guard"):
            continue
        layer = z.GetLayerSet().Seq()[0]
        for fp in board.GetFootprints():
            for pad in fp.Pads():
                if pad.IsOnLayer(layer) and pad.GetNetname() not in ("HTR_A", "HTR_B", "") \
                        and z.Outline().Collide(pad.GetPosition()):
                    bad.append(f"{fp.GetReference()}.{pad.GetNumber()} [{pad.GetNetname()}] in {z.GetZoneName()}")
    if bad:
        raise SystemExit("pads inside heater guards:\n  " + "\n  ".join(bad))
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
MAINS_PARTS = ["Q1", "Q2", "Q3", "U1", "U2", "U3"]   # parts whose own pin pitch governs inside their courtyard


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
            blocks.append(f"    (class_class (classes M{i} M{j})\n      (rule (clearance {um(2.0)}))\n    )\n")
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


JOB_TIMEOUT = [120]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mains", type=int, default=120)
    ap.add_argument("--passes", type=int, default=40)
    ap.add_argument("--no-route", action="store_true")
    ap.add_argument("--attempts", type=int, default=3, help="batches")
    ap.add_argument("--parallel", type=int, default=4, help="attempts per batch, run side by side")
    ap.add_argument("--snap", choices=["90", "45"], default="45")
    ap.add_argument("--timeout", type=int, default=360, help="seconds per routing attempt (fail fast)")
    ap.add_argument("--no-stage2", action="store_true", help="skip the sequential finishing pass (quick checks)")
    a = ap.parse_args()
    JOB_TIMEOUT[0] = a.timeout
    spec = json.load(open(os.path.join(BUILD, f"parts-{a.mains}v.json")))
    stem = os.path.join(BUILD, f"coffee-controller-{a.mains}v")
    board = build(spec, a.mains)
    pcbnew.SaveBoard(stem + "-placed.kicad_pcb", board)
    write_rules(stem + "-placed.kicad_dru", spec)
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
    fix_dsn(dsn, spec)
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

    # Freerouting is single-threaded for most of its work and not deterministic, so run a
    # batch of independent attempts side by side and keep the best (first complete) one.
    def launch(dsn_path, ses_path, passes):
        if os.path.exists(ses_path):
            os.remove(ses_path)
        return subprocess.Popen(["java", "-Xmx3g", "-Djava.awt.headless=true", "-jar", FREEROUTING_JAR, "--gui.enabled=false",
                                 "-de", dsn_path, "-do", ses_path, "-mt", "2",
                                 # Freerouting 2.1 ignores -mp; its job timeout does stop it AND writes the session
                                 f"--router.job_timeout={JOB_TIMEOUT[0] // 3600:02d}:{JOB_TIMEOUT[0] // 60 % 60:02d}:{JOB_TIMEOUT[0] % 60:02d}"],
                                stdout=open(ses_path + ".log", "w"), stderr=subprocess.STDOUT)   # live progress per attempt

    def evaluate(ses_path):
        board = pcbnew.LoadBoard(stem + "-placed.kicad_pcb")
        n_t, n_v = import_ses(board, ses_path)
        stubs = remove_stubs(board)
        return board, n_t, n_v, stubs, finish(board)

    def violations(board):
        """Clearance/short violations against the mains rules (the router can still leave some)."""
        tmp = stem + "-check.kicad_pcb"
        pcbnew.SaveBoard(tmp, board)
        write_rules(stem + "-check.kicad_dru", spec)
        b = pcbnew.LoadBoard(tmp)
        pcbnew.WriteDRCReport(b, stem + "-check.rpt", pcbnew.EDA_UNITS_MILLIMETRES, False)
        return sum(1 for l in open(stem + "-check.rpt") if l.startswith(("[clearance", "[shorting", "[items_not_allowed")))

    best = None
    for batch in range(1, a.attempts + 1):
        procs = {i: launch(dsn, f"{stem}-a{i}.ses", a.passes) for i in range(a.parallel)}
        deadline = time.time() + a.timeout + 30   # routers stop themselves at a.timeout; 30 s to write out
        for i, pr in procs.items():
            try:
                pr.wait(timeout=max(0.1, deadline - time.time()))
            except subprocess.TimeoutExpired:
                pr.kill()
        for i in procs:
            ses_i = f"{stem}-a{i}.ses"
            if not os.path.exists(ses_i):
                continue
            board, n_t, n_v, stubs, unrouted = evaluate(ses_i)
            stage = "1"
            if unrouted and not a.no_stage2:   # stage 2: finish the leftovers on top of this attempt's copper
                dsn2, ses2 = stem + f"-a{i}-s2.dsn", stem + f"-a{i}-s2.ses"
                pcbnew.ExportSpecctraDSN(board, dsn2)
                fix_dsn(dsn2, spec)
                pr = launch(dsn2, ses2, 20)
                try:
                    pr.wait(timeout=a.timeout)
                except subprocess.TimeoutExpired:
                    pr.kill()
                if os.path.exists(ses2):
                    b2, t2, v2, s2, u2 = evaluate(ses2)
                    if u2 < unrouted:
                        board, n_t, n_v, stubs, unrouted, stage = b2, t2, v2, s2, u2, "2"
            bad = violations(board)
            print(f"batch {batch} attempt {i}: {n_t} segments, {n_v} vias, unconnected {unrouted}, violations {bad} (stage {stage})", flush=True)
            if best is None or (unrouted, bad, n_v) < (best[0], best[5], best[3]):   # fewest vias breaks ties
                pcbnew.SaveBoard(stem + "-best.kicad_pcb", board)   # finish from disk: pcbnew aborts filling a board juggled in memory
                best = (unrouted, None, n_t, n_v, stubs, bad)
        if best and best[0] == 0:
            break
    if best is not None:
        b_ = pcbnew.LoadBoard(stem + "-best.kicad_pcb")
        add_pours(b_)
        b_.BuildConnectivity()
        pcbnew.ZONE_FILLER(b_).Fill(b_.Zones())
        best = (finish(b_), b_) + best[2:]
    if best is None:
        raise SystemExit("routing failed")
    unrouted, board, n_t, n_v, stubs, _ = best
    pcb = stem + ".kicad_pcb"
    pcbnew.SaveBoard(pcb, board)
    write_rules(stem + ".kicad_dru", spec)
    board = pcbnew.LoadBoard(pcb)          # reload so the rules apply to fill + DRC
    board.BuildConnectivity()
    pcbnew.ZONE_FILLER(board).Fill(board.Zones())
    pcbnew.SaveBoard(pcb, board)
    rpt = stem + "-drc.rpt"
    pcbnew.WriteDRCReport(board, rpt, pcbnew.EDA_UNITS_MILLIMETRES, True)
    fab_outputs(pcb, stem)
    print(f"routed: {n_t} track segments ({stubs} stubs removed), {n_v} vias, unconnected: {unrouted}\nwrote {pcb}\nDRC: {rpt}")

if __name__ == "__main__":
    main()
