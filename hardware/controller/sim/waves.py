#!/usr/bin/env python3
"""Export representative waveforms (120 V build) for the waveform page: sim/waves.json.

    python3 sim/waves.py
"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import run

captured = {}
_wave = run.wave
def grab(netlist, vectors, tstop):
    w = _wave(netlist, vectors, tstop)
    captured["last"] = w
    return w
run.wave = grab


def thin(t, ys, n=900, t0=None, t1=None):
    """Down-sample for the browser: keep an even spread plus every point on a fast edge."""
    idx = [i for i, x in enumerate(t) if (t0 is None or x >= t0) and (t1 is None or x <= t1)]
    if len(idx) <= n:
        keep = idx
    else:
        step = len(idx) / n
        keep = {idx[int(k * step)] for k in range(n)}
        for y in ys:
            seg = [y[i] for i in idx]
            thr = 0.02 * (max(seg) - min(seg) + 1e-12)
            last = idx[0]
            for i in idx[1:]:
                if abs(y[i] - y[last]) > thr:   # moved more than 2 % of the range since the last kept point
                    keep.add(i)
                    last = i
        keep = sorted(keep)
    return [t[i] for i in keep], [[y[i] for i in keep] for y in ys]


out = {}

# 1. pump turn-off at 120 V: voltage across the triac, with and without the snubber
ps = []
for with_rc, label in ((False, "No snubber"), (True, "With 100 Ω + 10 nF snubber")):
    run.snubber(with_rc, vrms=120.0)
    w = captured["last"]
    tt, (v,) = thin(w["time"], [w["v(mt2,mt1)"]], n=1500, t0=0.0395, t1=0.0475)
    ps.append({"name": label, "x": [(x - 0.0395) * 1e3 for x in tt], "y": v})
out["snubber"] = {"series": ps, "unit": "V"}

# 2. valve turn-off at 132 V (+10 %): with and without the 100 R + 10 nF snubber
vs = []
for with_rc, label in ((False, "No snubber"), (True, "With 100 Ω + 10 nF snubber")):
    run.valve(with_rc, vrms=132.0, csn="10n", ih=0.010)
    w = captured["last"]
    tt, (v,) = thin(w["time"], [w["v(mt2,mt1)"]], n=1500, t0=0.0440, t1=0.0600)
    vs.append({"name": label, "x": [(x - 0.0440) * 1e3 for x in tt], "y": v})
out["valve"] = {"series": vs, "unit": "V"}

# 2b. thermistor: ADC voltage vs boiler temperature
temps = [20 + i for i in range(0, 141)]
out["ntc"] = {"series": [{"name": "GPIO1 voltage (100k B3950, 8.2k to 3.3 V)", "x": temps, "y": [run.ntc_v(t) for t in temps]}], "unit": "V"}

# 3. rails during Wi-Fi bursts: raw transient
def rails_wave(vpsu):
    net = f"""* rails
.model SS14 D(IS=5e-6 N=1.1 RS=0.05)
Vpsu psu 0 DC {vpsu}
Rpsu psu v5 0.15
C1 v5 0 22u
Rl5 v5 0 300
Bldo v33r 0 V = min(3.3, v(v5) - (1.0 + 0.4*i(Vsense)))
Rout v33r v33s 0.02
Vsense v33s v33 0
Bin v5 0 I = i(Vsense)
C3 v33 0 22u
C4 v33 0 10u
Iesp v33 0 PULSE(0.08 0.5 2m 2u 2u 1m 5m)
.tran 2u 14m 0 2u
.control
run
@WRDATA@
.endc
.end
"""
    return run.wave(net, ["v(v5)", "v(v33)"], 0.014)
r33, r5 = [], []
for vpsu, label in ((5.0, "PSU 5.00 V (nominal)"), (4.75, "PSU 4.75 V (−5 %)")):
    w = rails_wave(vpsu)
    tt, (a, b) = thin(w["time"], [w["v(v5)"], w["v(v33)"]], n=1200, t0=0.001)
    r5.append({"name": label, "x": [x * 1e3 for x in tt], "y": a})
    r33.append({"name": label, "x": [x * 1e3 for x in tt], "y": b})
out["rail5"] = {"series": r5, "unit": "V"}
out["rail33"] = {"series": r33, "unit": "V"}

# 4. pressure input frequency response
net = """* pressure filter
Vt t 0 DC 0 AC 1
Rs t s 100
R1 s a 10k
R2 a 0 20k
C1 a 0 100n
Rin a 0 1meg
.control
ac dec 40 1 100k
let mag = vdb(a) + 3.5218
wrdata @OUT@ mag
.endc
.end
"""
import tempfile, subprocess
with tempfile.TemporaryDirectory() as d:
    f = os.path.join(d, "ac.dat")
    run.spice(net.replace("@OUT@", f))
    rows = [list(map(float, l.split())) for l in open(f) if l.strip()]
out["pressure"] = {"series": [{"name": "Gain relative to DC", "x": [r[0] for r in rows], "y": [r[1] for r in rows]}], "unit": "dB", "logx": True}

# 5. opto drive current at the corners (from the GPIO)
bars = []
for vg in (3.3, 3.0):
    for ir, lab in (("ir_typ", "typ LED"), ("ir_max", "max-Vf LED")):
        i, _ = run.drive(vg, ir)
        bars.append({"name": f"VDD {vg:.1f} V, {lab}", "value": round(i, 2)})
out["drive"] = bars

json.dump(out, open(os.path.join(os.path.dirname(__file__), "waves.json"), "w"))
print("wrote sim/waves.json:", ", ".join(out))
