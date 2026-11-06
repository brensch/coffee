#!/usr/bin/env python3
"""Give every profile a grind derived from its own design point.

Grind in coffee's scale is a puck-resistance multiplier (1.0 = a normal espresso
puck). A profile is built to run at some pressure P and flow Q in its main
phase, i.e. a puck resistance R = P / Q. Using the model's puck law
(R = r_per_g * dose * grind * (1 + compression * P) * ~1.12 for swell/erosion)
the grind that gives that resistance is

    grind = (P / Q) / (r_per_g * dose * (1 + compression * P) * 1.12)

Design point per profile:
  * pressure phases: P = the profile's highest pressure target; Q = the flow the
    author states in the notes (OVERRIDES), else a standard 2.0 ml/s.
  * flow phases: Q = the profile's main flow target; P = its pressure cap if it
    has one, else 8.5 bar.
Sanity check: Classic 9 bar -> 1.00, Turbo flow -> ~0.37, Magic -> ~1.3 (the
values tuned by hand in the emulator).

    python3 tools/fit_grind.py profiles/community   # writes "grind" into each JSON
"""
import json, os, sys

R_PER_G, DOSE, COMP, SWELL = 0.095, 18.0, 0.15, 1.12   # models/gaggia_classic_pro_e24_120v.toml [puck]

# Design flow (ml/s) stated in the profile authors' notes.
OVERRIDES = {
    "TurboBloom": (None, 3.75, "notes: ending flow 3-4.5 ml/s"),
    "TurboTurbo": (None, 3.75, "notes: ending flow 3-4.5 ml/s"),
    "Extractamundo Dos!": (None, 3.25, "notes: ending flow 2.5-4 ml/s"),
    "Best practice (light roast)": (None, 2.5, "notes: 2.5 ml/s at peak pressure"),
}


# Qualitative guidance from the notes (applied after the design-point grind).
NOTE_ADJUST = {
    "Hybrid pour over espresso": (1.35, "notes: 'grind your coffee very finely'"),
    "Innovative long preinfusion": (1.35, "notes: 'grind your coffee very finely'"),
    "E61 rocketing up to 10 bar": (1.15, "notes: 'you will need to grind finer'"),
}
# Adaptive profiles adjust to the grind: start from normal.
FIXED = {
    "Adaptive v2": (1.0, "adaptive profile: adjusts to your grind; start at normal"),
    "Adaptive v3": (1.0, "adaptive profile; notes: fine enough that ~4 g drips during preinfusion"),
    "I got your back": (1.0, "notes: designed to work at any grinder setting"),
}


def design_point(p):
    ov = OVERRIDES.get(p["name"])
    phases = p["phases"]
    pres = [ph for ph in phases if "pressure" in ph["target"] and ph["target"]["pressure"] > 3]
    flows = [ph for ph in phases if "flow" in ph["target"] and ph["target"]["flow"] > 0.5]
    # Main phase: the last substantial one (where most of the cup is made).
    main = next((ph for ph in reversed(phases) if ph in pres or ph in flows), None)
    if main is None:
        return None
    if main in pres or (pres and not flows):
        P = max(ph["target"]["pressure"] for ph in pres)
        Q = ov[1] if ov else 2.0
        basis = ov[2] if ov else "pressure profile: peak pressure at 2.0 ml/s"
    else:
        Q = main["target"]["flow"]
        if ov:
            Q = ov[1]
        P = main.get("limit") or 8.5
        basis = (ov[2] + ", " if ov else "") + ("flow profile: flow at its pressure cap" if main.get("limit") else "flow profile: flow at 8.5 bar")
    return P, Q, basis


def grind_for(P, Q):
    return (P / Q) / (R_PER_G * DOSE * (1 + COMP * P) * SWELL)


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "profiles/community"
    rows = []
    for dp, _, fns in os.walk(root):
        for fn in sorted(fns):
            if not fn.endswith(".json"):
                continue
            path = os.path.join(dp, fn)
            p = json.load(open(path))
            d = design_point(p)
            if not d:
                continue
            P, Q, basis = d
            g = grind_for(P, Q)
            if p["name"] in NOTE_ADJUST:
                k, why = NOTE_ADJUST[p["name"]]
                g *= k
                basis += f"; {why}"
            if p["name"] in FIXED:
                g, basis = FIXED[p["name"]]
            g = round(min(max(g, 0.25), 3.0), 2)
            p["grind"] = g
            json.dump(p, open(path, "w"), indent=2, ensure_ascii=False)
            open(path, "a").write("\n")
            rows.append((p["name"], p.get("author", ""), g, P, Q, basis))
    rows.sort(key=lambda r: r[0].lower())
    for r in rows:
        print(f"{r[0][:42]:42} {r[1][:9]:9} grind x{r[2]:<5}  P {r[3]:4.1f} bar  Q {r[4]:4.2f} ml/s  ({r[5]})")
    print(f"{len(rows)} profiles updated")


if __name__ == "__main__":
    main()
