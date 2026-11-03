#!/usr/bin/env python3
"""Circuit simulations for the controller board (ngspice), with tolerance corners.

    python3 sim/run.py            # writes sim/REPORT.md, exits 1 if any check fails

Each block mirrors controller.py's values. Models are behavioural (diodes fitted to
datasheet Vf, opto CTR as a current-controlled source, regulator dropout as a curve),
so the point is margins at the corners, not exact waveforms.
"""
import os, re, subprocess, sys, tempfile

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


# --------------------------------------------------------------- 1. triac drive current
DIODES = {  # Is, N, Rs fitted to Vf @ 10 mA
    "green_typ": (1e-20, 2.0, 10), "green_max": (2e-23, 2.0, 15),   # ~2.0 V / ~2.45 V
    "ir_typ": (1e-14, 1.6, 3), "ir_max": (2e-17, 1.75, 5),           # ~1.15 V / ~1.5 V (MOC3083 max)
}
def drive(vgpio, ir, rgpio=25, ropto=150):
    """ESP32 GPIO (VOH vgpio, ~25 ohm at drive strength 3) -> 150 R -> MOC3083 LED -> GND."""
    i = DIODES[ir]
    net = f"""* triac drive straight from the GPIO
.model DI D(IS={i[0]} N={i[1]} RS={i[2]})
Vg g 0 DC {vgpio}
Rg g pin {rgpio}
Ro pin a {ropto}
Dopto a 0 DI
.control
op
let ifm = (v(pin)-v(a))/{ropto}*1000
echo "RESULT $&ifm $&v(pin)"
.endc
.end
"""
    v, out = spice(net)
    m = re.search(r"RESULT\s+([-+0-9.eE]+)\s+([-+0-9.eE]+)", out)
    return float(m.group(1)), float(m.group(2))


# --------------------------------------------------------------- 3. pump snubber
def snubber(with_rc, vrms=230.0, L=1.2, R=220.0):
    """Pump (diode + coil + stray C) on a triac modelled as a gated diode: it conducts until its
    current falls to zero after the gate is removed. Returns max |dv/dt| (V/us) and peak |V| across it."""
    vpk = vrms * 2 ** 0.5
    rc = "Rsn mt2 s 100\nCsn s mt1 10n" if with_rc else ""
    net = f"""* triac commutation with an inductive half-wave pump
.model DP D(IS=1e-12 N=1.5 RS=0.5 CJO=40p TT=2u)
.model DT D(IS=1e-12 N=1.3 RS=0.02)
.model SW SW(RON=0.01 ROFF=100meg VT=0.5 VH=0.1)
Vline mt2 0 SIN(0 {vpk} 50)
* triac: gate held for 2 cycles, then removed; it keeps conducting until its current reaches zero
Vg g 0 PWL(0 1 40.6m 1 40.61m 0)
Vsense mt2 xs 0
Bctl c 0 V = max(v(g), abs(i(Vsense)) > 0.035 ? 1 : 0)
S1 xs x c 0 SW
Dt x mt1 DT
Ctr mt2 mt1 60p
Roff mt2 mt1 50meg
{rc}
* pump: series diode, coil with winding capacitance
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


def valve(with_rc, vrms=120.0, L=4.4, R=300.0, csn="10n", ih=0.010):
    """3-way valve: AC solenoid, NO diode. Triac (modelled as a gated antiparallel diode pair) stops
    at a current zero after the gate is removed. Returns max |dv/dt| (V/us) and peak |V| across it."""
    vpk = vrms * 2 ** 0.5
    rc = f"Rsn mt2 s 100\nCsn s mt1 {csn}" if with_rc else ""
    net = f"""* valve turn-off
.model DT D(IS=1e-12 N=1.3 RS=0.05)
.model SW SW(RON=0.01 ROFF=100meg VT=0.5 VH=0.1)
Vline mt2 0 SIN(0 {vpk} 60)
Vg g 0 PWL(0 1 40.6m 1 40.61m 0)
* triac: on while gated, and latched on while |I| > IH (10 mA); it drops out at a current zero
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
    net = """* transducer -> 10k/20k -> 100 nF -> ESP32-C6 ADC1 (high-impedance SAR input; leakage modelled as 1 Mohm)
Vt t 0 DC 4.5 AC 1
Rs t s 100          ; transducer output impedance (est)
R1 s a 10k
R2 a 0 20k
C1 a 0 100n
Rin a 0 1meg
.control
op
print v(a)
ac dec 50 1 10k
meas ac f3db WHEN vdb(a)=-6.52 FALL=1
meas ac g60 FIND vdb(a) AT=60
meas ac g100 FIND vdb(a) AT=100
.endc
.end
"""
    v, out = spice(net)
    return v, out


# --------------------------------------------------------------- 5. 5 V / 3.3 V rails under load steps
def rails(vpsu, label):
    net = f"""* PSU -> 22u||10u -> AMS1117-3.3 (dropout ~1.0 V @ 0.1 A, ~1.2 V @ 0.5 A) -> 22u||10u||0.1u -> ESP32
.model SS14 D(IS=5e-6 N=1.1 RS=0.05)
Vpsu psu 0 DC {vpsu}
Rpsu psu v5 0.15             ; PSU + wiring output resistance (est); PSU feeds +5V directly (v5: no OR diode)
C1 v5 0 22u
* other 5 V loads: transducer (~10 mA); the opto LEDs now run from 3.3 V GPIOs
Rl5 v5 0 300
* LDO: tracks 3.3 V unless dropout bites: vout = min(3.3, vin - (1.0 + 0.4*I))
Bldo v33r 0 V = min(3.3, v(v5) - (1.0 + 0.4*i(Vsense)))
Rout v33r v33s 0.02
Vsense v33s v33 0
Bin v5 0 I = i(Vsense)        ; LDO input current = output current
C3 v33 0 22u
C4 v33 0 10u
* ESP32: 80 mA idle, Wi-Fi TX bursts to 500 mA (1 ms, every 5 ms)
Iesp v33 0 PULSE(0.11 0.53 2m 2u 2u 1m 5m)   ; ESP32 + 3 opto LEDs (~10 mA each)
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
def ntc_r(t_c, r25=100e3, beta=3950.0):
    return r25 * 2.718281828459045 ** (beta * (1 / (t_c + 273.15) - 1 / 298.15))


def ntc_v(t_c, rref=8.2e3, vref=3.3, **kw):
    r = ntc_r(t_c, **kw)
    return vref * r / (r + rref)


def ntc_from_v(v, rref=8.2e3, vref=3.3, r25=100e3, beta=3950.0):
    import math
    r = rref * v / (vref - v)
    return 1 / (1 / 298.15 + math.log(r / r25) / beta) - 273.15


def main():
    # 1. drive
    for vg, lab in ((3.3, "VDD 3.3 V"), (3.0, "VDD 3.0 V (rail at its simulated minimum)")):
        for ir in ("ir_typ", "ir_max"):
            ifm, vpin = drive(vg, ir)
            check("Triac drive (opto LED current from GPIO)", f"{lab}, {ir}", ifm, "5-20 mA (MOC3083 IFT max 5; GPIO comfortable)", 5.0 <= ifm <= 20, "mA")
    # 2. pump snubber (Ulka: diode inside, inductive)
    for vr, with_rc in ((120.0, False), (120.0, True), (132.0, True)):
        dv, peak = snubber(with_rc, vrms=vr)
        label = f"{vr:.0f} V, " + ("with 100R+10nF snubber" if with_rc else "no snubber")
        check("Pump turn-off dv/dt across triac/opto", label, dv, "< 300 V/us (MOC3083 static dv/dt 600 min; 2x margin)", dv < 300, "V/us")
        if with_rc:
            check("Pump turn-off peak voltage", label, peak, "< 533 V (2/3 of the 800 V triac/opto rating)", peak < 533, "V")
        else:
            check("Pump turn-off peak voltage", label, peak, "reference only: why the snubber is fitted", True, "V")
    # 3. valve snubber (AC solenoid; the triac drops out at its holding current with the coil still carrying it)
    dv, peak = valve(False, vrms=120.0)
    check("Valve turn-off peak voltage", "120 V, no snubber", peak, "reference only: why the snubber is fitted", True, "V")
    for vr in (120.0, 132.0):
        for ih, csn in ((0.005, "10n"), (0.010, "10n"), (0.010, "8n")):   # Z0109 IH typ / max; 8 nF = 10 nF X7R derated at voltage
            dv, peak = valve(True, vrms=vr, csn=csn, ih=ih)
            label = f"{vr:.0f} V, 100R+{csn}F, IH {ih * 1e3:.0f} mA"
            check("Valve turn-off dv/dt", label, dv, "< 300 V/us (MOC3083 static dv/dt 600 min; 2x margin)", dv < 300, "V/us")
            check("Valve turn-off peak voltage", label, peak, "< 533 V (2/3 of the 800 V triac/opto rating; ~2x line peak is physics, not the snubber)", peak < 533, "V")
    # 4. pressure into the C6's ADC1
    v, out = pressure()
    va = float(re.findall(r"v\(a\)\s*=\s*([-+0-9.eE]+)", out)[0])
    check("Pressure input", "4.5 V full scale -> GPIO0", va, "<= 3.1 V (inside the C6 ADC's 12 dB range, below the 3.3 V rail)", va <= 3.1, "V")
    check("Pressure input", "0.5 V (0 bar) -> GPIO0", 0.5 * 2 / 3, ">= 0.1 V (clear of the ADC's non-linear bottom)", 0.5 * 2 / 3 >= 0.1, "V")
    f3 = v.get("f3db", 0)
    check("Pressure filter", "-3 dB corner", f3, "150-400 Hz (keeps the pump's 60 Hz pulses, kills RF)", 150 <= f3 <= 400, "Hz")
    g60 = v.get("g60", 0) + 3.52   # relative to the 2/3 divider's DC gain (-3.52 dB)
    check("Pressure filter", "60 Hz pump ripple", g60, "> -1 dB (the pulses stay visible to firmware)", g60 > -1.0, "dB")
    kick = 3.0 * 10e-12 / 100e-9 * 1e3   # 10 pF sampling cap charge-sharing with the 100 nF reservoir
    check("Pressure ADC kick-back", "10 pF sample cap vs 100 nF", kick, "< 0.8 mV (1 LSB of 12 bits over 3.3 V)", kick < 0.8, "mV")
    # 5. thermistor (100k B3950 to GND, 8.2k 0.1% to 3V3)
    for t in (20.0, 93.0, 125.0, 150.0):
        vt = ntc_v(t)
        check("Thermistor divider voltage", f"{t:.0f} C", vt, "0.15-3.15 V (inside the ADC's usable range)", 0.15 <= vt <= 3.15, "V")
    for t in (93.0, 140.0):
        slope = (ntc_v(t - 0.5) - ntc_v(t + 0.5)) * 1e3
        lsb = 3300 / 4096 / 4   # 12-bit, 16x oversampled -> ~2 extra bits
        check("Thermistor sensitivity", f"{t:.0f} C", slope, f"> 10 mV/C ({lsb / slope:.3f} C per oversampled LSB)", slope > 10, "mV/C")
    r93 = ntc_r(93.0)
    p = (3.3 / (8.2e3 + r93)) ** 2 * r93 * 1e3
    err = p / 1.5   # free-air dissipation constant 1.5 mW/C: worst case (a probe in brass sinks far better)
    check("Thermistor self-heating", f"93 C, {p:.2f} mW", err, "< 0.5 C worst case (constant, so the boiling-water calibration point cancels it)", err < 0.5, "C")
    worst = max(abs(ntc_from_v(ntc_v(93.0, r25=r25, beta=b)) - 93.0) for r25 in (99e3, 101e3) for b in (3910.0, 3990.0))
    check("Thermistor error before calibration", "93 C, R25 +-1 %, B +-1 %", worst, "info: the 2-point calibration (room + boiling water) removes this", True, "C")
    r_ref_err = abs(ntc_from_v(ntc_v(93.0, rref=8.2e3 * 1.001)) - 93.0)
    check("Thermistor error from the reference resistor", "93 C, 0.1 % resistor", r_ref_err, "< 0.1 C", r_ref_err < 0.1, "C")
    # 6. rails
    for vpsu, label in ((5.0, "PSU 5.0 V"), (4.75, "PSU 4.75 V (-5%)"), (4.6, "USB 5.0 V through D2 (~4.6 V)")):
        v, out = rails(vpsu, label)
        check("3.3 V rail under Wi-Fi bursts", label, v.get("v33min", 0), ">= 3.0 V (ESP32-C6 min)", v.get("v33min", 0) >= 3.0, "V")
        check("5 V rail", label, v.get("v5min", 0), ">= 4.2 V (pressure transducer)", v.get("v5min", 0) >= 4.2, "V")

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
