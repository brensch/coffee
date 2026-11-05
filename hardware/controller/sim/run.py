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
    }
    drive = f"Vg g 0 PULSE(0 {vgpio} 1m 1u 1u 8m 20m)\nRgpio g pin 25" if rpu is None else f"Rpu v33 pin {rpu}\nVv33 v33 0 3.3"
    net = f"""* output stage
.model N3400 NMOS(LEVEL=1 VTO={vto} KP={kp} CGSO=1.5n CGDO=0.3n)
.model D4148 D(IS=2.5n N=1.75 RS=0.6 BV=75 CJO=2p TT=4n)
.model DRED D(IS=1e-19 N=1.8 RS=8)
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
    for load, vmin, lab in (("ssr_sch", 3.0, "heater SSR (Schneider SSP1A125BDT, 3.0 V)"),
                            ("ssr_board", 4.0, "pump/valve SSR board (Omron G3MB-202P DC5, must-operate 4.0 V)")):
        for vg in (2.64, 3.3):   # C6 datasheet guarantees VOH >= 0.8 x VDD = 2.64 V
            for fet in ("vth_typ", "vth_max"):
                for v5 in (4.6, 5.0):
                    r = output(vg, fet, v5, load)
                    check("Output on: voltage across the module input", f"{lab}, GPIO {vg} V, {fet}, +5V {v5} V", r["vin_on"], f">= {vmin} V", r["vin_on"] >= vmin, "V")
        r = output(2.64, "vth_max", 5.0, load)
        check("Output off: current left through the module input", lab, abs(r["iin_off"]) * 1e3, "< 0.1 mA (SSR must-release ~1 mA)", abs(r["iin_off"]) * 1e3 < 0.1, "mA")
        check("Output turn-off spike at the drain", lab, r["vdrain_pk"], "< 20 V (AO3400A 30 V rating; resistive SSR inputs, no flyback diode)", r["vdrain_pk"] < 20, "V")
    # boot/reset: GPIO high-Z with a weak internal pull-up fighting the 4.7k pull-down
    for rpu in ("45k", "20k"):
        r = output(3.3, "vth_typ", 5.0, "ssr_sch", rpu=rpu)
        check("Output during reset (GPIO high-Z)", f"internal pull-up {rpu} vs 3.3k pull-down", r["vgate_rst"], "< 0.6 V (AO3400A Vth min 0.65 V): load stays off", r["vgate_rst"] < 0.6, "V")
        check("Output during reset: module input current", f"internal pull-up {rpu}", abs(r["iin_off"]) * 1e3, "< 0.1 mA (off)", abs(r["iin_off"]) * 1e3 < 0.1, "mA")
    r = output(3.3, "vth_typ", 5.0, "ssr_sch", rpd="100k", rpu="45k")
    check("Output during reset, 100k pull-down (rejected value)", "internal pull-up 45k", r["vgate_rst"], "reference only: why the pull-down is 3.3k", True, "V")
    # 2. reverse polarity
    v5, i = revpol(5.0)
    check("Reverse-polarity P-FET", "J1 at +5.0 V, 250 mA load", v5, ">= 4.95 V (drop < 50 mV)", v5 >= 4.95, "V")
    v5, i = revpol(-5.0)
    check("Reverse-polarity P-FET", "J1 wired backwards (-5 V)", abs(i) * 1e3, "< 0.01 mA into the board", abs(i) * 1e3 < 0.01, "mA")
    # mis-plug: the 5 V lead (every cable is the same GH 3-pin) in the thermistor socket J5
    i_clamp = (5.0 - 3.3 - 0.5) / 1000 * 1e3
    check("J5 mis-plug (5 V lead in the thermistor socket)", "1k R27 into GPIO1's clamp diode", i_clamp, "< 2 mA (no damage; was unlimited without R27)", i_clamp < 2, "mA")
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
    check("Pressure transducer load", "R16 + R17", 147, "> 100 kOhm (TE M3200 datasheet p2, voltage output)", 147 > 100, "kOhm")
    f3 = v.get("f3db", 0)
    check("Pressure filter", "-3 dB corner", f3, "150-400 Hz (keeps the pump's 60 Hz pulses, kills RF)", 150 <= f3 <= 400, "Hz")
    g60 = v.get("g60", 0) - 20 * math.log10(va / 4.5)   # relative to the divider's own DC gain
    check("Pressure filter", "60 Hz pump ripple", g60, "> -1 dB (the pulses stay visible to firmware)", g60 > -1.0, "dB")
    # The C6 datasheet doesn't give the ADC's sampling capacitance: 10 pF is an ESTIMATE (typical SAR
    # input). The criterion is a tenth of the transducer's own +-0.25 % FS accuracy at the pin (2.7 mV).
    kick = 3.0 * 10e-12 / 22e-9 * 1e3
    check("Pressure ADC kick-back (estimate)", "10 pF sample cap (assumed) vs 22 nF", kick, "< 2.7 mV (1/10 of the transducer's +-0.25 % FS at the pin)", kick < 2.7, "mV")
    # 5. thermistor (Molex 218537-3919 50k to GND, 4.7k 1% to 3V3)
    for t in (20.0, 93.0, 125.0, 150.0):
        vt = ntc_v(t)
        check("Thermistor divider voltage", f"{t:.0f} C", vt, "0.15-3.15 V (inside the ADC's usable range)", 0.15 <= vt <= 3.15, "V")
    for t in (93.0, 140.0):
        slope = (ntc_v(t - 0.5) - ntc_v(t + 0.5)) * 1e3
        lsb = 3300 / 4096 / 4   # 12-bit, 16x oversampled -> ~2 extra bits
        check("Thermistor sensitivity", f"{t:.0f} C", slope, f"> 10 mV/C ({lsb / slope:.3f} C per oversampled LSB)", slope > 10, "mV/C")
    r93 = ntc_r(93.0)
    p = (3.3 / (4.7e3 + r93)) ** 2 * r93 * 1e3
    err = p / 1.0   # Molex dissipation constant 1 mW/C (sales drawing note 7); clamped to the boiler it sinks far better
    check("Thermistor self-heating", f"93 C, {p:.2f} mW, still air", err, "< 1 C (constant, so the boiling-water calibration point cancels it; clamped to the boiler it is far lower)", err < 1.0, "C")
    worst = max(abs(ntc_from_v(ntc_v(93.0, scale=k)) - 93.0) for k in (0.99, 1.01))
    check("Thermistor error before calibration", "93 C, R +-1 % (Molex R25 tolerance)", worst, "info: the 2-point calibration (room + boiling water) removes this", True, "C")
    r_ref_err = abs(ntc_from_v(ntc_v(93.0, rref=4.7e3 * 1.01)) - 93.0)
    check("Thermistor error from the reference resistor", "93 C, 1 % resistor", r_ref_err, "< 0.5 C before calibration (the boiling-water point removes it)", r_ref_err < 0.5, "C")
    gain = (100 * 1.01 / (100 * 1.01 + 47 * 0.99)) / (100 / 147) - 1
    check("Pressure gain error from 1 % divider resistors", "worst-case ratio", abs(gain) * 9.0, "< 0.15 bar at 9 bar (zero is calibrated at power-up)", abs(gain) * 9.0 < 0.15, "bar")
    # 6. rails
    for vpsu, label in ((5.0, "PSU 5.0 V"), (4.75, "PSU 4.75 V (-5%)"), (4.65, "USB 5.0 V through D2 (~4.65 V)")):
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
