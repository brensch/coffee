#!/usr/bin/env python3
"""Fail-fast routing: mains gate, then a race of LV seeds; first clean board wins, the rest are killed.

    source ~/opt/kienv.sh && python3 race.py plan.json [seeds...]

1. Mains gate (~10 s per pass): route the mains only, run KiCad's DRC on it, mark every net whose
   track violates as strict, repeat until the mains are clean. Nothing slow starts on a bad mains.
2. Race: one full route per seed in parallel (LV runs stop after SELV_PATIENCE rounds without
   progress). Progress is printed every 20 s. When the first clean board (0 unconnected, 0 DRC
   errors) lands, the rest get RACE_GRACE seconds (default 120) to finish; then the best clean
   board wins: fewest mains vias, then signal vias, then track length. Stragglers are killed.
"""
import json, os, re, shutil, subprocess, sys, time

HERE = os.path.dirname(os.path.abspath(__file__))
TMP = os.environ.get("RACE_DIR", os.path.join(os.environ.get("CLAUDE_JOB_DIR", "/tmp"), "tmp", "race"))
ERR = r"^\[(clearance|unconnected_items|shorting_items|hole_clearance|hole_near_hole|tracks_crossing|items_not_allowed|copper_edge_clearance)\]"


def builddir(name):
    d = os.path.join(TMP, name)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    shutil.copy(os.path.join(HERE, "build", "parts-120v.json"), d)
    return d


def mains_gate(plan, env):
    strict = [n for n in env.get("STRICT_NETS", "").split(",") if n]
    spec = json.load(open(os.path.join(HERE, "build", "parts-120v.json")))
    mains = set(spec["mains_nets"])
    for i in range(5):
        d = builddir("gate")
        t0 = time.time()
        r = subprocess.run([sys.executable, "-u", "layout.py", "--no-route"], cwd=HERE, capture_output=True, text=True,
                           env=dict(env, BUILD_DIR=d, PLACE_JSON=plan, OWN_SELV="0", KEEP_GOING="1", STRICT_NETS=",".join(strict) or ","))
        fails = re.findall(r"mainsroute: (\d+) pads unrouted", r.stdout)
        import pcbnew
        b = pcbnew.LoadBoard(os.path.join(d, "coffee-controller-120v-placed.kicad_pcb"))
        rpt = os.path.join(d, "gate.rpt")
        pcbnew.WriteDRCReport(b, rpt, pcbnew.EDA_UNITS_MILLIMETRES, True)
        bad = set()
        for blk in re.split(r"\n(?=\[)", open(rpt).read()):
            if blk.startswith("[clearance") or blk.startswith("[shorting"):
                bad |= {n for n in re.findall(r"Track \[([^\]]+)\]", blk) if n in mains}
        unrouted = int(fails[-1]) if fails else 0
        print(f"mains gate pass {i + 1} ({time.time() - t0:.0f} s): unrouted {unrouted}, violating nets {sorted(bad) or 'none'}", flush=True)
        if unrouted:
            print("  mains don't route on this placement: fix the plan before any LV routing", flush=True)
            return None
        if not bad:
            return strict
        strict += sorted(bad - set(strict))
    return None


def score(d):
    """(mains vias, total vias, track length mm) of a finished board: lower is better."""
    import pcbnew
    spec = json.load(open(os.path.join(HERE, "build", "parts-120v.json")))
    mains = set(spec["mains_nets"])
    b = pcbnew.LoadBoard(os.path.join(d, "coffee-controller-120v.kicad_pcb"))
    vias = [t for t in b.GetTracks() if t.Type() == pcbnew.PCB_VIA_T]
    length = sum(pcbnew.ToMM(t.GetLength()) for t in b.GetTracks() if t.Type() == pcbnew.PCB_TRACE_T)
    return (sum(1 for v in vias if v.GetNetname() in mains), sum(1 for v in vias if v.GetNetname() != "GND"), round(length))


def race(plan, seeds, env, strict, grace=float(os.environ.get("RACE_GRACE", "120"))):
    """Run all seeds; once the first clean board lands, the rest get `grace` seconds to finish,
    then the best clean board (fewest mains vias, then signal vias, then track length) wins."""
    procs = {}
    for s in seeds:
        d = builddir(f"seed{s}")
        procs[s] = (subprocess.Popen([sys.executable, "-u", "layout.py"], cwd=HERE, stdout=open(os.path.join(d, "run.log"), "w"),
                                     stderr=subprocess.STDOUT, env=dict(env, BUILD_DIR=d, PLACE_JSON=plan, SELV_SEED=str(s),
                                                                         STRICT_NETS=",".join(strict) or ",", KEEP_GOING="1")), d)
    t0 = time.time()
    clean, first = [], None
    while procs and not (first and time.time() - first > grace):
        time.sleep(20)
        line = []
        for s, (p, d) in list(procs.items()):
            log = open(os.path.join(d, "run.log")).read()
            last = (re.findall(r"(selv round \d+: \d+ pads unrouted|own route:.*)", log) or ["mains"])[-1]
            line.append(f"s{s}: {last[:40]}")
            if p.poll() is not None:
                rpt = os.path.join(d, "coffee-controller-120v-drc.rpt")
                n = len(re.findall(ERR, open(rpt).read(), re.M)) if os.path.exists(rpt) else 99
                if n == 0:
                    sc = score(d)
                    clean.append((sc, s, d))
                    first = first or time.time()
                    print(f"  seed {s} clean: mains vias {sc[0]}, signal vias {sc[1]}, track {sc[2]} mm", flush=True)
                else:
                    print(f"  seed {s} finished with {n} DRC errors", flush=True)
                del procs[s]
        print(f"[{time.time() - t0:4.0f} s] " + " | ".join(line), flush=True)
    for p, _ in procs.values():
        p.kill()
    if not clean:
        return None
    clean.sort()
    sc, s, d = clean[0]
    return s, d, sc


if __name__ == "__main__":
    plan = os.path.abspath(sys.argv[1])
    seeds = [int(s) for s in sys.argv[2:]] or [2, 5, 13, 14, 21, 34]
    env = dict(os.environ)
    strict = mains_gate(plan, env)
    if strict is None:
        sys.exit(1)
    print(f"mains clean with strict nets {strict or 'none'}; racing seeds {seeds}", flush=True)
    w = race(plan, seeds, env, strict)
    print(f"WINNER seed {w[0]} {w[2]} (mains vias, signal vias, track mm): {w[1]}" if w else "no clean board this round", flush=True)
    sys.exit(0 if w else 1)
