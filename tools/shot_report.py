#!/usr/bin/env python3
"""Summarise shots in an emulator trace: truth vs engine, control quality."""
import csv, sys, math
rows = list(csv.DictReader(open(sys.argv[1])))
f = lambda r, k: float(r[k]) if r.get(k) not in (None, "") else math.nan
st = lambda r: int(f(r, "engine.state")) if not math.isnan(f(r, "engine.state")) else -1
shots, cur = [], None
for i, r in enumerate(rows):
    s = st(r)
    if s == 3 and cur is None: cur = [i, None]
    if cur and s not in (3,) and cur[1] is None: cur[1] = i
    if cur and cur[1] is not None:
        shots.append(cur); cur = None
for n, (a, b) in enumerate(shots, 1):
    seg = rows[a:b]; t0 = f(seg[0], "time")
    post = [r for r in rows[b:] if f(r, "time") < f(rows[b], "time") + 120]
    ext = [r for r in seg if f(r, "puck.flow") > 0.2]
    wt = [f(r, "boiler.water_temp") for r in ext]; bt = [f(r, "brew.water_temp") for r in ext if not math.isnan(f(r, "brew.water_temp"))]
    tp = [(f(r, "engine.pressure"), f(r, "engine.target_pressure")) for r in seg if not math.isnan(f(r, "engine.target_pressure"))]
    err = [abs(p - t) for p, t in tp if t > 3]
    # Overshoot only where the target is holding (ramps lag by design of the filter).
    hold = [r for i2, r in enumerate(seg[1:], 1) if not math.isnan(f(r, "engine.target_pressure")) and abs(f(r, "engine.target_pressure") - f(seg[i2-1], "engine.target_pressure")) < 1e-3]
    over = max((f(r, "engine.pressure") - f(r, "engine.target_pressure") for r in hold), default=0)
    lim_viol = 0
    first_drip = next((f(r, "time") - t0 for r in seg if f(r, "puck.flow") > 0.1), math.nan)
    settle = [r for r in rows[b:] if f(r, "time") < f(rows[b], "time") + 6]
    cupfinal = f(settle[-1], "cup.mass") if settle else math.nan
    print(f"shot {n}: {f(seg[-1],'time')-t0:5.1f}s  first drip {first_drip:4.1f}s  cup {cupfinal:5.1f}g  pf start {f(seg[0],'portafilter.temp'):5.1f}C")
    if wt: print(f"   water  (truth)  mean {sum(wt)/len(wt):5.1f}  min {min(wt):5.1f}  max {max(wt):5.1f}   [target {f(seg[0],'engine.setpoint'):.1f}]")
    if bt: print(f"   at puck(truth)  mean {sum(bt)/len(bt):5.1f}  min {min(bt):5.1f}  max {max(bt):5.1f}")
    hd = [f(r, "engine.heater_duty") for r in ext]
    if "engine.water" in rows[0]:
        e = [f(r, "engine.water") - f(r, "boiler.water_temp") for r in seg]
        print(f"   water observer error: mean {sum(e)/len(e):+.2f}  max |{max(abs(x) for x in e):.2f}|")
    hp = [f(r, "heater.power") for r in ext]
    if hd: print(f"   heater during extraction: duty cmd mean {sum(hd)/len(hd):.2f}, true power mean {sum(hp)/len(hp):.0f} W")
    print(f"   body max {max(f(r,'boiler.temp') for r in seg):5.1f}   water max in 2 min after: {max((f(r,'boiler.water_temp') for r in post), default=math.nan):5.1f}")
    if err: print(f"   pressure |err| mean {sum(err)/len(err):.2f} bar  max {max(err):.2f}   overshoot above target {over:+.2f} bar")
    # phase table
    ph = {}
    for r in seg:
        p = int(f(r, "engine.phase")); ph.setdefault(p, []).append(r)
    for p, rs in sorted(ph.items()):
        if p == 255: continue
        print(f"   phase {p}: {f(rs[-1],'time')-f(rs[0],'time'):4.1f}s  P {min(f(r,'hydraulics.pressure') for r in rs):4.1f}-{max(f(r,'hydraulics.pressure') for r in rs):4.1f}  est flow {sum(f(r,'engine.flow') for r in rs)/len(rs):4.2f} true pump {sum(f(r,'pump.flow') for r in rs)/len(rs):4.2f}  wet {f(rs[-1],'puck.wet'):.2f}")
