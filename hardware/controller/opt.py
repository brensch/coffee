#!/usr/bin/env python3
"""Place every SMD part where its weighted runs are shortest (sb.WEIGHT), legally.

    python3 opt.py plan.json out.json [passes]
Coordinate descent: each pass visits every movable part and tries every 1 mm grid spot (then 0.25 mm
around the best), rotations 0/90/180/270, both sides; keeps the cheapest spot with no overlap, no
through-hole pin under it, pads inside their own zone and on the board. THT parts and J4 stay put.
"""
import json, math, sys
import sb

STEP, FINE = 1.0, 0.25
MARGIN = float(__import__("os").environ.get("MAINS_MARGIN", "0.0"))
LV_GAP = float(__import__("os").environ.get("LV_GAP", "0.5"))          # room between LV parts for tracks and pour   # extra mm between mains nets: room for tracks


SWAP = set()


def padlist(ref):
    ps = sb.P[ref]["pads"]
    if ref not in SWAP:
        return ps
    out = []
    for q in ps:
        if q["n"] in ("1", "2"):
            other = next(p for p in ps if p["n"] == ("2" if q["n"] == "1" else "1"))
            q = dict(q, net=other["net"], mains=other["mains"])
        out.append(q)
    return out


def geom(ref, pl):
    return sb.box(pl, sb.P[ref]["box"]), [(*sb.xf(pl, q["x"], q["y"]), q) for q in padlist(ref)]


def mst_edges(pts):
    if len(pts) < 2:
        return []
    d = [math.inf] * len(pts); par = [-1] * len(pts)
    d[0], used, out = 0, [False] * len(pts), []
    for _ in pts:
        i = min((k for k in range(len(pts)) if not used[k]), key=d.__getitem__)
        used[i] = True
        if par[i] >= 0:
            out.append((pts[par[i]], pts[i]))
        for k in range(len(pts)):
            if not used[k]:
                dd = math.dist(pts[i], pts[k])
                if dd < d[k]:
                    d[k], par[k] = dd, i
    return out


def mst(pts):
    if len(pts) < 2:
        return 0.0
    d = [math.inf] * len(pts)
    d[0], used, tot = 0, [False] * len(pts), 0.0
    for _ in pts:
        i = min((k for k in range(len(pts)) if not used[k]), key=d.__getitem__)
        used[i] = True
        tot += d[i]
        for k in range(len(pts)):
            if not used[k]:
                d[k] = min(d[k], math.dist(pts[i], pts[k]))
    return tot


def main():
    plan = json.load(open(sys.argv[1]))
    passes = int(sys.argv[3]) if len(sys.argv) > 3 else 3
    W, H = plan["W"], plan["H"]
    place = {**sb.G["place"], **plan["place"]}
    SWAP.update(plan.get("swap", []))
    zones = {True: plan["mains"], False: plan["selv"]}
    tht = {r: any(q["tht"] for q in sb.P[r]["pads"]) for r in place}
    movable = [r for r in place if not tht[r] and r not in ("J4",) and r not in plan.get("lock", [])]
    G = {r: geom(r, place[r]) for r in place}
    nets = {}
    for r in place:
        for q in padlist(r):
            if q["net"] and q["net"] != "GND":
                nets.setdefault(q["net"], set()).add(r)

    keepouts = plan.get("keepout", [])

    def legal(ref, pl, b, pads):
        side = pl[3]
        for k in keepouts:
            if side in k["sides"] and ref not in k.get("except", []) and sb.ovl(b, k["rect"], 0.0):
                return False
        if b[0] < 0 or b[1] < 0 or b[2] > W or b[3] > H:
            return False
        for x, y, q in pads:
            hw, hh = (q["h"], q["w"]) if pl[2] % 180 else (q["w"], q["h"])
            if not q["tht"] and not (0.55 < x - hw / 2 and x + hw / 2 < W - 0.55 and 0.55 < y - hh / 2 and y + hh / 2 < H - 0.55):
                return False
            if q["net"]:
                z = zones[q["mains"]]
                hw, hh = (q["h"], q["w"]) if pl[2] % 180 else (q["w"], q["h"])
                if not all(sb.inside(z, c) for c in ((x - hw / 2, y - hh / 2), (x + hw / 2, y - hh / 2), (x - hw / 2, y + hh / 2), (x + hw / 2, y + hh / 2))):
                    return False
        for o, (ob, opads) in G.items():
            if o == ref:
                continue
            for x, y, q in pads:                     # 2 mm between different mains nets
                if not (q["net"] and q["mains"]):
                    continue
                hw, hh = (q["h"], q["w"]) if pl[2] % 180 else (q["w"], q["h"])
                ra = (x - hw / 2, y - hh / 2, x + hw / 2, y + hh / 2)
                for X, Y, Q in opads:
                    if not (Q["net"] and Q["mains"] and sb.mains_conflict(q["net"], Q["net"])):
                        continue
                    if place[o][3] != side and not (q["tht"] or Q["tht"]):
                        continue
                    W2, H2 = (Q["h"], Q["w"]) if place[o][2] % 180 else (Q["w"], Q["h"])
                    if sb.rectgap(ra, (X - W2 / 2, Y - H2 / 2, X + W2 / 2, Y + H2 / 2)) < sb.MAINS_CLR + MARGIN:
                        return False
            gap = LV_GAP if not any(q["mains"] for q in sb.P[ref]["pads"]) and not any(q["mains"] for q in sb.P[o]["pads"]) else 0.05
            if place[o][3] == side and sb.ovl(b, ob, -gap):
                return False
            if place[o][3] != side and tht[o]:
                for x, y, q in opads:
                    if q["tht"]:
                        r_ = max(q["w"], q["h"]) / 2 + 0.5
                        if b[0] < x + r_ and x - r_ < b[2] and b[1] < y + r_ and y - r_ < b[3]:
                            return False
        return True

    mains_nets = {q["net"] for r in place for q in padlist(r) if q["net"] and q["mains"]}
    FIXED = {}

    def fixed_edges(ref):
        """MST edges of every mains net that doesn't touch ref (they don't move while ref moves)."""
        mynets = {q["net"] for q in padlist(ref) if q["net"]}
        out = []
        for n in mains_nets - mynets:
            pts = [(x, y) for o in nets.get(n, ()) for x, y, q in G[o][1] if q["net"] == n]
            out += [(a, b, n) for a, b in mst_edges(pts)]
        return out

    VIA_COST = 5.0     # a net whose SMD pads sit on both sides needs a via

    def sides_of(n, ref, myside):
        s = set()
        for o2 in nets.get(n, ()):
            if o2 == ref:
                continue
            for x, y, q in G[o2][1]:
                if q["net"] == n:
                    s |= {"F", "B"} if q["tht"] else {place[o2][3]}
        return s

    def cost(ref, pads, side=None):
        c = 0.0
        mine = {}
        for x, y, q in pads:
            if q["net"] and q["net"] != "GND":
                mine.setdefault(q["net"], []).append((x, y))
        if side is not None:
            for n in mine:
                s = sides_of(n, ref, side)
                if s and side not in s:
                    c += VIA_COST * sb.WEIGHT.get(n, 1)
        my_edges = []
        for n, pts in mine.items():
            others = [(x, y) for o in nets[n] if o != ref for x, y, q in G[o][1] if q["net"] == n]
            allp = pts + others
            if n in mains_nets:
                es = mst_edges(allp)
                c += sum(math.dist(a, b) for a, b in es) * sb.WEIGHT.get(n, 1)
                my_edges += [(a, b, n) for a, b in es]
            else:
                c += mst(allp) * sb.WEIGHT.get(n, 1)
        if my_edges:
            fe = FIXED[ref]
            cr = 0
            for i, (a, b, n1) in enumerate(my_edges):
                for (p, q_, n2) in fe:
                    if sb.mains_conflict(n1, n2) and sb.segx(a, b, p, q_):
                        cr += 1
                for (p, q_, n2) in my_edges[i + 1:]:
                    if sb.mains_conflict(n1, n2) and sb.segx(a, b, p, q_):
                        cr += 1
            c += sb.CROSS * cr
        return c

    seed = int(__import__("os").environ.get("OPT_SEED", "0"))
    if seed:                                   # random restart: scatter the mains SMDs, shuffle the visiting order
        import random
        rnd = random.Random(seed)
        for r in movable:
            if any(q["mains"] for q in sb.P[r]["pads"]):
                place[r] = [round(rnd.uniform(1, plan["mains"][1][0] - 1), 1), round(rnd.uniform(1, H - 1), 1), rnd.choice([0, 90]), rnd.choice("FB")]
                G[r] = geom(r, place[r])
        rnd.shuffle(movable)
    rots = {r: ([0, 90] if len(sb.P[r]["pads"]) <= 2 and r not in ("D1", "D2") else [0, 90, 180, 270]) for r in movable}
    for p in range(passes):
        moved = 0
        for ref in (movable if seed else sorted(movable, key=lambda r: -len(sb.P[r]["pads"]))):
            cur = place[ref]
            FIXED[ref] = fixed_edges(ref)
            b, pads = G[ref]
            wrong_side = (plan.get("lv_side") and not any(q["mains"] for q in sb.P[ref]["pads"]) and cur[3] != plan["lv_side"]) \
                or (plan.get("smd_side") and cur[3] != plan["smd_side"])
            best = (cost(ref, pads, cur[3]) if legal(ref, cur, b, pads) and not wrong_side else math.inf, cur)

            def scan(xs, ys, sides, rr):
                nonlocal best
                for side in sides:
                    for rot in rr:
                        for x in xs:
                            for y in ys:
                                pl = [round(x, 2), round(y, 2), rot, side]
                                b, pads = geom(ref, pl)
                                if b[0] < 0 or b[1] < 0 or b[2] > W or b[3] > H:
                                    continue
                                c = cost(ref, pads, side)
                                if c < best[0] - 1e-6 and legal(ref, pl, b, pads):
                                    best = (c, pl)
            n = lambda a, b_, s: [a + i * s for i in range(int((b_ - a) / s) + 1)]
            sides = ["F", "B"]
            if plan.get("lv_side") and not any(q["mains"] for q in sb.P[ref]["pads"]):
                sides = [plan["lv_side"]]
            if plan.get("smd_side"):
                sides = [plan["smd_side"]]
            scan(n(0, W, STEP), n(0, H, STEP), sides, rots[ref])
            bx, by, br, bs = best[1]
            scan(n(bx - 1, bx + 1, FINE), n(by - 1, by + 1, FINE), [bs], [br])
            if best[1] != cur:
                moved += 1
                place[ref] = best[1]
                G[ref] = geom(ref, best[1])
        s = sb.score({**plan, "place": place})
        print(f"pass {p + 1}: moved {moved}, weighted runs {s['total']:.0f}, problems {len(s['probs'])}", flush=True)
        if not moved:
            break
    json.dump({**plan, "place": place}, open(sys.argv[2], "w"))


if __name__ == "__main__":
    main()
