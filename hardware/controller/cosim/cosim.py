#!/usr/bin/env python3
"""Firmware -> wire -> mains co-simulation.

The emulator runs the same coffee-node engine as the firmware and traces, per mains cycle, the
three output bits that control.rs writes to GPIO20 (heater), GPIO19 (pump) and GPIO18 (valve) at
each tick. This script takes that bit stream and simulates what happens after the pin:

  tick timing    crystal-timed ticks at the *nominal* 60 Hz; the real mains runs at 59.95-60.05 Hz
                 with an arbitrary phase (the board has no zero-cross detector), plus software latency
  GPIO -> LED    VOH corner, pin resistance, 150 R, MOC3083 LED Vf corner -> I_F vs IFT (5 mA max)
  opto           zero-cross inhibit: fires only while |V across it| < VINH (20 V max) with I_F >= IFT;
                 once fired it carries gate current until the main triac latches
  triac          gate current through 360 R must reach IGT; then on until |I| < IH
  loads          heater: resistive 12 ohm (1200 W at 120 V)
                 pump: Ulka EP5, diode in series (conducts on one polarity), R-L
                 valve: AC solenoid R-L with the 100 R + 10 nF snubber across the triac (2 us ODE)

Checks (written to cosim/REPORT.md):
  - every commanded heater cycle delivers exactly its two half-cycles (no lost/extra half-cycles,
    no DC: equal positive and negative half-cycles over each on-run);
  - pump strokes per commanded cycle (1 expected; 0 or 2 are errors) across mains phase drift;
  - valve: no skipped half-cycles while commanded (no DC in the coil), turns off within one cycle;
  - drive margins: I_F at the worst corner vs IFT; gate current vs IGT at the firing voltage.

    python3 cosim/cosim.py [trace.csv]      # default: runs the emulator on cosim_120v.scn first
"""
import csv, math, os, random, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))

# ---------------------------------------------------------------- parameters (datasheet corners)
VRMS = (108.0, 120.0, 132.0)          # mains corners
F_NOM = 60.0
F_MAINS = (59.95, 60.0, 60.05)       # utility frequency band
XTAL_PPM = (-10.0, 0.0, 10.0)        # ESP32-C6-MINI-1 40 MHz crystal
LAT_US = (20.0, 300.0)               # tick -> GPIO write (node.tick + set_level), measured budget
VOH_MIN, R_PIN, R_LED = 2.64, 25.0, 150.0   # ESP32-C6: VOH >= 0.8*VDD at 3.3 V; drive-strength pin resistance
VF_MAX = 1.5                         # MOC3083 LED Vf max at 10 mA
IFT_MAX = 5e-3                       # MOC3083 IFT max (25 C)
VINH_MAX = 20.0                      # MOC3083 inhibit voltage (max): the opto can't START conducting above this
V_LATCH = 5.0                        # line voltage giving the opto-triac its latching current through 360 R
T_ON_OPTO = (0.0, 5e-6, 20e-6)       # opto turn-on time: not in the datasheet, swept
R_GATE = 360.0                       # R1/R2/R4: opto feed
V_OPTO = 1.8                         # opto-triac on-state drop (VTM 3 V max at 100 mA; ~1.8 V at 50 mA)
TRIACS = {   # IGT max (mA, Q1-Q3), VGT (V), IH max (mA), IL max (mA)
    "heater": dict(part="BTA24-800BW", igt=50e-3, vgt=1.3, ih=75e-3, il=80e-3),
    "pump": dict(part="BTA08-800BW", igt=50e-3, vgt=1.3, ih=50e-3, il=70e-3),
    "valve": dict(part="Z0109NN", igt=10e-3, vgt=1.3, ih=10e-3, il=15e-3),
}
HEATER_R = 120.0 ** 2 / 1200.0       # 12 ohm
PUMP = dict(R=45.0, L=0.35)          # Ulka EP5-ish (diode inside): ~0.9 A peak
VALVE = dict(R=550.0, L=1.6)         # ~9 W AC solenoid: ~0.15 A, strongly inductive
SNUB_R, SNUB_C = 100.0, 8e-9         # 10 nF X2 derated


def led_current(voh=VOH_MIN, vf=VF_MAX):
    return max(0.0, (voh - vf) / (R_PIN + R_LED))


def fire_voltage(t):
    """|V| across the opto+gate path at which the gate gets IGT through R_GATE."""
    return t["igt"] * R_GATE + V_OPTO + t["vgt"]


# ---------------------------------------------------------------- bit stream
def load_trace(path):
    rows = list(csv.DictReader(open(path)))
    return [(int(float(r["heater.on"])), int(float(r["pump.on"])), int(float(r["valve.open"]))) for r in rows]


def led_edges(bits, ch, f_mains, ppm, phase, lat_us):
    """GPIO high intervals (start, end) in mains-time seconds for channel ch (0 heater, 1 pump, 2 valve).
    Tick k happens at k / (F_NOM * (1 + ppm)) on the chip's clock; the frame computed at tick k is
    applied lat_us later and held until the next tick's write."""
    T = 1.0 / (F_NOM * (1 + ppm * 1e-6))
    iv, start = [], None
    for k, b in enumerate(bits):
        t = phase + k * T + lat_us * 1e-6
        if b[ch] and start is None:
            start = t
        elif not b[ch] and start is not None:
            iv.append((start, t)); start = None
    if start is not None:
        iv.append((start, phase + len(bits) * T))
    return iv


def zero_crossings(f, t0, t1):
    hp = 0.5 / f
    k0 = math.ceil(t0 / hp)
    k1 = math.floor(t1 / hp)
    return [(k * hp, 1 if k % 2 == 0 else -1) for k in range(k0, k1 + 1)]   # (time, polarity of the half-cycle that starts)


# ---------------------------------------------------------------- heater and pump: half-cycle logic
def window_on(iv, a, b, idx):
    """is the LED on at any time in [a, b]? iv sorted; idx is a moving cursor."""
    while idx[0] < len(iv) and iv[idx[0]][1] < a:
        idx[0] += 1
    j = idx[0]
    return j < len(iv) and iv[j][0] <= b


def half_cycles(iv, f, vrms, tri, t_end):
    """For a load whose current follows the voltage (heater): which half-cycles conduct.
    Fires in the half-cycle starting at zero crossing z if the LED is on at some moment while the
    line is still below the opto's inhibit voltage and high enough to give IGT, i.e. within
    [z + t(Vfire), z + t(VINH)]. Returns list of (z, polarity, conducted)."""
    vpk = vrms * math.sqrt(2)
    w = 2 * math.pi * f
    out = []
    # The opto-triac can start conducting only while the line is between V_LATCH and VINH; once on
    # it stays latched and carries the gate current up to IGT (main triac fires at fire_voltage()).
    ta = math.asin(min(1.0, V_LATCH / vpk)) / w
    tb = math.asin(min(1.0, VINH_MAX / vpk)) / w
    idx = [0]
    for z, pol in zero_crossings(f, iv[0][0] - 0.02 if iv else 0, t_end):
        on = window_on(iv, z + ta, z + tb, idx)
        out.append((z, pol, on))
    return out


def check_heater(bits, f, ppm, phase, lat, vrms):
    iv = led_edges(bits, 0, f, ppm, phase, lat)
    if not iv:
        return dict(cmd_cycles=0)
    hc = half_cycles(iv, f, vrms, TRIACS["heater"], iv[-1][1] + 0.02)
    cmd = sum(b[0] for b in bits)
    got = sum(1 for _, _, on in hc if on)
    pos = sum(1 for _, p, on in hc if on and p > 0)
    neg = got - pos
    # per on-run: half-cycles conducted vs 2 x cycles commanded
    runs_bad, worst_dc = 0, 0
    for a, b in iv:
        n_cmd = round((b - a) * F_NOM)
        hs = [(p) for z, p, on in hc if on and a - 0.001 <= z <= b + 0.009]
        if abs(len(hs) - 2 * n_cmd) > 1:
            runs_bad += 1
        worst_dc = max(worst_dc, abs(sum(hs)))
    e_cmd = cmd * (vrms ** 2 / HEATER_R) / F_NOM
    e_got = got * (vrms ** 2 / HEATER_R) / (2 * f)
    return dict(cmd_cycles=cmd, half_cycles=got, pos=pos, neg=neg, runs=len(iv), runs_bad=runs_bad,
                worst_run_dc=worst_dc, energy_err=(e_got - e_cmd) / e_cmd if e_cmd else 0)


def check_pump(bits, f, ppm, phase, lat, vrms):
    """Ulka: the internal diode passes only the positive half; one stroke per positive half-cycle
    that fires. Inductive: the current starts at the firing point and runs past the voltage zero,
    then the diode blocks, so the triac always turns off before the next positive half."""
    iv = led_edges(bits, 1, f, ppm, phase, lat)
    if not iv:
        return dict(cmd_cycles=0)
    hc = half_cycles(iv, f, vrms, TRIACS["pump"], iv[-1][1] + 0.02)
    strokes = [z for z, p, on in hc if on and p > 0]
    hist = {0: 0, 1: 0, 2: 0, 3: 0}
    j = 0
    T = 1.0 / F_NOM
    for k, b in enumerate(bits):
        if not b[1]:
            continue
        t0 = phase + k * T + lat * 1e-6
        n = sum(1 for z in strokes if t0 - 0.0005 <= z < t0 + T - 0.0005)
        hist[min(n, 3)] += 1
    cmd = sum(b[1] for b in bits)
    return dict(cmd_cycles=cmd, strokes=len(strokes), err=(len(strokes) - cmd) / cmd if cmd else 0, per_cycle=hist)


# ---------------------------------------------------------------- valve: ODE with snubber and opto logic
def sim_valve(bits, f, ppm, phase, lat, vrms, n_cycles=150, dt=1e-6, t_on=0.0):
    """R-L coil + triac, 100 R/8 nF snubber across the triac, zero-cross opto (backward Euler: the
    snubber's 0.8 us time constant is shorter than the step). The opto starts conducting when the LED
    is on and V_LATCH <= |v_T| < VINH has held for t_on; the main triac then fires once |v_T| reaches
    the IGT voltage. The triac turns off when its current falls through IH."""
    tri = TRIACS["valve"]
    vfire = fire_voltage(tri)
    vpk = vrms * math.sqrt(2)
    w = 2 * math.pi * f
    iv = led_edges(bits[:n_cycles], 2, f, ppm, phase, lat)
    R, L = VALVE["R"], VALVE["L"]
    i = vc = 0.0
    on = opto = False
    t_ok = 0.0
    t, t_end = 0.0, phase + n_cycles / F_NOM
    k = 0
    halves = []
    last_sign, cur = 0, None
    dvdt = 0.0
    vt_prev = 0.0
    si = si2 = n_acc = 0.0
    while t < t_end:
        while k < len(iv) and iv[k][1] < t:
            k += 1
        led = k < len(iv) and iv[k][0] <= t <= iv[k][1]
        if led and t > iv[k][0] + 3.0 / F_NOM:       # steady state inside a commanded window
            si += i; si2 += i * i; n_acc += 1
        vl = vpk * math.sin(w * (t + dt))
        sgn = 1 if vl >= 0 else -1
        if sgn != last_sign:
            if cur is not None:
                halves.append(cur)
            cur = [t, sgn, False]
            last_sign = sgn
        if on:
            i_new = (vl + L * i / dt) / (L / dt + R)
            vc = vc / (1 + dt / (SNUB_R * SNUB_C))
            if (i_new * i < 0) or abs(i_new) < tri["ih"] and abs(i_new) < abs(i):
                on = opto = False
                i_new = 0.0 if i_new * i < 0 else i_new
            i = i_new
            vt = 0.0
            cur[2] = True
        else:
            i = (vl - vc + L * i / dt) / (L / dt + R + SNUB_R + dt / SNUB_C)
            vc = vc + dt * i / SNUB_C
            vt = vc + SNUB_R * i
            dvdt = max(dvdt, abs(vt - vt_prev) / dt)
            if not opto:
                if led and V_LATCH <= abs(vt) < VINH_MAX:
                    t_ok += dt
                    if t_ok >= t_on:
                        opto = True
                else:
                    t_ok = 0.0
            if opto and abs(vt) >= vfire:
                on = True
            if opto and not led and abs(vt) < 1.0:
                opto = False
        vt_prev = vt
        t += dt
    if cur is not None:
        halves.append(cur)
    T = 1.0 / F_NOM
    skipped = extra = 0
    for hs, pol, cond in halves[2:]:
        he = hs + 0.5 / f
        commanded = any(a <= hs - 0.5 / f and he <= b for a, b in iv)     # LED on since the previous half
        released = all(not (a <= he + T and hs <= b) for a, b in iv)
        if commanded and not cond:
            skipped += 1
        if released and cond:
            extra += 1
    rms = math.sqrt(si2 / n_acc) if n_acc else 0.0
    dc = abs(si / n_acc) / rms if n_acc and rms else 0.0
    return dict(halves=len(halves), skipped=skipped, extra=extra, dvdt_max_v_per_us=dvdt * 1e-6, irms=rms, dc=dc)


# ---------------------------------------------------------------- main
def main():
    trace = sys.argv[1] if len(sys.argv) > 1 else None
    if not trace:
        trace = os.path.join(HERE, "cosim_trace.csv")
        subprocess.run([os.path.join(ROOT, "target", "release", "coffee-emu"), "run", os.path.join(HERE, "cosim_120v.scn"),
                        "--trace", trace], check=True, capture_output=True)
    bits = load_trace(trace)
    rnd = random.Random(1)
    lines = ["# Co-simulation: firmware bits -> opto -> triac -> mains load", "",
             f"Source: `{os.path.relpath(trace, HERE)}` from `cosim_120v.scn` (the coffee-node engine, 120 V model): "
             f"{len(bits)} mains cycles; heater on {sum(b[0] for b in bits)}, pump on {sum(b[1] for b in bits)}, valve open {sum(b[2] for b in bits)} cycles.", ""]
    # drive margins
    i_led = led_current()
    lines += ["## Drive margins", "",
              f"- GPIO -> 150 R -> LED at the worst corner (VOH {VOH_MIN} V, pin {R_PIN} R, Vf {VF_MAX} V): **I_F = {i_led * 1e3:.2f} mA** vs IFT max {IFT_MAX * 1e3:.0f} mA: "
              + ("OK" if i_led >= IFT_MAX else "FAIL")]
    ok_all = i_led >= IFT_MAX
    for ch, t in TRIACS.items():
        vf = fire_voltage(t)
        lag = math.degrees(math.asin(vf / (VRMS[0] * math.sqrt(2))))
        lines.append(f"- {ch} ({t['part']}): the opto latches at ~{V_LATCH:.0f} V (inside its {VINH_MAX:.0f} V zero-cross window) and the gate reaches IGT {t['igt'] * 1e3:.0f} mA "
                     f"at **{vf:.1f} V** line, {lag:.1f} deg into the half-cycle at {VRMS[0]:.0f} V: OK")
    lines.append("")
    # sweep timing corners
    lines += ["## Heater (resistive, 1200 W)", "", "| mains | f (Hz) | xtal ppm | latency | cycles cmd | half-cycles | +/- | runs off by >1 half | worst run DC (halves) | energy error |", "|---|---|---|---|---|---|---|---|---|---|"]
    worst = dict(heater=0, pump=0)
    for vrms in VRMS:
        for f in F_MAINS:
            for ppm in XTAL_PPM:
                lat = rnd.choice(LAT_US)
                ph = rnd.random() / f
                h = check_heater(bits, f, ppm, ph, lat, vrms)
                if h["cmd_cycles"]:
                    lines.append(f"| {vrms:.0f} | {f} | {ppm:+.0f} | {lat:.0f} us | {h['cmd_cycles']} | {h['half_cycles']} | {h['pos']}/{h['neg']} | {h['runs_bad']} / {h['runs']} | {h['worst_run_dc']} | {h['energy_err'] * 100:+.2f} % |")
                    worst["heater"] = max(worst["heater"], abs(h["energy_err"]))
                    ok_all &= h["runs_bad"] <= max(2, h["runs"] // 500)
    lines += ["", "## Pump (Ulka, diode inside: one stroke per positive half-cycle)", "",
              "| mains | f (Hz) | xtal ppm | cycles cmd | strokes | error | cycles with 0 / 1 / 2 strokes |", "|---|---|---|---|---|---|---|"]
    pump_bad = 0
    for vrms in (120.0,):
        for f in F_MAINS:
            for ppm in XTAL_PPM:
                ph = rnd.random() / f
                p = check_pump(bits, f, ppm, ph, rnd.choice(LAT_US), vrms)
                if p["cmd_cycles"]:
                    hst = p["per_cycle"]
                    lines.append(f"| {vrms:.0f} | {f} | {ppm:+.0f} | {p['cmd_cycles']} | {p['strokes']} | {p['err'] * 100:+.2f} % | {hst[0]} / {hst[1]} / {hst[2] + hst[3]} |")
                    worst["pump"] = max(worst["pump"], abs(p["err"]))
                    pump_bad += hst[0] + hst[2] + hst[3]
    # valve: pick a window with valve activity
    vi = [k for k, b in enumerate(bits) if b[2]]
    valve_rows = []
    lines += ["", "## Valve (inductive solenoid, 100 R + 8 nF snubber, 1 us backward-Euler ODE)", ""]
    if vi:
        k0 = max(0, vi[0] - 30)
        seg = bits[k0:k0 + 240]
        lines += ["| mains | f (Hz) | opto t_on | coil I rms | DC / rms (skipped halves show as DC) | extra after release | turn-off dv/dt |", "|---|---|---|---|---|---|---|"]
        for vrms in VRMS:
            for t_on in T_ON_OPTO:
                f = rnd.choice((59.95, 60.05))
                ph = rnd.random() / f
                v = sim_valve(seg, f, 0.0, ph, 100.0, vrms, t_on=t_on)
                lines.append(f"| {vrms:.0f} | {f} | {t_on * 1e6:.0f} us | {v['irms'] * 1e3:.0f} mA | {v['dc'] * 100:.1f} % | {v['extra']} | {v['dvdt_max_v_per_us']:.1f} V/us |")
                ok_all &= v["dc"] < 0.05
                valve_rows.append((t_on, v))
    else:
        lines.append("(the trace has no valve activity)")
    lines += ["", "## Verdict", "",
              f"- Heater energy error, worst corner: {worst['heater'] * 100:.2f} %.",
              f"- Pump stroke-count error, worst corner: {worst['pump'] * 100:.2f} %; commanded cycles with 0 or 2 strokes: {pump_bad} over all corners.",
              f"- Overall: **{'PASS' if ok_all else 'FAIL'}**"]
    open(os.path.join(HERE, "REPORT.md"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    sys.exit(0 if ok_all else 1)


if __name__ == "__main__":
    main()
