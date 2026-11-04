#!/usr/bin/env python3
"""Live view of every board job, for out/view/jobs.html (polls out/view/jobs/status.json).

Every 2 s: finds the running layout/routing/optimisation/test processes, reads what each is working
on (plan, seed, build dir from its environment), its elapsed time and CPU, and the tail of whatever
file its stdout goes to. Also lists recently finished runs with their last lines.

    nohup python3 jobs_watch.py &        # then open http://localhost:8099/out/view/jobs.html
"""
import glob, json, os, re, subprocess, time

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "..", "out", "view", "jobs")
JOB = os.environ.get("CLAUDE_JOB_DIR", "")
WATCH = re.compile(r"(layout\.py|ext_route\.py|opt\.py|pack\.py|fastroute|freerouting.*\.jar|fr-2\.1\.0\.jar|"
                   r"coffee_board-|cargo (test|build)|sim/run\.py|cosim\.py|jlc\.py|apply_silk\.py|gen_kicad_sch\.py)")
KIND = [("ext_route.py", "build + external router"), ("fastroute -", "fastroute (Rust Specctra router)"), ("/fastroute", "fastroute (Rust Specctra router)"),
        ("freerouting", "Freerouting"), ("fr-2.1.0", "Freerouting 2.1"), ("layout.py", "placement"),
        ("opt.py", "placement optimiser"), ("pack.py", "big-part packer"), ("coffee_board-", "Rust tests"), ("cargo", "cargo"),
        ("sim/run.py", "SPICE suite"), ("cosim.py", "co-simulation"), ("jlc.py", "JLC export"), ("apply_silk.py", "silkscreen"),
        ("gen_kicad_sch.py", "KiCad schematic export")]


def tail(path, n=30):
    try:
        with open(path, "rb") as f:
            f.seek(0, 2); size = f.tell(); f.seek(max(0, size - 12000))
            lines = f.read().decode("utf-8", "replace").splitlines()
        lines = [l for l in lines if not l.startswith(("Warn", "./src/common", "swig/python"))]
        return lines[-n:], size, os.path.getmtime(path)
    except OSError:
        return [], 0, 0


def procs():
    out = subprocess.run(["ps", "-eo", "pid,ppid,etimes,pcpu,rss,args", "--no-headers"], capture_output=True, text=True).stdout
    rows = []
    for line in out.splitlines():
        parts = line.split(None, 5)
        if len(parts) < 6 or not WATCH.search(parts[5]) or "jobs_watch" in parts[5] or parts[5].startswith(("bash", "/bin/bash", "timeout")):
            continue
        pid = int(parts[0])
        env = {}
        try:
            for kv in open(f"/proc/{pid}/environ", "rb").read().split(b"\0"):
                k, _, v = kv.decode("utf-8", "replace").partition("=")
                if k in ("BUILD_DIR", "PLACE_JSON", "SELV_SEED", "STRICT_NETS", "MAINS_ROUTER", "GND_TREE", "OPT_SEED"):
                    env[k] = v
        except OSError:
            pass
        try:
            stdout = os.readlink(f"/proc/{pid}/fd/1")
        except OSError:
            stdout = ""
        kind = next((k for key, k in KIND if key in parts[5]), "job")
        lines, size, mt = tail(stdout) if stdout.startswith("/") and not stdout.startswith("/dev") else ([], 0, 0)
        live = None
        m = re.search(r"--live(?:=(\d+))?", parts[5])
        if m:
            live = f"http://127.0.0.1:{m.group(1) or 7878}"
        rows.append(dict(pid=pid, ppid=int(parts[1]), elapsed=int(parts[2]), cpu=float(parts[3]), rss_mb=int(parts[4]) // 1024,
                         kind=kind, cmd=parts[5][:300], env=env, stdout=stdout, tail=lines, log_size=size,
                         log_age=round(time.time() - mt) if mt else None, live=live))
    return rows


def recent(running_logs):
    files = glob.glob(os.path.join(JOB, "tmp", "**", "run.log"), recursive=True) + \
            glob.glob("/tmp/claude-1000/*/*/tasks/*.output")
    out = []
    for f in sorted(files, key=lambda p: -os.path.getmtime(p))[:12]:
        if f in running_logs or time.time() - os.path.getmtime(f) > 3 * 3600:
            continue
        lines, size, mt = tail(f, 8)
        out.append(dict(path=f.replace(JOB, "$JOB"), age=round(time.time() - mt), tail=lines))
    return out


RUNS = os.path.join(OUT, "runs")
DRC_LINE = re.compile(r"^\[(\w+)\]: ([^(\n]*)(?:\((?:rule|netclass) '?([^;']*))?", re.M)


XY = re.compile(r"@\(([-\d.]+) mm, ([-\d.]+) mm\): (.*)")


def marks(stem, r):
    """Where each problem is, in render coordinates: DRC errors (not silk) and unrouted connections."""
    try:
        head = open(stem + "-top.svg").read(2000)
        vb = [float(v) for v in re.search(r'viewBox="([^"]+)"', head).group(1).split()]
        pcb = open(stem + ".kicad_pcb").read()
    except (OSError, AttributeError):
        return None
    xs, ys = [], []
    for m in re.finditer(r"\(gr_(?:line|rect) \(start ([-\d.]+) ([-\d.]+)\) \(end ([-\d.]+) ([-\d.]+)\)\s+\(stroke \(width [\d.]+\) \(type \w+\)\)(?: \(fill \w+\))? \(layer \"Edge\.Cuts\"\)", pcb):
        xs += [float(m[1]), float(m[3])]; ys += [float(m[2]), float(m[4])]
    if not xs:
        return None
    ox = (vb[2] - (max(xs) - min(xs))) / 2 - min(xs)
    oy = (vb[3] - (max(ys) - min(ys))) / 2 - min(ys)
    out = dict(viewBox=" ".join(f"{v:.3f}" for v in vb), drc=[], unrouted=[])
    try:
        rpt = open(stem + "-drc.rpt").read()
    except OSError:
        rpt = ""
    for blk in re.split(r"\n(?=\[)", rpt):
        m = re.match(r"\[(\w+)\]: (.*)", blk)
        if not m or m[1].startswith(("silk", "lib_")):
            continue
        pts = [(float(a) + ox, float(b) + oy, c.strip()) for a, b, c in XY.findall(blk)]
        if pts:
            msg = re.sub(r"\(IEC[^)]*\)", "", m[2]).strip()
            out["drc"].append(dict(kind=m[1], msg=msg, pts=pts))
    for u in r.get("_unrouted_raw", []):
        out["unrouted"].append(dict(net=u["net"], a=[u["from_xy"][0] + ox, u["from_xy"][1] + oy], b=[u["to_xy"][0] + ox, u["to_xy"][1] + oy]))
    return out


def runs():
    """Every finished board build under the job's tmp dir: what it was, what came out, its renders."""
    out = []
    dirs = {os.path.dirname(p) for pat in ("*-drc.rpt", "*-fastroute.json")
            for p in glob.glob(os.path.join(JOB, "tmp", "**", "coffee-controller-120v" + pat[1:]), recursive=True)}
    for d in sorted(dirs, key=lambda p: -os.path.getmtime(p))[:40]:
        name = os.path.relpath(d, os.path.join(JOB, "tmp")).replace("/", "_")
        stem = os.path.join(d, "coffee-controller-120v")
        r = dict(name=name, dir=d.replace(JOB, "$JOB"), mtime=os.path.getmtime(d))
        try:
            r["run"] = json.load(open(os.path.join(d, "run.json")))
        except (OSError, ValueError):
            r["run"] = {}
        try:
            fr = json.load(open(stem + "-fastroute.json"))
            st = fr["stats"]
            r["router"] = {k: st.get(k) for k in ("unrouted", "vias", "trace_length_mm", "violations")}
            r["router"]["time_s"] = fr["time_s"]["total"]
            r["_unrouted_raw"] = [u for u in fr.get("unrouted", []) if "from_xy" in u and "to_xy" in u]
            r["unrouted"] = [dict(net=u["net"], a=f"{u['from'].get('component', '')}.{u['from'].get('pin', '')}",
                                  b=f"{u['to'].get('component', '')}.{u['to'].get('pin', '')}",
                                  why=(u.get("diagnosis") or {}).get("class", "")) for u in fr.get("unrouted", [])]
        except (OSError, ValueError, KeyError):
            pass
        try:
            txt = open(stem + "-drc.rpt").read()
            cnt = {}
            for kind, msg, rule in DRC_LINE.findall(txt):
                if kind in ("lib_footprint_issues",):
                    continue
                key = kind + (f" ({rule.strip()})" if rule else "")
                cnt[key] = cnt.get(key, 0) + 1
            r["drc"] = cnt
            r["drc_errors"] = sum(v for k, v in cnt.items() if not k.startswith("silk"))
        except OSError:
            pass
        r["marks"] = marks(stem, r)
        lines, _, _ = tail(os.path.join(d, "run.log"), 400)
        for l in lines:
            m = re.search(r"own route: (\d+) GND stitching vias, (\d+) vias total, unconnected (\d+)", l)
            if m:
                r["final"] = dict(stitch=int(m[1]), vias=int(m[2]), unconnected=int(m[3]))
        r["log"] = lines[-12:]
        dst = os.path.join(RUNS, name)
        r["svgs"] = []
        for layer in ("top", "bottom", "all"):
            src = f"{stem}-{layer}.svg"
            if os.path.exists(src):
                os.makedirs(dst, exist_ok=True)
                tgt = os.path.join(dst, layer + ".svg")
                if not os.path.exists(tgt) or os.path.getmtime(tgt) < os.path.getmtime(src):
                    import shutil
                    shutil.copy(src, tgt)
                r["svgs"].append(f"jobs/runs/{name}/{layer}.svg")
        # the final word on connectivity is KiCad's DRC, not the router's count
        if "drc" in r:
            r["unconnected"] = r["drc"].get("unconnected_items", 0)
        else:
            r["unconnected"] = r.get("final", {}).get("unconnected", r.get("router", {}).get("unrouted", 99))
        un = r["unconnected"]
        errs = r.get("drc_errors", 99)
        # one fixed ranking: clean first, then fewest unconnected, DRC errors, vias, track length
        r["score"] = [0 if (errs == 0 and un == 0) else 1, un, errs, r.get("final", {}).get("vias", 999),
                      round(r.get("router", {}).get("trace_length_mm", 9999))]
        r.pop("_unrouted_raw", None)
        out.append(r)
    out += bare_router_runs({os.path.join(JOB, "tmp", r["dir"].replace("$JOB/tmp/", "")) for r in out})
    out.sort(key=lambda r: -r["mtime"])
    if out:
        min(out, key=lambda r: r["score"])["best"] = True
    return out[:50]


def bare_router_runs(seen):
    """fastroute runs that weren't full board builds (tests, experiments): any other fastroute report."""
    out = []
    for f in glob.glob(os.path.join(JOB, "tmp", "**", "*.json"), recursive=True):
        if f.endswith("-fastroute.json") or "easyeda-cache" in f or time.time() - os.path.getmtime(f) > 6 * 3600:
            continue
        try:
            if os.path.getsize(f) > 5_000_000:
                continue
            fr = json.load(open(f))
            if not (isinstance(fr, dict) and "fastroute" in fr and "stats" in fr):
                continue
        except (OSError, ValueError):
            continue
        st = fr["stats"]
        stem = f[:-5]
        name = os.path.relpath(stem, os.path.join(JOB, "tmp")).replace("/", "_")
        lines, _, _ = tail(stem + ".log", 400)
        pre = next((l.split("WARN", 1)[1].strip() for l in lines if "pre-existing clearance" in l and "Board has" in l), "")
        r = dict(name=name, dir=os.path.dirname(f).replace(JOB, "$JOB"), mtime=os.path.getmtime(f),
                 run={"tool": "fastroute only (no KiCad build)", "env": {"input": os.path.basename(fr.get("input", ""))}},
                 router={k: st.get(k) for k in ("unrouted", "vias", "trace_length_mm", "violations")},
                 unrouted=[dict(net=u["net"], a=f"{u['from'].get('component', '')}.{u['from'].get('pin', '')}",
                                b=f"{u['to'].get('component', '')}.{u['to'].get('pin', '')}",
                                why=(u.get("diagnosis") or {}).get("class", "")) for u in fr.get("unrouted", [])],
                 svgs=[], marks=None, log=([pre] if pre else []) + [l for l in lines if "INFO" in l][-8:])
        r["router"]["time_s"] = fr["time_s"]["total"]
        r["unconnected"] = st.get("unrouted", 99)
        r["score"] = [1, r["unconnected"], st.get("violations", 99), st.get("vias", 999), round(st.get("trace_length_mm", 9999))]
        out.append(r)
    return out


def main():
    os.makedirs(OUT, exist_ok=True)
    while True:
        p = procs()
        state = dict(time=time.strftime("%H:%M:%S"), running=p, recent=recent({r["stdout"] for r in p}), runs=runs())
        tmp = os.path.join(OUT, "status.json.tmp")
        json.dump(state, open(tmp, "w"))
        os.replace(tmp, os.path.join(OUT, "status.json"))
        time.sleep(2)


if __name__ == "__main__":
    main()
