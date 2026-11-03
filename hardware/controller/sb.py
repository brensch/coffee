#!/usr/bin/env python3
"""Layout sandbox scorer: try a floorplan in milliseconds, no KiCad.

    python3 sb.py plan.json [out.png]

plan.json: {"W": .., "H": .., "mains": [[x,y],..], "selv": [[x,y],..],   # zone polygons; the gap is the barrier
            "place": {"REF": [x, y, rot, "F"|"B"], ..}}                    # missing refs keep parts.json's place
Geometry comes from out/view/sandbox/parts.json (sandbox_export.py). Checks: same-side courtyard overlap
(THT pins count on both sides), pads inside their zone, zone gap >= 7 mm, wire-entry parts touching an edge.
Prints area, problems and rat's-nest length; writes an SVG (and a PNG via headless Chrome if out.png given).
"""
import json, math, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
G = json.load(open(os.path.join(HERE, "..", "..", "out", "view", "sandbox", "parts.json")))
P = G["parts"]
EDGE = {"J1", "J2", "J3", "J4", "J5", "J6", "J7", "Q1"}   # wires plug in / heater triac bolts to the case
GAP = 7.0
MAINS_CLR = 2.0          # between pads of different mains nets (same as the DRC rule)
SAME_GROUPS = [("AC_L", "PUMP_OPTO", "VALVE_OPTO", "PUMP_SNUB", "VALVE_SNUB"), ("PUMP_L", "PUMP_GATE"),
               ("VALVE_L", "VALVE_GATE"), ("HTR_A", "HEATER_GATE"), ("HTR_B", "HEATER_OPTO")]
_grp = {n: i for i, g in enumerate(SAME_GROUPS) for n in g}


def mains_conflict(n1, n2):
    """True if two mains nets need the full 2 mm."""
    return n1 != n2 and not (_grp.get(n1) is not None and _grp.get(n1) == _grp.get(n2))


CROSS = 25.0     # a crossing of two mains connections that need 2 mm: costs a via or a detour


def segx(a, b, c, d):
    """proper intersection of segments ab and cd (shared endpoints don't count)."""
    def o(p, q, r):
        v = (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
        return 0 if abs(v) < 1e-9 else (1 if v > 0 else -1)
    if min(math.dist(a, c), math.dist(a, d), math.dist(b, c), math.dist(b, d)) < 1e-6:
        return False
    return o(a, b, c) * o(a, b, d) < 0 and o(c, d, a) * o(c, d, b) < 0


def rectgap(a, b):
    dx = max(a[0] - b[2], b[0] - a[2], 0)
    dy = max(a[1] - b[3], b[1] - a[3], 0)
    return math.hypot(dx, dy)
# how much a millimetre of each net costs: current, loop area, noise. GND is a pour (free).
WEIGHT = {"HTR_A": 10, "HTR_B": 10, "PUMP_L": 3, "AC_L": 2, "AC_L_FUSED": 2, "AC_N": 2, "VALVE_L": 2,
          "HEATER_GATE": 4, "PUMP_GATE": 4, "VALVE_GATE": 4, "PUMP_SNUB": 6, "VALVE_SNUB": 6,
          "PUMP_OPTO": 4, "VALVE_OPTO": 4, "HEATER_OPTO": 4,
          "+3V3": 3, "+5V": 2, "VBUS": 2, "USB_D+": 2, "USB_D-": 2, "EN": 1.5, "P_ADC": 1.5, "NTC": 1.5}


def xf(pl, x, y):
    px, py, r, side = pl
    if side == "B":
        y = -y            # pcbnew Flip(pos, False) mirrors top-bottom, then the rotation applies
    t = math.radians(r)
    c, s = round(math.cos(t), 9), round(math.sin(t), 9)
    return px + x * c + y * s, py - x * s + y * c


def box(pl, b):
    pts = [xf(pl, x, y) for x, y in ((b[0], b[1]), (b[2], b[1]), (b[2], b[3]), (b[0], b[3]))]
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    return [min(xs), min(ys), max(xs), max(ys)]


def inside(poly, p):
    c = False
    for i in range(len(poly)):
        (x1, y1), (x2, y2) = poly[i], poly[(i + 1) % len(poly)]
        if (y1 > p[1]) != (y2 > p[1]) and p[0] < (x2 - x1) * (p[1] - y1) / (y2 - y1) + x1:
            c = not c
    return c


def segdist(p, a, b):
    ax, ay, bx, by = *a, *b
    dx, dy = bx - ax, by - ay
    t = 0 if dx == dy == 0 else max(0, min(1, ((p[0] - ax) * dx + (p[1] - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(p[0] - ax - t * dx, p[1] - ay - t * dy)


def polydist(A, B):
    d = 1e9
    for poly, other in ((A, B), (B, A)):
        for p in poly:
            for i in range(len(other)):
                d = min(d, segdist(p, other[i], other[(i + 1) % len(other)]))
    return d


def ovl(A, B, m=0.0):
    return A[0] < B[2] - m and B[0] < A[2] - m and A[1] < B[3] - m and B[1] < A[3] - m


def score(plan):
    W, H = plan["W"], plan["H"]
    place = {**G["place"], **plan.get("place", {})}
    zones = {"mains": plan["mains"], "selv": plan["selv"]}
    swap = set(plan.get("swap", []))
    probs, bx, pads, nets = [], {}, {}, {}
    for r, pl in place.items():
        bx[r] = box(pl, P[r]["box"])
        pads[r] = []
        for q in P[r]["pads"]:
            if r in swap and q["n"] in ("1", "2"):
                o = next(p for p in P[r]["pads"] if p["n"] == ("2" if q["n"] == "1" else "1"))
                q = dict(q, net=o["net"], mains=o["mains"])
            x, y = xf(pl, q["x"], q["y"])
            w, h = (q["h"], q["w"]) if pl[2] % 180 else (q["w"], q["h"])
            pads[r].append((x - w / 2, y - h / 2, x + w / 2, y + h / 2, q))
            if not q["tht"] and not (0.55 < x - w / 2 and x + w / 2 < W - 0.55 and 0.55 < y - h / 2 and y + h / 2 < H - 0.55):
                probs.append(f"{r}.{q['n']} within 0.55 mm of the edge")
            if not (0.3 < x - w / 2 and x + w / 2 < W - 0.3 and 0.3 < y - h / 2 and y + h / 2 < H - 0.3):
                probs.append(f"{r}.{q['n']} off board")
            if q["net"]:
                z = zones["mains" if q["mains"] else "selv"]
                if not all(inside(z, c) for c in ((x - w / 2, y - h / 2), (x + w / 2, y - h / 2), (x - w / 2, y + h / 2), (x + w / 2, y + h / 2))):
                    probs.append(f"{r}.{q['n']} ({q['net']}) outside {'mains' if q['mains'] else 'LV'} zone")
                nets.setdefault(q["net"], [q["mains"], []])[1].append((x, y))
        b = bx[r]
        m = 2.0 if r in EDGE else 0.01      # connectors may overhang the edge
        if b[0] < -m or b[1] < -m or b[2] > W + m or b[3] > H + m:
            probs.append(f"{r} courtyard off board")
        if r in EDGE and min(b[0], b[1], W - b[2], H - b[3]) > 1.0:
            probs.append(f"{r} not at an edge")
    tht = {r: any(q["tht"] for q in P[r]["pads"]) for r in place}
    refs = list(place)
    for i, a in enumerate(refs):
        for b in refs[i + 1:]:
            if place[a][3] == place[b][3]:
                if ovl(bx[a], bx[b], 0.01):
                    probs.append(f"overlap {a}/{b}")
            else:   # other side: only through-hole pins collide (with 0.5 mm clearance)
                for t, o in ((a, b), (b, a)):
                    if tht[t] and any(ovl([p[0] - .5, p[1] - .5, p[2] + .5, p[3] + .5], bx[o]) for p in pads[t] if p[4]["tht"]):
                        probs.append(f"{t} pins hit {o} on the other side")
    mp = [(r, p) for r in place for p in pads[r] if p[4]["net"] and p[4]["mains"]]
    for i, (ra, a) in enumerate(mp):
        for rb, b in mp[i + 1:]:
            if ra == rb or not mains_conflict(a[4]["net"], b[4]["net"]):
                continue
            sa, sb_ = place[ra][3], place[rb][3]
            if sa != sb_ and not (a[4]["tht"] or b[4]["tht"]):
                continue
            g = rectgap(a, b)
            if g < MAINS_CLR - 0.01:
                probs.append(f"mains {ra}.{a[4]['n']}/{rb}.{b[4]['n']} {g:.2f} mm")
    zd = polydist(zones["mains"], zones["selv"])
    if zd < GAP - 0.01:
        probs.append(f"zone gap {zd:.2f} < {GAP}")
    rats, total, rats_n = [], 0.0, []
    for n, (m, pts) in nets.items():
        if n == "GND" or len(pts) < 2:
            continue
        used, left = [0], list(range(1, len(pts)))
        while left:
            d, u, v = min((math.dist(pts[u], pts[v]), u, v) for u in used for v in left)
            total += d * WEIGHT.get(n, 1)
            rats.append((pts[u], pts[v], m))
            rats_n.append((pts[u], pts[v], m, n))
            used.append(v)
            left.remove(v)
    edges = [(a, b, n) for a, b, m, n in rats_n if m]
    cross = 0
    for i, (a, b, n1) in enumerate(edges):
        for c, d, n2 in edges[i + 1:]:
            if mains_conflict(n1, n2) and segx(a, b, c, d):
                cross += 1
    total += CROSS * cross
    return dict(cross=cross, W=W, H=H, place=place, zones=zones, bx=bx, pads=pads, probs=probs, rats=rats, total=total, zd=zd)


def svg(s, title=""):
    W, H, pad, S = s["W"], s["H"], 4, 12
    o = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{(W + 2 * pad) * S:.0f}" height="{(H + 2 * pad + 4) * S:.0f}" '
         f'viewBox="{-pad} {-pad - 4} {W + 2 * pad} {H + 2 * pad + 4}" font-family="sans-serif">',
         f'<rect x="{-pad}" y="{-pad - 4}" width="{W + 2 * pad}" height="{H + 2 * pad + 4}" fill="#fff"/>',
         f'<text x="0" y="-4.5" font-size="2">{title}  {W}x{H}={W * H:.0f}mm²  rats {s["total"]:.0f}  problems {len(s["probs"])}</text>',
         f'<rect x="0" y="0" width="{W}" height="{H}" fill="#e9eee9" stroke="#333" stroke-width=".25"/>']
    for x in range(0, int(W) + 1, 5):
        o.append(f'<text x="{x}" y="-0.7" font-size="1.3" fill="#999" text-anchor="middle">{x}</text>')
    for y in range(0, int(H) + 1, 5):
        o.append(f'<text x="-0.8" y="{y + .4}" font-size="1.3" fill="#999" text-anchor="end">{y}</text>')
    for k, col in (("mains", "#f6d5cf"), ("selv", "#d3e1f5")):
        o.append(f'<polygon points="{" ".join(f"{x},{y}" for x, y in s["zones"][k])}" fill="{col}" stroke="none"/>')
    order = sorted(s["place"], key=lambda r: s["place"][r][3] == "F")      # bottom first, top drawn over
    for r in order:
        b, bot = s["bx"][r], s["place"][r][3] == "B"
        st = 'stroke="#2a8a3a" stroke-dasharray=".6 .4" fill="rgba(210,240,215,.5)"' if bot else 'stroke="#7a3aa0" fill="rgba(255,255,255,.6)"'
        o.append(f'<rect x="{b[0]:.2f}" y="{b[1]:.2f}" width="{b[2] - b[0]:.2f}" height="{b[3] - b[1]:.2f}" stroke-width=".15" {st}/>')
        for x0, y0, x1, y1, q in s["pads"][r]:
            col = "#bbb" if not q["net"] else ("#d9473a" if q["mains"] else "#3a72c6")
            rx = min(x1 - x0, y1 - y0) / 2 if q["tht"] else .1
            o.append(f'<rect x="{x0:.2f}" y="{y0:.2f}" width="{x1 - x0:.2f}" height="{y1 - y0:.2f}" rx="{rx:.2f}" fill="{col}" opacity="{.5 if bot else 1}"/>')
        fs = 1.6 if (b[2] - b[0]) > 6 else 1.0
        o.append(f'<text x="{b[0] + .3:.2f}" y="{b[1] + fs:.2f}" font-size="{fs}" fill="{"#1d6a2b" if bot else "#5a2a70"}">{r}</text>')
    for a, b, m in s["rats"]:
        o.append(f'<line x1="{a[0]:.2f}" y1="{a[1]:.2f}" x2="{b[0]:.2f}" y2="{b[1]:.2f}" stroke="{"#d9473a" if m else "#3a72c6"}" stroke-width=".12" opacity=".6"/>')
    o.append("</svg>")
    return "\n".join(o)


if __name__ == "__main__":
    plan = json.load(open(sys.argv[1]))
    s = score(plan)
    name = os.path.splitext(os.path.basename(sys.argv[1]))[0]
    print(f"{name}: {s['W']} x {s['H']} = {s['W'] * s['H']:.0f} mm2, zone gap {s['zd']:.1f}, cost {s['total']:.0f}, mains crossings {s['cross']}, problems {len(s['probs'])}")
    for p in s["probs"]:
        print("  " + p)
    if len(sys.argv) > 2:
        out = os.path.abspath(sys.argv[2])
        sv = out[:-4] + ".svg"
        open(sv, "w").write(svg(s, name))
        gal = os.path.join(HERE, "..", "..", "out", "view", "sandbox", "layouts")     # browser gallery: out/view/sandbox/gallery.html
        os.makedirs(gal, exist_ok=True)
        open(os.path.join(gal, name + ".svg"), "w").write(svg(s, name))
        idx = os.path.join(gal, "index.json")
        lst = json.load(open(idx)) if os.path.exists(idx) else []
        lst = [e for e in lst if e["name"] != name] + [dict(name=name, area=round(s["W"] * s["H"]), W=s["W"], H=s["H"],
                                                            rats=round(s["total"]), problems=s["probs"])]
        json.dump(lst, open(idx, "w"), indent=1)
        w, h = (s["W"] + 8) * 12, (s["H"] + 12) * 12
        subprocess.run(["google-chrome", "--headless", "--no-sandbox", "--disable-gpu", "--hide-scrollbars", f"--screenshot={out}",
                        f"--window-size={w:.0f},{h:.0f}", "file://" + sv], capture_output=True, timeout=60)
