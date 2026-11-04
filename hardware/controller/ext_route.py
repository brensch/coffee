#!/usr/bin/env python3
"""Route a plan with an external Specctra router (fastroute / Freerouting), then the same finishing
and KiCad DRC as layout.py: pours, stitching, ground repair, fab outputs.

    source ~/opt/kienv.sh && BUILD_DIR=... PLACE_JSON=plan.json python3 ext_route.py fastroute|freerouting [seconds]
"""
import json, os, subprocess, sys, time
# fastroute routes every net (mains, heater, signals, ground); this script only prepares its input
# (routing_pads: the triac pins as Specctra can express them) and runs KiCad's pours, DRC and fab outputs.
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
layout.fix_dsn(dsn, spec)                       # mains classes: 2.0 mm between, 0.5 mm within same-potential groups


def lv_classes(path):
    """Split the default class: power at 0.4 mm, the USB-C fine-pitch nets at 0.2 mm, signals at 0.25 mm.
    No class narrower than 0.2 mm (JLC's minimum at 2 oz), and no neck-down below that."""
    import re as _re
    txt = open(path).read()
    m = _re.search(r'\(class kicad_default ""(.*?)\n      \(circuit', txt, _re.S)
    names = _re.findall(r'"[^"]*"|\S+', m.group(1))
    plain = lambda n: n.strip('"')
    power = [n for n in names if plain(n) in ("+3V3", "+5V", "VBUS", "GND")]
    fine = [n for n in names if plain(n) in ("USB_D+", "USB_D-", "CC1", "CC2")]
    rest = [n for n in names if n not in power and n not in fine]
    txt = txt[:m.start(1)] + " " + " ".join(rest) + txt[m.end(1):]
    blk = lambda name, nets, w: (f"    (class {name} {' '.join(nets)}\n      (circuit\n        (use_via Via[0-1]_600:300_um)\n      )\n"
                                 f"      (rule\n        (width {w})\n        (clearance 200.1)\n      )\n    )\n")
    k = txt.index("    (class kicad_default")
    txt = txt[:k] + blk("PWR", power, 400) + blk("FINE", fine, 200) + txt[k:]
    open(path, "w").write(txt)


lv_classes(dsn)


def routing_pads(path):
    """The router's model of the triac pins (KiCad keeps the real pads). At 2.3-2.54 mm pitch no
    track can leave a real triac pad and stay 2.0 mm from the next pin, so Specctra sees every exit
    as illegal ("pre-existing, unfixable" violations, connections left unrouted). Here the triac pins
    become 0.2 mm points: 2.54 mm pitch (TO-220) then leaves 2.0 mm beside a 0.5 mm track, 2.3 mm
    (SOT-223, the valve triac) beside a 0.3 mm one (valve 0.07 A). The SOT-223 tab keeps its real
    shape. The real pads overlap the track ends; KiCad's DRC (0.5 mm inside the part courtyard,
    2.0 mm outside) checks the result."""
    import re as _re
    txt = open(path).read()

    def points(blk, ps="Route[A]Point_200_um"):
        return _re.sub(r"\(pin \S+ (\d) ", f"(pin {ps} \\1 ", blk)
    img = txt.index('(image "TO-220-3_Vertical_Drill1.2"')
    end = txt.index("\n    (", img + 10) + 1
    txt = txt[:img] + points(txt[img:end]) + txt[end:]
    # the valve triac gets its own copy of the SOT-223 image (the LDO keeps the real one)
    img = txt.index('(image "SOT-223-3_TabPin2"')
    end = txt.index("\n    (", img + 10) + 1
    q3 = points(txt[img:end], "Route[T]Point_200_um").replace('"SOT-223-3_TabPin2"', '"SOT-223-3_Q3_route"', 1)
    # pin 2 (AC_L, joined at the tab instead) as a point would let tracks pass its real 1.5 x 2 mm pad
    # just outside the courtyard: keep out its 2.0 mm halo beyond the courtyard (x < -4.4 mm)
    q3 = q3.replace("\n", "\n      (keepout \"\" (polygon signal 0  -5650 -2250  -4400 -2250  -4400 2250  -5650 2250  -5650 -2250))\n", 1)
    txt = txt[:end] + q3 + txt[end:]
    m = _re.search(r"\n\s*\(place Q3 [^\n]*\)", txt)
    txt = txt[:m.start()] + txt[m.end():]
    k = txt.index('    (component "SOT-223-3_TabPin2"')
    txt = txt[:k] + f'    (component "SOT-223-3_Q3_route"\n      {m.group(0).strip()}\n    )\n' + txt[k:]
    k = txt.index("    (padstack ")
    txt = txt[:k] + ("    (padstack Route[A]Point_200_um\n      (shape (circle F.Cu 200))\n"
                     "      (shape (circle B.Cu 200))\n      (attach off)\n    )\n"
                     "    (padstack Route[T]Point_200_um\n      (shape (circle F.Cu 200))\n      (attach off)\n    )\n") + txt[k:]
    # mains signal widths: 0.5 mm, valve nets 0.3 mm
    def width(cls_net, w):
        nonlocal txt
        m = _re.search(r"\(class (M\d+)[^\n]*\b" + cls_net + r"\b.*?\(width [\d.]+\)", txt, _re.S)
        txt = txt[:m.start()] + _re.sub(r"\(width [\d.]+\)", f"(width {w})", m.group(0)) + txt[m.end():]
    txt = txt.replace("(width 800.0)", "(width 500.0)")
    width("VALVE_L", "300.0")
    open(path, "w").write(txt)


routing_pads(dsn)
t0 = time.time()
if tool == "fastroute":
    exe = os.environ["FASTROUTE"]
    cmd = [exe, "--diagnose", "-de", dsn, "-do", ses, f"--max-time={secs}", "--report=" + stem + "-fastroute.json",
           "--no-neckdown-classes=kicad_default,PWR,FINE"]
    # live view on a free port (shown on the jobs page while the router runs)
    import socket
    _s = socket.socket(); _s.bind(("127.0.0.1", 0)); port = _s.getsockname()[1]; _s.close()
    cmd.append(f"--live={os.environ.get('FASTROUTE_LIVE', port)}")
    os.environ["FASTROUTE_LIVE_NO_OPEN"] = "1"
    cmd += os.environ.get("FASTROUTE_ARGS", "").split()
else:
    cmd = ["java", "-Xmx4g", "-Djava.awt.headless=true", "-jar", os.environ["FREEROUTING_JAR"], "--gui.enabled=false",
           "-de", dsn, "-do", ses, f"--router.job_timeout=00:{secs // 60:02d}:{secs % 60:02d}"]
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
