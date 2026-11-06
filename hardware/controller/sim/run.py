#!/usr/bin/env python3
"""Circuit simulations for the controller board (ngspice), with tolerance corners.

    python3 sim/run.py            # writes sim/REPORT.md, exits 1 if any check fails

Each block mirrors controller.py's values. Models are behavioural (diodes fitted to
datasheet Vf, opto CTR as a current-controlled source, regulator dropout as a curve),
so the point is margins at the corners, not exact waveforms.
"""
import math, os, re, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
NG = os.environ.get("NGSPICE", os.path.expanduser("~/opt/ngspice-root/usr/bin/ngspice"))
ENV = dict(os.environ, LD_LIBRARY_PATH=os.path.expanduser("~/opt/ngspice-root/usr/lib/x86_64-linux-gnu"))
results = []   # (block, case, value, limit, ok)


def spice(netlist):
    with tempfile.NamedTemporaryFile("w", suffix=".cir", delete=False) as f:
        f.write(netlist)
    out = subprocess.run([NG, "-b", f.name], capture_output=True, text=True, env=ENV).stdout
    os.remove(f.name)
    vals = dict(re.findall(r"^\s*(\w+)\s*=\s*([-+0-9.eE]+)", out, re.M))
    return {k.lower(): float(v) for k, v in vals.items()}, out


def wave(netlist, vectors, tstop):
    """Run a transient and return {name: [values]} plus 'time' (via wrdata)."""
    with tempfile.TemporaryDirectory() as d:
        dat = os.path.join(d, "w.dat")
        net = netlist.replace("@WRDATA@", f"wrdata {dat} " + " ".join(vectors))
        spice(net)
        rows = [list(map(float, l.split())) for l in open(dat) if l.strip()]
    out = {"time": [r[0] for r in rows]}
    for i, name in enumerate(vectors):
        out[name] = [r[2 * i + 1] for r in rows]
    return out


def check(block, case, value, limit, ok, unit=""):
    results.append((block, case, f"{value:.3g} {unit}".strip(), limit, ok))


# --------------------------------------------------------------- 1. output stage (GPIO -> AO3400A -> module input)
# AO3400A (datasheet): Vth 0.65 / 1.05 / 1.45 V min/typ/max; Rds(on) at Vgs 2.5 V: 24 typ, 48 max mOhm.
# Level-1 fit: 48 mOhm at 2.5 V with typical Vth, 66 mOhm with max Vth (worse than the datasheet max).
NMOS = {"vth_typ": (1.05, 14.4), "vth_max": (1.45, 14.4)}   # VTO, KP (A/V^2): Rds(2.5 V) = 1/(KP (2.5 - VTO))
def output(vgpio, fet, v5, load, rpd="3.3k", rpu=None, tstop="20m"):
    """GPIO (25 ohm) -> gate, rpd to GND, optional internal pull-up rpu (GPIO in reset = high-Z).
    load: 'ssr_sch' (Schneider SSP1A125BDT heater SSR: 3-32 V, ~10 mA),
          'ssr_board' (1-channel Omron G3MB-202P board, the pump/valve modules: G3MB-202P DC5 input ~250 ohm
          +-20 % plus the board's LED, modelled as 200 ohm; resistive, no coil, so no flyback diode since v23).
          All loads have a 1 uH cable."""
    vto, kp = NMOS[fet]
    loads = {
        "ssr_sch": "Lcab v5 m 1u\nBssr m out I = min(0.010, max(0, (v(m)-v(out)-1.2)/150))",
        "ssr_board": "Lcab v5 m 1u\nRin m out 200",
        # v32: MOC3083 IR LED behind 330 R (+1 %): onsemi Vf 1.15 typ / 1.5 max at 10 mA. DIR fitted to 1.5 V
        "opto_led": "Rled v5 a 333.3\nDled a out DIR",
    }
    drive = f"Vg g 0 PULSE(0 {vgpio} 1m 1u 1u 8m 20m)\nRgpio g pin 25" if rpu is None else f"Rpu v33 pin {rpu}\nVv33 v33 0 3.3"
    net = f"""* output stage
.model N3400 NMOS(LEVEL=1 VTO={vto} KP={kp} CGSO=1.5n CGDO=0.3n)
.model D4148 D(IS=2.5n N=1.75 RS=0.6 BV=75 CJO=2p TT=4n)
.model DRED D(IS=1e-19 N=1.8 RS=8)
.model DIR D(IS=1e-14 N=1.9 RS=2)
V5 v5 0 DC {v5}
{drive}
Rgs pin gate 0.01
Rpd gate 0 {rpd}
M1 out gate 0 0 N3400 L=1u W=1u
Cout out 0 120p
Vimeas v5 v5m 0
{loads[load].replace("v5 ", "v5m ", 1)}
.tran 2u {tstop} 0 2u
.control
run
let vin = v(v5) - v(out)
meas tran vgate_on FIND v(gate) AT=5m
meas tran vin_on FIND vin AT=8.5m
meas tran vds_on FIND v(out) AT=8.5m
meas tran vdrain_pk MAX v(out) from=9m to=20m
meas tran vin_off FIND vin AT=19m
meas tran iin_off FIND i(Vimeas) AT=19m
meas tran iin_on FIND i(Vimeas) AT=8.5m
meas tran vgate_rst AVG v(gate) from=0.1m to=20m
.endc
.end
"""
    v, out = spice(net)
    return v


# --------------------------------------------------------------- 2. reverse-polarity P-FET (AO3401A)
def revpol(vin):
    net = f"""* J1 -> AO3401A (D to J1, S to rail, G straight to GND since v24) -> +5V rail with ~0.25 A load
.model P3401 PMOS(LEVEL=1 VTO=-0.9 KP=4.07)   ; 60 mOhm at Vgs -5 V: the AO3401A's max Rds(on) (47 typ)
Vin in 0 DC {vin}
M1 in g v5 v5 P3401 L=1u W=1u
Vg g 0 0
Rload v5 0 20
C1 v5 0 22u
.control
op
print v(v5)
print i(Vin)
.endc
.end
"""
    v, out = spice(net)
    v5 = float(re.findall(r"v\(v5\)\s*=\s*([-+0-9.eE]+)", out)[0])
    i = float(re.findall(r"i\(vin\)\s*=\s*([-+0-9.eE]+)", out)[0])
    return v5, i


# --------------------------------------------------------------- 3b. v32 mains: triac turn-off transients
# Restored from v14 (git 9632e10), with v32's values: ST T435-600B triacs (IH 35 mA max, 600 V), the
# 100 R + 22 nF X2 snubber. The triac is a gated switch that keeps conducting while |I| > IH after the
# gate is removed; the pump has its own series diode; coil values are estimates (see REPORT notes).
def snubber(with_rc, vrms=120.0, L=1.2, R=220.0, csn="22n", ih=0.035):
    """Pump (diode + coil + stray C). Returns max |dv/dt| (V/us) and peak |V| across the triac."""
    vpk = vrms * 2 ** 0.5
    rc = f"Rsn mt2 s 100\nCsn s mt1 {csn}" if with_rc else ""
    net = f"""* triac commutation with an inductive half-wave pump
.model DP D(IS=1e-12 N=1.5 RS=0.5 CJO=40p TT=2u)
.model DT D(IS=1e-12 N=1.3 RS=0.02)
.model SW SW(RON=0.01 ROFF=100meg VT=0.5 VH=0.1)
Vline mt2 0 SIN(0 {vpk} 60)
Vg g 0 PWL(0 1 40.6m 1 40.61m 0)
Vsense mt2 xs 0
Bctl c 0 V = max(v(g), abs(i(Vsense)) > {ih} ? 1 : 0)
S1 xs x c 0 SW
Dt x mt1 DT
Ctr mt2 mt1 60p
Roff mt2 mt1 50meg
{rc}
Dp mt1 p DP
Lp p q {L}
Cw p q 150p
Rp q 0 {R}
.tran 0.05u 80m 30m 0.05u
.control
run
@WRDATA@
.endc
.end
"""
    w = wave(net, ["v(mt2,mt1)"], 0.08)
    t, v = w["time"], w["v(mt2,mt1)"]
    dv = max(abs((v[i] - v[i - 1]) / (t[i] - t[i - 1])) for i in range(1, len(t)) if t[i] > 40.6e-3 and t[i] > t[i - 1])
    return dv / 1e6, max(abs(x) for x, tt in zip(v, t) if tt > 40.6e-3)


def valve(with_rc, vrms=120.0, L=4.4, R=300.0, csn="22n", ih=0.035):
    """3-way valve: AC solenoid, no diode. Returns max |dv/dt| (V/us) and peak |V| across the triac."""
    vpk = vrms * 2 ** 0.5
    rc = f"Rsn mt2 s 100\nCsn s mt1 {csn}" if with_rc else ""
    net = f"""* valve turn-off
.model DT D(IS=1e-12 N=1.3 RS=0.05)
.model SW SW(RON=0.01 ROFF=100meg VT=0.5 VH=0.1)
Vline mt2 0 SIN(0 {vpk} 60)
Vg g 0 PWL(0 1 40.6m 1 40.61m 0)
Vsense mt2 xs 0
Bctl c 0 V = max(v(g), abs(i(Vsense)) > {ih} ? 1 : 0)
S1 xs x c 0 SW
Dt1 x mt1 DT
Dt2 mt1 x DT
Ctr mt2 mt1 30p
Roff mt2 mt1 50meg
{rc}
Lv mt1 q {L}
Cw mt1 q 100p
Rv q 0 {R}
.tran 0.05u 80m 30m 0.05u
.control
run
@WRDATA@
.endc
.end
"""
    w = wave(net, ["v(mt2,mt1)"], 0.08)
    t, v = w["time"], w["v(mt2,mt1)"]
    dv = max(abs((v[i] - v[i - 1]) / (t[i] - t[i - 1])) for i in range(1, len(t)) if t[i] > 40.6e-3 and t[i] > t[i - 1])
    return dv / 1e6, max(abs(x) for x, tt in zip(v, t) if tt > 40.6e-3)


# --------------------------------------------------------------- 4. pressure input
def pressure():
    net = """* transducer -> 47k/100k -> 22 nF -> ESP32-C6 ADC1 (high-impedance SAR input)
Vt t 0 DC 4.5 AC 1
Rs t s 100          ; transducer output impedance (est)
R1 s a 47k
R2 a 0 100k
C1 a 0 22n
Rin a 0 100meg      ; ESP32-C6 SAR input between samples: leakage only (no datasheet figure; 100 Mohm is conservative)
.control
op
print v(a)
ac dec 50 1 10k
meas ac f3db WHEN vdb(a)=-6.95 FALL=1
meas ac g60 FIND vdb(a) AT=60
meas ac g100 FIND vdb(a) AT=100
.endc
.end
"""
    v, out = spice(net)
    return v, out


# --------------------------------------------------------------- 5. 5 V / 3.3 V rails under load steps
def rails(vpsu, label):
    net = f"""* PSU -> P-FET -> 22u -> TLV1117LV33 (dropout <= 0.7 V at 1 A max, modelled linear: 0.35 V at 0.5 A) -> 22u||10u||0.1u -> ESP32
.model SS14 D(IS=5e-6 N=1.1 RS=0.05)
Vpsu psu 0 DC {vpsu}
Rpsu psu v5 0.21             ; PSU + wiring (0.15) + AO3401A (0.06) in series
C1 v5 0 22u
* other 5 V loads: transducer 10 mA + heater SSR 10 + pump and valve SSR boards 2 x 25 + 3 LEDs 12 mA
Rl5 v5 0 61
* LDO: tracks 3.3 V unless dropout bites
Bldo v33r 0 V = min(3.3, v(v5) - 0.7*i(Vsense))   ; TLV1117LV33: 230 mV typ at 500 mA, 700 mV max at 1 A (SBVS160C table 6.5)
Rout v33r v33s 0.02
Vsense v33s v33 0
Bin v5 0 I = i(Vsense)        ; LDO input current = output current
C3 v33 0 22u
C4 v33 0 10u
* ESP32: 80 mA idle, Wi-Fi TX bursts to 500 mA (1 ms, every 5 ms)
Iesp v33 0 PULSE(0.085 0.505 2m 2u 2u 1m 5m)   ; ESP32 (80 mA idle, 500 mA TX) + status LED + gate pull-downs
.tran 2u 20m 0 2u
.control
run
meas tran v5min MIN v(v5) from=1m to=20m
meas tran v33min MIN v(v33) from=1m to=20m
.endc
.end
"""
    v, out = spice(net)
    return v, out


# --------------------------------------------------------------- 6. thermistor input
SH = (8.965973e-4, 2.145003e-4, 1.078228e-7)   # Molex 218537-3919, fitted to its R-T table (25/100/140 C)


def sh_c(r):
    import math
    l = math.log(r)
    return 1 / (SH[0] + SH[1] * l + SH[2] * l ** 3) - 273.15


def ntc_r(t_c, scale=1.0):   # Molex 218537-3919 (docs/BUY.md); scale = R tolerance
    lo, hi = 10.0, 1e7
    for _ in range(200):
        m = (lo * hi) ** 0.5
        lo, hi = (m, hi) if sh_c(m) > t_c else (lo, m)
    return m * scale


def ntc_v(t_c, rref=4.7e3, vref=3.3, **kw):
    r = ntc_r(t_c, **kw)
    return vref * r / (r + rref)


def ntc_from_v(v, rref=4.7e3, vref=3.3):
    return sh_c(rref * v / (vref - v))


def main():
    # 1. outputs: on-state at the corners, with each module type
    for load, vmin, lab in (("ssr_sch", 3.0, "heater SSR (Schneider SSP1A125BDT, 3.0 V)"),):
        for vg in (2.64, 3.3):   # C6 datasheet guarantees VOH >= 0.8 x VDD = 2.64 V
            for fet in ("vth_typ", "vth_max"):
                for v5 in (4.65, 5.1):   # USB through D2 / IRM-10-5 +2 %
                    r = output(vg, fet, v5, load)
                    check("Output on: voltage across the module input", f"{lab}, GPIO {vg} V, {fet}, +5V {v5} V", r["vin_on"], f">= {vmin} V", r["vin_on"] >= vmin, "V")
        r = output(2.64, "vth_max", 5.0, load)
        check("Output off: current left through the module input", lab, abs(r["iin_off"]) * 1e3, "< 0.1 mA (SSR must-release ~1 mA)", abs(r["iin_off"]) * 1e3 < 0.1, "mA")
        check("Output turn-off spike at the drain", lab, r["vdrain_pk"], "< 20 V (AO3400A 30 V rating; resistive SSR input, no flyback diode)", r["vdrain_pk"] < 20, "V")
    # v32: pump and valve FETs drive the MOC3083 LEDs (330 R from +5V)
    for vg in (2.64, 3.3):
        for fet in ("vth_typ", "vth_max"):
            for v5 in (4.65, 5.1):
                r = output(vg, fet, v5, "opto_led")
                i = abs(r.get("iin_on", 0)) * 1e3
                check("Opto-triac LED current (MOC3083: IFT 5 mA max, IF 50 mA abs max)", f"GPIO {vg} V, {fet}, +5V {v5} V, Vf 1.5 V max", i, ">= 7.5 mA (1.5x IFT max) and <= 30 mA", 7.5 <= i <= 30, "mA")
    r = output(2.64, "vth_max", 5.0, "opto_led")
    check("Opto-triac LED off", "FET off", abs(r["iin_off"]) * 1e3, "< 0.1 mA (far below any trigger)", abs(r["iin_off"]) * 1e3 < 0.1, "mA")
    # boot/reset: GPIO high-Z with a weak internal pull-up fighting the 4.7k pull-down
    for rpu in ("45k", "20k"):
        r = output(3.3, "vth_typ", 5.0, "ssr_sch", rpu=rpu)
        check("Output during reset (GPIO high-Z)", f"internal pull-up {rpu} vs 3.3k pull-down", r["vgate_rst"], "< 0.6 V (AO3400A Vth min 0.65 V): load stays off", r["vgate_rst"] < 0.6, "V")
        check("Output during reset: module input current", f"internal pull-up {rpu}", abs(r["iin_off"]) * 1e3, "< 0.1 mA (off)", abs(r["iin_off"]) * 1e3 < 0.1, "mA")
    r = output(3.3, "vth_typ", 5.0, "ssr_sch", rpd="100k", rpu="45k")
    check("Output during reset, 100k pull-down (rejected value)", "internal pull-up 45k", r["vgate_rst"], "reference only: why the pull-down is 3.3k", True, "V")
    # 3b. mains: triac turn-off transients (v32)
    for vr in (120.0, 132.0):
        for with_rc in (False, True):
            dv, peak = snubber(with_rc, vrms=vr)
            lab = f"{vr:.0f} V, {'100 R + 22 nF X2' if with_rc else 'no snubber'}"
            if with_rc:
                check("Pump triac (T435) turn-off: peak voltage", lab, peak, "< 480 V (80 % of the T435's 600 V)", peak < 480, "V")
                check("Pump triac turn-off: dV/dt", lab, dv, "< 100 V/us (T435 static 400 V/us; MOC3083 static >= 600 V/us)", dv < 100, "V/us")
            else:
                check("Pump triac turn-off: peak voltage (reference)", lab, peak, "info: why the snubber is fitted", True, "V")
            dv, peak = valve(with_rc, vrms=vr, ih=0.015)    # T410: IH 15 mA max
            if with_rc:
                check("Valve triac (T410) turn-off: peak voltage", lab, peak, "< 480 V (80 % of the T410's 600 V)", peak < 480, "V")
                check("Valve triac (T410) turn-off: dV/dt", lab, dv, "< 32 V/us (80 % of the T410's 40 V/us; MOC3083 static >= 600 V/us)", dv < 32, "V/us")
            else:
                check("Valve triac (T410) turn-off: peak voltage (reference)", lab, peak, "info: why the snubber is fitted", True, "V")
    dv, peak = valve(True, vrms=132.0, ih=0.035)
    check("Valve with a T435 instead (rejected)", "132 V, IH 35 mA, 100 R + 22 nF", peak, "reference: over the 600 V rating, why the valve uses the T410", True, "V")
    i_leak = 120.0 * 2 * math.pi * 60 * 22e-9 * 1e3
    check("Snubber off-state leakage", "22 nF at 120 V 60 Hz", i_leak, "< 2 mA (valve hold-in needs ~50 mA; pump coil ignores it)", i_leak < 2, "mA")
    i_cap = 120 * 2 ** 0.5 / 100
    check("Snubber discharge into the triac at turn-on", "170 V peak / 100 R", i_cap, "< 31 A (T435 ITSM) and < 4 A di/dt-friendly", i_cap < 4, "A")
    p_r = (i_leak / 1e3) ** 2 * 100 * 1e3
    check("Snubber resistor dissipation", "steady off-state", p_r, "< 100 mW (2512 1 W)", p_r < 100, "mW")
    # thermals at 70 C inside the case (analytic; ST T4 datasheet: Vt0 0.89 V, Rd 0.12 ohm, DPAK
    # Rth(j-a) 70 C/W on 0.5 cm2 of copper, lower on more: 60 C/W assumed for ~1-2 cm2 of AC_L_F copper)
    TA = 70.0
    for ipk, lab in ((0.9, "pump 0.45 A rms half-wave (0.9 A peak)"), (1.4, "pump, 1.4 A peak allowance")):
        p = 0.89 * ipk / math.pi + 0.12 * (ipk / 2) ** 2
        tj = TA + 60 * p
        check("Pump triac (T435) junction temperature", lab, tj, "< 115 C (Tj max 125 C, 10 C margin)", tj < 115, "C")
    p = 0.89 * 0.1 * 2 * 2 ** 0.5 / math.pi + 0.12 * 0.1 ** 2
    check("Valve triac (T410) junction temperature", "0.1 A rms full-wave", TA + 60 * p, "< 115 C", TA + 60 * p < 115, "C")
    i_avg, i_pk = 0.20, 0.45      # 5 V load: ESP32-C6 Wi-Fi average / bursts, LEDs, SSR input, opto LEDs, transducer
    check("IRM-10-5 load vs derating", "70 C: 50 % of 2 A = 1.0 A (Mean Well derating curve)", i_pk, "<= 1.0 A (peak, Wi-Fi + both opto LEDs + heater SSR)", i_pk <= 1.0, "A")
    p_ldo = (5.0 - 3.3) * 0.15
    check("LDO (TLV1117LV, SOT-223) junction temperature", "1.7 V x 150 mA average, ~100 C/W on its copper", TA + 100 * p_ldo, "< 115 C (Tj max 125 C)", TA + 100 * p_ldo < 115, "C")
    p_r = 0.011 ** 2 * 330 * 1e3
    check("Opto LED resistor dissipation (R47/R48, 0603)", "11 mA", p_r, "< 50 mW (0603 100 mW, half derated at 70 C)", p_r < 50, "mW")
    check("Board fuse (F1, 2 A slow-blow) continuous load", "IRM 0.12 A + pump 0.45 A + valve 0.1 A", 0.67, "< 1.45 A (2 A x 0.75 x 0.97 rerating at 70 C)", 0.67 < 1.45, "A")
    # thermocouple input (analytic): the MCP3421 at PGA 8 against a K-type's 41 uV/C
    fs = 2.048 / 8 * 1e3
    v_hot = 41e-3 * (170 - 20)
    check("Thermocouple signal range", "tip 170 C, board 20 C (K-type ~41 uV/C)", v_hot, f"< {fs:.0f} mV (MCP3421 PGA 8 full scale)", v_hot < fs, "mV")
    v_open = 3.3 * 281e3 / (10e6 + 281e3 + 2e3) * 1e3
    check("Thermocouple open-probe reading", "10M bias vs ~281k ADC input (PGA 8)", v_open, "> 15 mV (firmware: open probe) and < 256 mV (in range)", 15 < v_open < 256, "mV")
    e_bias = 3.3 / 10e6 * 20 / 41e-6
    check("Thermocouple bias error", "0.33 uA through a 20-ohm probe loop", e_bias, "< 0.3 C (constant; calibration removes it)", e_bias < 0.3, "C")
    # status LED
    i_led = (3.3 - 2.0) / 1000 * 1e3
    check("Status LED (red, 1k from GPIO16 / U0TXD)", "3.3 V", i_led, "1-3 mA (visible, well within GPIO drive)", 1 <= i_led <= 3, "mA")
    # front panel (v24): 1k series at the connector, 10k pull-up on the GPIO side, plus the C6's 45k internal pull-up
    rpu = 1 / (1 / 10e3 + 1 / 45e3)
    v_closed = 3.3 * 1e3 / (1e3 + rpu)
    check("Front-panel switch closed", "1k series, 10k || 45k pull-up", v_closed, "<= 0.5 V (C6 VIL max 0.25 x VDD = 0.825 V: >= 0.3 V margin)", v_closed <= 0.5, "V")
    i_wet = 3.3 / (1e3 + rpu) * 1e3
    check("Front-panel switch wetting current", "closed", i_wet, ">= 0.2 mA (keeps an old rocker's contacts clean)", i_wet >= 0.2, "mA")
    i_lamp = (3.3 - 2.0) / 330 * 1e3
    check("Front-panel LED drive", "330 R from a 3.3 V GPIO, red LED Vf ~2.0 V", i_lamp, "2-8 mA (bright, within the C6's 20 mA drive)", 2 <= i_lamp <= 8, "mA")
    # 4. pressure into the C6's ADC1
    v, out = pressure()
    va = float(re.findall(r"v\(a\)\s*=\s*([-+0-9.eE]+)", out)[0])
    check("Pressure input", "4.5 V full scale -> GPIO0", va, "<= 3.1 V (inside the C6 ADC's 12 dB range, below the 3.3 V rail)", va <= 3.1, "V")
    check("Pressure input", "0.5 V (0 bar) -> GPIO0", 0.5 * 100 / 147, ">= 0.1 V (clear of the ADC's non-linear bottom)", 0.5 * 100 / 147 >= 0.1, "V")
    check("Pressure transducer load", "R16 + R17", 147, "> 100 kOhm (any 0.5-4.5 V voltage-output transducer; XIDIBEI drives ~10 mA)", 147 > 100, "kOhm")
    f3 = v.get("f3db", 0)
    check("Pressure filter", "-3 dB corner", f3, "150-400 Hz (keeps the pump's 60 Hz pulses, kills RF)", 150 <= f3 <= 400, "Hz")
    g60 = v.get("g60", 0) - 20 * math.log10(va / 4.5)   # relative to the divider's own DC gain
    check("Pressure filter", "60 Hz pump ripple", g60, "> -1 dB (the pulses stay visible to firmware)", g60 > -1.0, "dB")
    # The C6 datasheet doesn't give the ADC's sampling capacitance: 10 pF is an ESTIMATE (typical SAR
    # input). The criterion is a tenth of the transducer's own +-0.25 % FS accuracy at the pin (2.7 mV).
    kick = 3.0 * 10e-12 / 22e-9 * 1e3
    check("Pressure ADC kick-back (estimate)", "10 pF sample cap (assumed) vs 22 nF", kick, "< 2.7 mV (1/10 of the transducer's +-0.25 % FS at the pin)", kick < 2.7, "mV")
    gain = (100 * 1.01 / (100 * 1.01 + 47 * 0.99)) / (100 / 147) - 1
    check("Pressure gain error from 1 % divider resistors", "worst-case ratio", abs(gain) * 9.0, "< 0.15 bar at 9 bar (zero is calibrated at power-up)", abs(gain) * 9.0 < 0.15, "bar")
    # 6. rails
    for vpsu, label in ((5.0, "IRM-10-5 5.0 V"), (4.9, "IRM-10-5 4.9 V (-2 %)"), (4.65, "USB 5.0 V through D2 (~4.65 V)")):
        v, out = rails(vpsu, label)
        check("3.3 V rail under Wi-Fi bursts", label, v.get("v33min", 0), ">= 3.0 V (ESP32-C6 min)", v.get("v33min", 0) >= 3.0, "V")
        check("5 V rail", label, v.get("v5min", 0), ">= 4.2 V (info: on USB the transducer runs below the 4.75 V a 5 V part is usually specified for; readings indicative)", v.get("v5min", 0) >= 4.2, "V")

    lines = ["# Circuit simulation report", "",
             "ngspice 42, behavioural models fitted to datasheet values (see `sim/run.py`). Generated by `python3 sim/run.py`.", "",
             "| Block | Case | Result | Requirement | |", "|---|---|---|---|---|"]
    for block, case, val, limit, ok in results:
        lines.append(f"| {block} | {case} | {val} | {limit} | {'✅' if ok else '❌'} |")
    fails = [r for r in results if not r[4]]
    lines += ["", f"**{len(results) - len(fails)} / {len(results)} checks pass.**"]
    open(os.path.join(HERE, "REPORT.md"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines[4:]))
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
