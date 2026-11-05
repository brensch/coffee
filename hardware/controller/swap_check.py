#!/usr/bin/env python3
"""Mis-plug check: every lead in every same-size socket must be harmless.

JST GH has one key, so a lead fits every socket of its size. For every lead in every socket of its
size, this script builds the circuit that results:
  - the board's drive on each socket pin, classified from its net in build/parts-120v.json;
  - the device at the lead's far end, as wired in docs/WIRING.md.
It then checks every combination of:
  - rocker positions;
  - firmware output states (each output on/off, each lamp drive high/low, SCK high/low);
  - board power: powered (USB, or the supply wherever it is) and unpowered.
It also follows paths through floating nodes: a lead's shared common, or the SSRs' daisy-chained +5V.
Each lead-in-socket case is independent of where the other leads are. The firmware states are
enumerated, not assumed, so a multi-lead swap is covered by its single-lead cases.

Exit 1 on any hazard. See docs/SWAP-SAFETY.md.

    python3 swap_check.py            # check the board's netlist
    python3 swap_check.py -v         # also list what every case does
"""
import itertools, json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))

# --- board side -------------------------------------------------------------------------------------------
# class -> (levels it can take, strength, vulnerable)
#   strength: 3 strong (rail, MOSFET drain, push-pull GPIO), 2 medium (GPIO behind 330 R), 1 weak (pull-up
#   >= 4.7k, ADC divider), 0 none (empty pin).
#   vulnerable: a direct GPIO / ADC pin that must never see more than 3.3 V from a >= medium source.
NETCLASS = {"GND": "GND", "+5V": "S5", "+5V_IN": "S5IN", "+3V3": "S3",
            "HEATER_OUT": "DR", "PUMP_OUT": "DR", "VALVE_OUT": "DR",
            "LED_PWR": "LEDDRV", "LED_COFFEE": "LEDDRV", "LED_STEAM": "LEDDRV",
            "SW_POWER_IN": "SWIN", "SW_BREW_IN": "SWIN", "SW_STEAM_IN": "SWIN",
            "SCALE_SCK": "SCK", "SCALE_DOUT1": "DIN", "SCALE_DOUT2": "DIN",
            "P_SIG": "PSIG", "NTC": "NTCN", None: "E"}
STRONG, MED, WEAK, NONE = 3, 2, 1, 0

def board_drive(cls, st, i):
    """(level, strength, vulnerable) of a board pin in firmware/power state st, or None if undriven."""
    pw = st["powered"]
    if cls == "GND": return (0.0, STRONG, False)
    if cls in ("S5", "S5IN"): return (5.0, STRONG, False) if pw else None   # S5IN: Q4 conducts both ways
    if cls == "S3": return (3.3, STRONG, False) if pw else None
    if cls == "DR": return (0.0, STRONG, False) if st["out"][i] else None
    if cls == "LEDDRV": return ((3.3 if st["lamp"][i] else 0.0), MED, False) if pw else None
    if cls == "SWIN": return (3.3, WEAK, True) if pw else None
    if cls == "SCK": return ((3.3 if st["sck"] or st["standby"] else 0.0), STRONG, True) if pw else None   # standby: held high
    if cls == "DIN": return (3.3, WEAK, True) if pw else None
    if cls == "PSIG": return (0.0, WEAK, False)         # 47k + 100k to GND: tolerates 5 V
    if cls == "NTCN": return (3.3, WEAK, True) if pw else None   # 4.7k to 3V3, direct to the ADC pin
    return None

# --- lead side --------------------------------------------------------------------------------------------
# Each lead: its home socket and the device's elements between named roles. Roles are mapped to lead pin
# numbers through the home socket's pinout (a lead is wired to match its own socket).
#   SW a b       a rocker contact (enumerated open/closed)
#   LEDR a b     front-panel LED with its own series resistor at the LED (anode a)
#   HSSR a b     heater SSR DC input (+ a, - b): on from 3 V at a few mA; blocks reverse
#   BRD a b      pump/valve SSR board, VCC+IN tied (a) to GND (b): on from 2.5 V; reverse unknown (worst
#                case: conducts, and >= 3 V reverse is itself a hazard)
#   CHIP sup gnd ios outs   HX711 / transducer: protection diodes from every pin to its rail and ground
#   RES a b      NTC (>= 1k)
#   SRC g v      the 5 V module output: v = g + 5, floating until one side is tied
ROLE_NET = {   # role in a lead -> the net of its home socket that role sits on
    "J2": {"X": "+5V", "h": "HEATER_OUT", "p": "PUMP_OUT", "v": "VALVE_OUT"},
    "J7": {"swP": "SW_POWER_IN", "swB": "SW_BREW_IN", "swS": "SW_STEAM_IN", "K": "GND",
           "L1": "LED_PWR", "L2": "LED_COFFEE", "L3": "LED_STEAM"},
    "J10": {"G": "GND", "V": "+3V3", "SCK": "SCALE_SCK", "D1": "SCALE_DOUT1", "D2": "SCALE_DOUT2"},   # v30: VCC+VDD on 3V3
    "J1": {"g": "GND", "v": "+5V_IN"},
    "J6": {"S": "P_SIG", "V": "+5V", "G": "GND"},
    "J5": {"n": "NTC", "G": "GND"},
}
LEADS = {
    "J2": [("HSSR", "X", "h"), ("BRD", "X", "p"), ("BRD", "X", "v")],
    "J7": [("SW", "swP", "K"), ("SW", "swB", "K"), ("SW", "swS", "K"),
           ("LEDR", "L1", "K"), ("LEDR", "L2", "K"), ("LEDR", "L3", "K")],
    "J10": [("CHIP", ["V"], "G", ["SCK", "D1", "D2"], ["D1", "D2"])],
    "J1": [("SRC", "g", "v")],
    "J6": [("CHIP", ["V"], "G", ["S"], ["S"])],
    "J5": [("RES", "n", "G")],
}
GROUPS = [["J1", "J5", "J6"], ["J2", "J7", "J10"]]
LEAD_NAME = {"J1": "5V supply", "J2": "SSR", "J5": "thermistor", "J6": "pressure", "J7": "front panel", "J10": "scale"}
SWIN_LOW = 2.5
NTC_SHORT_V = 0.1   # firmware fault::NTC_SHORT threshold (short_mv, 100 mV default)
BARE_LEDS = "--bare-leds" in sys.argv   # what-if: front LEDs fitted without their own resistor    # ESP32-C6 V_IH min = 0.75 * 3.3 V: a switch input pulled below this may read "on"

def role_pins(home_pins, home):
    """role -> list of lead pin indices (a role can sit on several pins, e.g. two GND wires)."""
    net2role = {n: r for r, n in ROLE_NET[home].items()}
    out = {}
    for i, n in enumerate(home_pins):
        if n in net2role: out.setdefault(net2role[n], []).append(i)
    return out

# --- the analysis -----------------------------------------------------------------------------------------
class UF:
    def __init__(s): s.p = {}
    def f(s, x):
        s.p.setdefault(x, x)
        while s.p[x] != x: s.p[x] = s.p[s.p[x]]; x = s.p[x]
        return x
    def u(s, a, b): s.p[s.f(a)] = s.f(b)

def analyse(home, home_pins, sock, sock_pins):
    """All hazards of the `home` lead plugged into socket `sock`. Returns a sorted list of strings."""
    rp = role_pins(home_pins, home)
    classes = [NETCLASS.get(n, "E") for n in sock_pins]
    nets = list(sock_pins)
    els = LEADS[home]
    n_sw = sum(1 for e in els if e[0] == "SW")
    has = lambda c: c in classes
    hz = set()
    # The front lead out of its own socket means the power rocker isn't connected: the firmware stays in
    # standby (heater, pump, valve, lamps off, SCK held low), provided no lead in J7 can pull SW_POWER low
    # (checked below). Only the rocker wakes the engine.
    standby = (home == "J7") != (sock == "J7")
    states = []
    for pw in (True, False):
        n7 = len(classes)
        dr = [i for i, c in enumerate(classes) if c == "DR"]; ld = [i for i, c in enumerate(classes) if c == "LEDDRV"]
        for o in itertools.product((False, True), repeat=0 if standby else len(dr)):
            out = [False] * n7
            for i, v in zip(dr, o): out[i] = v
            for l in itertools.product((False, True), repeat=len(ld) if pw and not standby else 0):
                lamp = [False] * n7
                for i, v in zip(ld, l): lamp[i] = v
                for sck in ((False, True) if has("SCK") and pw and not standby else (False,)):
                    for sw in itertools.product((False, True), repeat=n_sw):
                        states.append(dict(powered=pw, out=out, lamp=lamp, sck=sck, sw=sw, standby=standby))
    for st in states:
        for h in one_state(home, rp, classes, nets, els, st, sock):
            hz.add(h)
    return sorted(hz)

def one_state(home, rp, classes, nets, els, st, sock):
    uf = UF()
    pins = [f"p{i}" for i in range(len(classes))]
    for p in pins: uf.f(p)
    # a role's pins are one wire in the lead
    for r, idx in rp.items():
        for i in idx[1:]: uf.u(f"p{idx[0]}", f"p{i}")
    node = lambda role: uf.f(f"p{rp[role][0]}") if role in rp else None
    k = 0
    for e in els:
        if e[0] == "SW":
            if st["sw"][k]: uf.u(node(e[1]), node(e[2]))
            k += 1
    node = lambda role: uf.f(f"p{rp[role][0]}") if role in rp else None
    # drives per merged node
    drv = {}
    for i, c in enumerate(classes):
        d = board_drive(c, st, i)
        n = uf.f(f"p{i}")
        drv.setdefault(n, []).append((c, nets[i], d, i))
    hz = []
    lvl = {}
    for n, ds in drv.items():
        act = [(c, net, d) for c, net, d, _ in ds if d]
        strong = [x for x in act if x[2][1] == STRONG]
        if len({x[2][0] for x in strong}) > 1:
            hz.append("short: " + " to ".join(sorted({x[1] for x in strong})))
        if any(x[0] == "SCK" for x in strong) and len(strong) > 1 and not st["standby"]:   # it toggles
            hz.append("SCK (push-pull GPIO) tied to " + ",".join(x[1] for x in strong if x[0] != "SCK"))
        if act:
            best = max(act, key=lambda x: x[2][1])
            lvl[n] = (best[2][0], best[2][1])
    # the supply lead: a floating 5 V source
    for e in els:
        if e[0] == "SRC":
            g, v = node(e[1]), node(e[2])
            if g in lvl and v in lvl:
                if lvl[v][0] - lvl[g][0] != 5.0 and lvl[g][1] >= MED and lvl[v][1] >= MED:
                    hz.append(f"5 V supply fights the board ({lvl[g][0]} / {lvl[v][0]} V)")
            elif g in lvl and lvl[g][1] >= MED: lvl[v] = (lvl[g][0] + 5.0, STRONG)
            elif v in lvl and lvl[v][1] >= MED: lvl[g] = (lvl[v][0] - 5.0, STRONG)
            # touching the board only on one side, or only weakly: a floating source drives nothing
    # 5 V / out-of-range levels reaching vulnerable pins or anything at all
    for n, ds in drv.items():
        if n not in lvl: continue
        L, S = lvl[n]
        if S < MED: continue
        for c, net, d, i in ds:
            if c == "E": continue
            if L > 3.6 and c in ("SCK", "DIN", "NTCN", "SWIN"):
                hz.append(f"{L:g} V onto {net} (GPIO)")
            if L < -0.3 or L > 5.3:
                hz.append(f"{L:g} V onto {net}")
            if c == "LEDDRV" and S == STRONG and abs(L - (3.3 if st['lamp'][i] else 0.0)) > 3.3 + 1e-9:
                hz.append(f"{L:g} V into a lamp drive")
    # conduction through two-terminal elements, following floating nodes
    cond = []   # (from, to, drop, strength) forward conduction
    for e in els:
        a, b = node(e[1]) if isinstance(e[1], str) else None, node(e[2]) if isinstance(e[2], str) else None
        if e[0] == "LEDR": cond.append((a, b, 1.8, STRONG if BARE_LEDS else MED, e))
        if e[0] == "HSSR": cond += [(a, b, 1.2, MED, e), (b, a, 0.7, MED, e)]   # worst case: a reverse-protection diode across the input
        if e[0] == "BRD": cond += [(a, b, 1.0, MED, e), (b, a, 0.7, MED, e)]
        if e[0] == "RES": cond += [(a, b, 0.0, WEAK, e), (b, a, 0.0, WEAK, e)]
        if e[0] == "CHIP":   # any pin -> (diode to rail, chip load) -> ground pin, and ground -> any pin (diode)
            g = node(e[2])
            for r in e[1] + e[3]:
                n = node(r)
                if n is not None and g is not None and n != g: cond += [(n, g, 1.5, MED, e), (g, n, 0.6, MED, e)]   # pin->rail->load->ground drops more than a diode
    def bounds(skip):
        """vmax/vmin reachable at each node, with the strength of the path, ignoring element `skip`."""
        hi = {n: v for n, v in lvl.items()}
        lo = {n: v for n, v in lvl.items()}
        fixed = {n for n, v in lvl.items() if v[1] >= MED}    # weakly held nodes can be pulled
        for _ in range(6):
            for f, t, drop, s, e in cond:
                if e is skip or f is None or t is None: continue
                if f in hi and t not in fixed:
                    c = (hi[f][0] - drop, min(hi[f][1], s))
                    if t not in hi or c[0] > hi[t][0] or (c[0] == hi[t][0] and c[1] > hi[t][1]): hi[t] = c
                if t in lo and f not in fixed:
                    c = (lo[t][0] + drop, min(lo[t][1], s))
                    if f not in lo or c[0] < lo[f][0] or (c[0] == lo[f][0] and c[1] > lo[f][1]): lo[f] = c
        return hi, lo
    for e in els:
        if e[0] == "LEDR" and BARE_LEDS:   # a bare LED straight across two strong nodes burns
            hi, lo = bounds(None)
            a, b = node(e[1]), node(e[2])
            if a in lvl and b in lvl and lvl[a][1] == STRONG and lvl[b][1] == STRONG and lvl[a][0] - lvl[b][0] >= 1.8:
                hz.append(f"bare LED across {lvl[a][0]:g} V to {lvl[b][0]:g} V")
        if e[0] in ("HSSR", "BRD"):
            hi, lo = bounds(e)
            a, b = node(e[1]), node(e[2])
            if a in hi and b in lo and hi[a][1] >= MED and lo[b][1] >= MED:
                dv = hi[a][0] - lo[b][0]
                on = 1.0   # must-turn-on is 3 V (heater) / ~2.5 V (boards), but only below ~1 V is "off" guaranteed: treat the gap as on
                if dv >= on - 1e-9 and sock != home:
                    hz.append(("heater SSR" if e[0] == "HSSR" else "pump/valve SSR board") + f" on ({dv:.1f} V)")
            if e[0] == "BRD" and b in hi and a in lo and hi[b][1] >= MED and lo[a][1] >= MED and hi[b][0] - lo[a][0] >= 3.0:
                hz.append(f"SSR board reverse-biased ({hi[b][0] - lo[a][0]:.1f} V)")
        if e[0] == "CHIP":
            hi, lo = bounds(None)
            sup, gnd, ios, outs = e[1], e[2], e[3], e[4]
            name = "HX711" if home == "J10" else "transducer"
            P_ = {r: node(r) for r in sup + [gnd] + ios if node(r) is not None}
            act = {r: lvl[n] for r, n in P_.items() if n in lvl and lvl[n][1] >= MED}
            if not act: continue
            hi_r = max(act, key=lambda r: act[r][0]); lo_r = min(act, key=lambda r: act[r][0])
            if act[hi_r][0] - act[lo_r][0] < 1.0: continue        # nothing flows
            g = act.get(gnd)
            rail = max([act[r][0] for r in sup if r in act], default=None)
            if g is None or g[0] > act[lo_r][0] + 0.3:
                hz.append(f"{name} grounded through a protection diode ({lo_r} on {act[lo_r][0]:g} V)")
            if rail is None or act[hi_r][0] > rail + 0.3:
                hz.append(f"{name} powered through a protection diode ({hi_r} at {act[hi_r][0]:g} V)")
            if g is not None and rail is not None and rail - g[0] >= 2.5:
                for r in outs:      # powered: its outputs must not face a strong drive
                    if r in P_ and P_[r] in lvl and lvl[P_[r]][1] == STRONG and sock != home:
                        hz.append(f"{name} output {r} driven against {lvl[P_[r]][0]:g} V")
    # functional: a foreign lead in the front socket must not pull a switch input low
    if sock == "J7" and home != "J7" and st["powered"]:
        hi, lo = bounds(None)
        for i, c in enumerate(classes):
            if nets[i] != "SW_POWER_IN": continue   # in standby only the power rocker is read
            n = uf.f(f"p{i}")
            if n in lo and lo[n][0] < SWIN_LOW and lo[n][1] >= MED:
                hz.append(f"{nets[i]} can read 'on' (pulled to {lo[n][0]:.1f} V)")
    # functional: the thermistor input must read open (nothing there) or shorted (pulled hard low): both are
    # faults that keep the heater off. A device that could hold it mid-range might read as a temperature.
    if sock == "J5" and home != "J5":
        hi, lo = bounds(None)
        for i, c in enumerate(classes):
            if c != "NTCN": continue
            n = uf.f(f"p{i}")
            touched = any(f == n or t == n for f, t, *_ in cond)
            shorted = (n in lvl and lvl[n][1] >= MED and lvl[n][0] <= NTC_SHORT_V) or (n in lo and lo[n][1] >= MED and lo[n][0] <= NTC_SHORT_V)
            # the pressure lead here means J6 has no transducer: PRESSURE_SENSOR faults and the heater
            # stays off whatever the thermistor input reads
            if touched and not shorted and home != "J6":
                hz.append("thermistor input sees a device (may read a plausible temperature)")
    return hz

def load_board():
    P = json.load(open(os.path.join(HERE, "build", "parts-120v.json")))["parts"]
    return {r: [P[r]["pins"][str(i)] for i in range(1, len(P[r]["pins"]) + 1)] for g in GROUPS for r in g}

def check(pinouts, verbose=False):
    bad = 0
    for grp in GROUPS:
        for home in grp:
            for sock in grp:
                h = analyse(home, pinouts[home], sock, pinouts[sock])
                tag = "home" if sock == home else "swap"
                if h or verbose:
                    print(f"{LEAD_NAME[home]:12s} lead in {sock:3s} ({tag}): " + ("; ".join(h) if h else "harmless"))
                bad += bool(h)
    return bad

if __name__ == "__main__":
    bad = check(load_board(), "-v" in sys.argv)
    print("ALL HARMLESS" if not bad else f"{bad} HAZARDOUS lead/socket combinations")
    sys.exit(1 if bad else 0)
