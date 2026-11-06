#!/usr/bin/env python3
"""For profiles the design-point formula gets wrong: brew each across a range of
grinds in the emulator and keep the grind where it best tracks its own targets.

    python3 tools/sweep_grind.py profiles/community "A-Flow / default-dark" ...
"""
import json, math, os, sys, tempfile, concurrent.futures
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import profile_check as pc

GRINDS = [0.4, 0.55, 0.7, 0.85, 1.0, 1.15, 1.3, 1.5, 1.75, 2.0]


def score(r):
    # tracking error (bar and ml/s weighted alike) + penalties for clearly broken shots
    s = (0 if math.isnan(r.get("mpe", math.nan)) else r["mpe"]) + (0 if math.isnan(r.get("mfe", math.nan)) else r["mfe"])
    if r.get("cup", 0) < 15:
        s += 5
    if r.get("time", 0) > 90:
        s += 2
    return s


def sweep(path):
    base = json.load(open(path))
    results = []
    with tempfile.TemporaryDirectory() as d:
        for g in GRINDS:
            p = dict(base, grind=g)
            tmp = os.path.join(d, f"g{g}.json")
            json.dump(p, open(tmp, "w"))
            r = pc.check(tmp, pc.os.path.join(pc.ROOT, "models/gaggia_classic_pro_e24_120v.toml"))
            results.append((score(r), g, r))
    best = min(results, key=lambda x: x[0])
    return base, best, results


def main():
    root, names = sys.argv[1], set(sys.argv[2:])
    paths = [os.path.join(dp, f) for dp, _, fs in os.walk(root) for f in fs if f.endswith(".json")]
    paths = [p for p in paths if json.load(open(p))["name"] in names]
    with concurrent.futures.ThreadPoolExecutor(8) as ex:
        for path, (base, (s, g, r), results) in zip(paths, ex.map(sweep, paths)):
            old = base.get("grind")
            curve = " ".join(f"{gg}:{ss:.1f}" for ss, gg, _ in results)
            print(f"{base['name'][:34]:34} x{old} -> x{g}  (score by grind: {curve})")
            base["grind"] = g
            json.dump(base, open(path, "w"), indent=2, ensure_ascii=False)
            open(path, "a").write("\n")


if __name__ == "__main__":
    main()
