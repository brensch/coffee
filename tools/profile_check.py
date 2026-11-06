#!/usr/bin/env python3
"""Brew every profile in a directory on the emulated machine and grade it.

    python3 tools/profile_check.py profiles/community [--model models/...toml] [-j 6]

For each profile: one shot on a warm machine, then the trace is checked for how
it ended, how far through its phases it got, shot time, yield, peak pressure,
how well the pump tracked the targets, and faults. Writes out/profile_check.md.
"""
import argparse, concurrent.futures, csv, json, math, os, subprocess, sys, tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EMU = os.path.join(ROOT, "target/release/coffee-emu")

SCN = """name "profile check"
machine "{model}"
model sim.initial_temp_c = {temp}
model puck.grind = {grind}
duration 4min
trace_every 100ms
trace time, engine.state, engine.faults, engine.phase, hydraulics.pressure, engine.pressure, engine.target_pressure, engine.flow, engine.target_flow, engine.pump_duty, puck.flow, cup.mass, opv.flow
engine_arg "--profile"
engine_arg "{profile}"
engine_arg "--profile-library"
engine_arg "/nonexistent"
when engine.state == ready for 3s as go: brew on
when engine.state == brew_done from go as done: brew off
expect eventually engine.state == brew_done from go
"""


def check(path, model):
    prof = json.load(open(path))
    with tempfile.TemporaryDirectory() as d:
        scn = os.path.join(d, "p.scn")
        trace = os.path.join(d, "t.csv")
        temp = prof.get("temperature_c") or 93
        open(scn, "w").write(SCN.format(model=os.path.abspath(model), profile=os.path.abspath(path), temp=temp, grind=prof.get("grind") or 1.0))
        subprocess.run([EMU, "run", scn, "--trace", trace], capture_output=True, text=True, cwd=ROOT)
        if not os.path.exists(trace):
            return dict(name=prof["name"], grade="ERROR", notes="emulator run failed")
        rows = list(csv.DictReader(open(trace)))
    f = lambda r, k: float(r[k]) if r.get(k) else math.nan
    st = lambda r: int(f(r, "engine.state")) if not math.isnan(f(r, "engine.state")) else -1
    shot = [r for r in rows if st(r) == 3]
    if not shot:
        return dict(name=prof["name"], grade="FAIL", notes="never started brewing")
    t0, t1 = f(shot[0], "time"), f(shot[-1], "time")
    after = [r for r in rows if t1 < f(r, "time") <= t1 + 6]
    cup = f(after[-1], "cup.mass") if after else f(shot[-1], "cup.mass")
    faults = max(f(r, "engine.faults") for r in rows)
    n_phases = len(prof["phases"])
    reached = int(max(f(r, "engine.phase") for r in shot)) + 1
    peak = max(f(r, "hydraulics.pressure") for r in shot)
    # tracking: pressure phases and flow phases, ignoring the first 1.5 s of each phase change
    perr, ferr, unreachable = [], [], 0
    capped_n = [0]
    last_phase, phase_t = None, t0
    for r in shot:
        ph = f(r, "engine.phase")
        if ph != last_phase:
            last_phase, phase_t = ph, f(r, "time")
        if f(r, "time") - phase_t < 1.5:
            continue
        tp, tf = f(r, "engine.target_pressure"), f(r, "engine.target_flow")
        phase_def = prof["phases"][int(ph)] if 0 <= ph < n_phases else {}
        limit = phase_def.get("limit")
        if not math.isnan(tp) and tp > 1.0:
            # The phase's flow cap holding pressure below target is the profile working as designed.
            capped = limit is not None and f(r, "engine.flow") >= limit - 0.6  # flow estimate lags ~0.25 s
            # So is the pump simply being at full stroke rate (it can't push harder).
            pinned = f(r, "engine.target_pressure") - f(r, "engine.pressure") > 0.5 and f(r, "engine.flow") > 0 and r.get("engine.pump_duty") and float(r["engine.pump_duty"]) > 0.97
            # Above target with the pump off: the pressure can only bleed away through
            # the puck (the valve stays shut mid-shot), so the controller can't help it.
            bleeding = f(r, "engine.pressure") > tp and r.get("engine.pump_duty") and float(r["engine.pump_duty"]) < 0.02
            if capped:
                capped_n[0] += 1
            elif not pinned and not bleeding:
                perr.append(abs(f(r, "engine.pressure") - tp))
        if not math.isnan(tf) and tf > 0.3:
            # A flow phase held back by its own pressure cap is working as designed.
            if limit is not None and f(r, "engine.pressure") >= limit - 0.4:
                capped_n[0] += 1
                continue
            e = abs(f(r, "engine.flow") - tf)
            ferr.append(e)
            if f(r, "engine.flow") < tf - 1.0 and f(r, "hydraulics.pressure") > 8:
                unreachable += 1
    mpe = sum(perr) / len(perr) if perr else math.nan
    mfe = sum(ferr) / len(ferr) if ferr else math.nan
    notes = []
    stop_w = (prof.get("stop") or {}).get("weight_g")
    if faults > 0:
        notes.append(f"fault bitmask {int(faults)}")
    if reached < n_phases and not (stop_w and cup >= stop_w - 2):
        notes.append(f"ended in phase {reached}/{n_phases}")
    if not math.isnan(mpe) and mpe > 0.6:
        notes.append(f"pressure tracking off by {mpe:.1f} bar")
    if not math.isnan(mfe) and mfe > 0.8:
        notes.append(f"flow tracking off by {mfe:.1f} ml/s")
    if unreachable > 10:
        notes.append("asks for more flow than the pump gives at pressure")
    if cup < 15:
        notes.append(f"tiny yield ({cup:.0f} g)")
    grade = "OK" if not notes else ("FAULT" if faults > 0 else "CHECK")
    return dict(name=prof["name"], author=prof.get("author", ""), grade=grade, time=t1 - t0, cup=cup, peak=peak,
                mpe=mpe, mfe=mfe, phases=f"{reached}/{n_phases}", notes="; ".join(notes), file=os.path.relpath(path, ROOT))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("dirs", nargs="+")
    ap.add_argument("--model", default=os.path.join(ROOT, "models/gaggia_classic_pro_e24_120v.toml"))
    ap.add_argument("-j", type=int, default=os.cpu_count() or 4)
    a = ap.parse_args()
    files = sorted(os.path.join(dp, fn) for d in a.dirs for dp, _, fns in os.walk(d) for fn in fns if fn.endswith(".json"))
    with concurrent.futures.ThreadPoolExecutor(a.j) as ex:
        res = list(ex.map(lambda p: check(p, a.model), files))
    fmt = lambda x, d=1: "–" if x is None or (isinstance(x, float) and math.isnan(x)) else f"{x:.{d}f}"
    lines = ["| Profile | By | Grade | Time s | Yield g | Peak bar | P err | F err | Phases | Notes |", "|---|---|---|---|---|---|---|---|---|---|"]
    order = {"OK": 0, "CHECK": 1, "FAULT": 2, "FAIL": 3, "ERROR": 4}
    for r in sorted(res, key=lambda r: (order.get(r["grade"], 9), r["name"].lower())):
        lines.append(f"| {r['name']} | {r.get('author','')} | {r['grade']} | {fmt(r.get('time'))} | {fmt(r.get('cup'),0)} | {fmt(r.get('peak'))} | "
                     f"{fmt(r.get('mpe'),2)} | {fmt(r.get('mfe'),2)} | {r.get('phases','')} | {r.get('notes','')} |")
    counts = {g: sum(1 for r in res if r["grade"] == g) for g in order}
    summary = ", ".join(f"{v} {k}" for k, v in counts.items() if v)
    os.makedirs(os.path.join(ROOT, "out"), exist_ok=True)
    out = os.path.join(ROOT, "out/profile_check.md")
    open(out, "w").write(f"# Profile check: {len(res)} profiles ({summary})\n\n" + "\n".join(lines) + "\n")
    print(f"{len(res)} profiles: {summary}  ->  {out}")


if __name__ == "__main__":
    main()
