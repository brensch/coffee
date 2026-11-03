#!/usr/bin/env python3
"""Random bottom-left packer for the mains through-hole parts, on top of a fixed sandbox plan.

    python3 pack.py base.json shift tries out.json
Shifts the barrier, PSU, optos and every LV part left by `shift` mm (narrower board), removes the
mains THT parts, then tries `tries` random orders/orientations, placing each part at the first spot
(fewest-y, then x) that fits with its edge rule. Keeps the best by heater-weighted rat's nest.
Mains SMDs on the bottom are dropped from the plan; they get placed afterwards.
"""
import copy, json, random, re, sys
import sb

MAINS_THT = ["J2", "Q1", "J1", "J3", "F1", "Q2", "RV1"]
MAINS_SMD = ["Q3", "C1", "C2", "R1", "R2", "R3", "R4", "R5", "R6"]
# allowed (rotation, edge) per part; connectors' wire side is footprint +y, Q1's tab is -y
CONN = [(270, "L"), (0, "B"), (180, "T")]
RULES = {"J1": CONN, "J2": CONN, "J3": CONN, "Q1": [(90, "L"), (180, "B"), (0, "T")],
         "F1": [(0, None), (90, None)], "Q2": [(0, None), (90, None), (180, None), (270, None)], "RV1": [(0, None), (90, None), (180, None), (270, None)]}


def rbox(ref, rot):
    return sb.box([0, 0, rot, "F"], sb.P[ref]["box"])


STATS = {}


def run(base, shift, tries, seed=1):
    plan = copy.deepcopy(base)
    W, H = plan["W"] - shift, plan["H"]
    place = {**sb.G["place"], **plan["place"]}
    for r in list(place):
        if r in MAINS_THT or r in MAINS_SMD:
            del place[r]
        elif sb.P[r]["kind"] != "mains":
            x, y, rot, s = place[r]
            place[r] = [round(x - shift, 2), y, rot, s]
    mains = [[x if x < 1 else x - shift, y] for x, y in plan["mains"]]
    selv = [[x - shift, y] for x, y in plan["selv"]]
    bx = max(x for x, _ in mains)                       # barrier's left edge
    fixed = [sb.box(pl, sb.P[r]["box"]) for r, pl in place.items() if place[r][3] == "F" or any(q["tht"] for q in sb.P[r]["pads"])]
    rnd = random.Random(seed)
    best = None
    for _ in range(tries):
        order = MAINS_THT[:]
        rnd.shuffle(order)
        boxes, got = list(fixed), {}
        for ref in order:
            opts = RULES[ref][:]
            rnd.shuffle(opts)
            cand = None
            for rot, edge in opts:
                b = rbox(ref, rot)
                w, h = b[2] - b[0], b[3] - b[1]
                xs = sorted({0.2} | {round(o[2] + .05, 2) for o in boxes})
                ys = sorted({0.2} | {round(o[3] + .05, 2) for o in boxes})
                if edge == "L": xs = [-1.0 if ref != "Q1" else 0.2]
                if edge == "B": ys = [H - h + (1.0 if ref != "Q1" else -0.2)]
                if edge == "T": ys = [-1.0 if ref != "Q1" else 0.2]
                for y in ys:
                    for x in xs:
                        nb = [x, y, x + w, y + h]
                        if nb[2] > bx + 0.01 or nb[0] < -1.5 or nb[3] > H + 1.5 or nb[1] < -1.5:
                            continue
                        if any(sb.ovl(nb, o, 0.01) for o in boxes):
                            continue
                        k = (y + h, x)
                        if cand is None or k < cand[0]:
                            cand = (k, ref, [round(x - b[0], 2), round(y - b[1], 2), rot, "F"], nb)
                        break
            if cand is None:
                break
            got[ref] = cand[2]
            boxes.append(cand[3])
        if len(got) < len(MAINS_THT):
            STATS["unplaced:" + ref] = STATS.get("unplaced:" + ref, 0) + 1
            continue
        sw = [r for r in ("J1", "J2", "J3") if rnd.random() < 0.5]
        p = dict(W=W, H=H, mains=mains, selv=selv, place={**place, **got}, swap=sw)
        s = sb.score(p)
        smd = {r for r in sb.P if not any(q["tht"] for q in sb.P[r]["pads"]) and r != "J4"}
        bad = [q for q in s["probs"] if not set(re.findall(r"\b([A-Z]+\d+)\b", q)) & smd]
        if bad:
            for q in bad[:3]:
                STATS[q] = STATS.get(q, 0) + 1
            continue
        TOP.append((s["total"], p))
        if best is None or s["total"] < best[0]:
            best = (s["total"], p)
    return best


TOP = []


if __name__ == "__main__":
    base = json.load(open(sys.argv[1]))
    best = run(base, float(sys.argv[2]), int(sys.argv[3]))
    if not best:
        print(f"shift {sys.argv[2]}: no fit;", sorted(STATS.items(), key=lambda kv: -kv[1])[:8])
        sys.exit(1)
    # mark the dropped SMDs as absent by keeping their old spots; sb reports clashes for re-placement
    json.dump(best[1], open(sys.argv[4], "w"))
    seen, k = set(), 0
    for c, pl in sorted(TOP, key=lambda t: t[0]):
        key = tuple((r, tuple(pl["place"][r])) for r in MAINS_THT) + (tuple(pl["swap"]),)
        if key in seen:
            continue
        seen.add(key); k += 1
        json.dump(pl, open(sys.argv[4].replace(".json", f"-c{k}.json"), "w"))
        if k == 6:
            break
    print(f"shift {sys.argv[2]}: fits, W {best[1]['W']}, rats {best[0]:.0f}")
