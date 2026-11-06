#!/usr/bin/env python3
"""Route a plan with an external Specctra router (fastroute / Freerouting), then the same finishing
and KiCad DRC as layout.py: pours, stitching, ground repair, fab outputs.

    source ~/opt/kienv.sh && BUILD_DIR=... PLACE_JSON=plan.json python3 ext_route.py fastroute|freerouting [seconds]
"""
import json, os, subprocess, sys, time
# fastroute routes every net; this script only prepares its input (net classes) and runs KiCad's
# pours, DRC and fab outputs.
import layout, pcbnew

tool, secs = sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 300
RUN = os.path.join(layout.BUILD, "run.json")         # what this run was, for the jobs page
_keys = ("PLACE_JSON", "FASTROUTE_ARGS")
json.dump({"tool": tool, "secs": secs, "start": time.time(), "env": {k: os.environ[k] for k in _keys if os.environ.get(k)}}, open(RUN, "w"))
spec = json.load(open(os.path.join(layout.BUILD, "parts-120v.json")))
stem = os.path.join(layout.BUILD, "coffee-controller-120v")
board = layout.build(spec, 120)                 # placement only
pcbnew.SaveBoard(stem + "-placed.kicad_pcb", board)
layout.write_rules(stem + "-placed.kicad_dru", spec)
board = pcbnew.LoadBoard(stem + "-placed.kicad_pcb")
dsn, ses = stem + ".dsn", stem + ".ses"
if not pcbnew.ExportSpecctraDSN(board, dsn):
    raise SystemExit("DSN export failed")
MAINS_CLASSES = layout.fix_dsn(dsn, spec)       # one class per mains net: width, 2.0 / 0.5 mm clearances


def lv_classes(path):
    """Split the default class: power at 0.4 mm, the USB-C fine-pitch nets at 0.2 mm, signals at 0.25 mm.
    No class narrower than 0.2 mm (JLC's minimum at 2 oz), and no neck-down below that."""
    import re as _re
    txt = open(path).read()
    m = _re.search(r'\(class kicad_default ""(.*?)\n      \(circuit', txt, _re.S)
    names = _re.findall(r'"[^"]*"|\S+', m.group(1))
    plain = lambda n: n.strip('"')
    power = [n for n in names if plain(n) in ("+3V3", "+5V", "+5V_IN", "VBUS", "GND")]
    fine = [n for n in names if plain(n) in ("USB_D+", "USB_D-", "CC1", "CC2")]
    rest = [n for n in names if n not in power and n not in fine]
    txt = txt[:m.start(1)] + " " + " ".join(rest) + txt[m.end(1):]
    blk = lambda name, nets, w: (f"    (class {name} {' '.join(nets)}\n      (circuit\n        (use_via Via[0-1]_600:300_um)\n      )\n"
                                 f"      (rule\n        (width {w})\n        (clearance 200.1)\n      )\n    )\n")
    k = txt.index("    (class kicad_default")
    txt = txt[:k] + blk("PWR", power, 400) + blk("FINE", fine, 200) + txt[k:]
    open(path, "w").write(txt)


lv_classes(dsn)


t0 = time.time()
if tool == "fastroute":
    exe = os.environ["FASTROUTE"]
    cmd = [exe, "--diagnose", "-de", dsn, "-do", ses, f"--max-time={secs}", "--report=" + stem + "-fastroute.json",
           "--no-neckdown-classes=" + ",".join(["kicad_default"] + ([] if os.environ.get("NECKDOWN_PWR") else ["PWR"]) + ["FINE"]
                                               + [c for c, n in MAINS_CLASSES if n not in ("AC_L", "HTR_A", "HTR_B")])]
    # NECKDOWN_PWR (plan "route", v31): power may narrow (to the 0.2 mm floor) right at fine-pitch pins,
    # e.g. the TMP117's 0.65 mm WSON, where a full 0.4 mm +3V3 track can't leave the pad.
    # live view on a free port (shown on the jobs page while the router runs)
    import socket
    _s = socket.socket(); _s.bind(("127.0.0.1", 0)); port = _s.getsockname()[1]; _s.close()
    cmd.append(f"--live={os.environ.get('FASTROUTE_LIVE', port)}")
    os.environ["FASTROUTE_LIVE_NO_OPEN"] = "1"
    cmd += os.environ.get("FASTROUTE_ARGS", "").split()
else:
    cmd = ["java", "-Xmx4g", "-Djava.awt.headless=true", "-jar", os.environ["FREEROUTING_JAR"], "--gui.enabled=false",
           "-de", dsn, "-do", ses, f"--router.job_timeout=00:{secs // 60:02d}:{secs % 60:02d}"]
if os.environ.get("SES_REUSE"):
    # re-finish an earlier fastroute session on the same placement (same pads and nets; the
    # import and the KiCad DRC below check it against the current rules like any fresh route)
    import shutil
    shutil.copy(os.environ["SES_REUSE"], ses)
    r = subprocess.CompletedProcess(cmd, 0, f"reused session {os.environ['SES_REUSE']}\n", "")
else:
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=secs + 120)
open(stem + f"-{tool}.log", "w").write(r.stdout + r.stderr)
print(f"{tool}: {time.time() - t0:.0f} s, exit {r.returncode}, session {'written' if os.path.exists(ses) else 'MISSING'}", flush=True)
b = pcbnew.LoadBoard(stem + "-placed.kicad_pcb")
n_t, n_v = layout.import_ses(b, ses)
stubs = layout.remove_stubs(b)
# the router narrows some tracks to 0.187 mm in tight spots; 0.2 mm is the floor (JLC at 2 oz).
# Widening by 6.5 um a side is then checked by the KiCad DRC like everything else.
thin = 0
for t in b.GetTracks():
    if t.Type() == pcbnew.PCB_TRACE_T and t.GetWidth() < pcbnew.FromMM(0.2):
        t.SetWidth(pcbnew.FromMM(0.2)); thin += 1
print(f"widened {thin} sub-0.2 mm segments to 0.2 mm", flush=True)
pcbnew.SaveBoard(stem + "-placed.kicad_pcb", b)
print(f"imported {n_t} segments, {n_v} vias ({stubs} stubs removed)", flush=True)
layout.finish_own(spec, stem)
_r = json.load(open(RUN)); _r["end"] = time.time(); json.dump(_r, open(RUN, "w"))
