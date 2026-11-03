#!/usr/bin/env python3
"""Build the audit package: everything a reviewer (human or LLM) needs, in one folder + zip.

    python3 audit.py [--mains 120]   ->  build/audit-<V>v/  and  build/coffee-controller-audit-<V>v.zip
"""
import argparse, csv, json, os, re, shutil, subprocess, zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))

BRIEF = """# Audit brief: coffee controller board (v6, {mains} V)

You are reviewing a **mains-connected controller board** for a home espresso machine (Gaggia Classic
Pro, US 120 V / 60 Hz). Treat it as a safety review first and a functional review second. The
owner holds an electrician's licence but is not an electronics designer. An AI assistant produced
the design: schematic as code, hand-constrained placement, autorouting, script checks.
**Nobody has built it yet.** Please be blunt.

## What the board does (one paragraph)
An ESP32-C6 module (external antenna via U.FL) runs a temperature/pressure controller. On the
mains side, three zero-cross opto-triacs (MOC3083, 800 V) fire three 800 V triacs straight from
GPIOs:
- **heater** (1370 W, BTA24-800BW, wired in place of the brew thermostat; the machine's steam
  thermostat and thermal fuse stay in series);
- **vibratory pump** (Ulka, half-wave diode inside; BTA08-800CW + 100 Ω/10 nF snubber);
- **3-way solenoid valve** (Z0109NN + 100 Ω/10 nF snubber; both snubber caps are Murata GA355 SMD X2).

Loads are switched in whole mains cycles. The firmware runs one control tick per mains cycle from
the crystal; the opto-triacs' own zero-cross circuits do the switching at the crossings. There is
**no zero-cross detector**: a one-cycle window always holds exactly one positive half-wave, so the
pump stroke count is exact without knowing the phase.

A RECOM RAC03E-05SK/277 module (85 °C, EN60335, 4 kV reinforced) makes 5 V; a TI TLV1117LV33 makes 3.3 V. Sensors go straight into the C6's ADC1:
- a 100k B3950 NTC thermistor (8.2k 0.1 % reference to 3.3 V, two-point calibration);
- a 0.5–4.5 V ratiometric pressure transducer (10k/20k divider).

The brew and steam switches are rewired as dry contacts. USB-C (the C6's USB Serial/JTAG) is for
flashing only.

The mains half and the SELV half are separated by a 7 mm copper-free band with milled slots; only
the optos and the PSU cross it.

## Files in this package
| File | What |
|---|---|
| `schematic/controller.py` | **source of truth**: the schematic as code (SKiDL); values, parts, notes, design checks |
| `schematic/netlist_readable.md` | every net and the pins on it, grouped mains / SELV (start here) |
| `schematic/schematic.svg` | net-label drawing of the same |
| `schematic/*.net` | KiCad netlist |
| `bom.csv` | parts list with MPNs and notes |
| `pcb/board.kicad_pcb` | the PCB (KiCad 7+); `pcb/board.kicad_dru` = the design rules / mains lint |
| `pcb/drc.rpt` | DRC result against those rules |
| `pcb/render-*`, `pcb/board.pdf`, `pcb/board.glb` | views: 2D layers, PDF, 3D model |
| `pcb/gerbers/` | fab outputs |
| `pcb/layout.py` | placement, keep-outs, hand-routed parts (heater pours, gate, opto feed, fan-out stubs) |
| `sim/run.py`, `sim/REPORT.md`, `sim/waves.json` | ngspice testbenches with tolerance corners, results, waveforms |
| `docs/CIRCUIT.md` | design notes and calculations |
| `docs/INSTALL.md` | how it's wired into the machine and brought up |
| `firmware/main.rs` | ESP32-C6 firmware entry point: pins, tick, ADC, outputs |

## Please check, in priority order
1. **Mains safety.**
   - The isolation barrier: 7 mm band, slots, parts that cross it.
   - Mains-to-mains spacing: 2.0 mm rule, with the exemptions listed in `board.kicad_dru`: a part's own pins (both items inside its courtyard) and same-potential groups. The fuse's two sides are deliberately *not* exempt (open fuse = full line voltage). Is IEC 60664-1 (~1.6 mm at 120 V) the right basis, or should UL-style spacings apply?
   - Fusing: F1 only protects the PSU branch; the loads rely on the machine's own protection.
   - The heater triac in place of the brew thermostat, with the stock steam thermostat and thermal fuse still in series.
   - Earthing of the chassis; mounting hole H1 on the mains side; the heater triac's tab bolt.
   - The enclosure (3D-printed, flame-retardant filament).
2. **Fail-safe behaviour.**
   - Are all loads off when the ESP32 is in reset, booting, flashing or crashed? There are **no pull-downs**. The argument: a floating or weakly pulled pin can't source the 5 mA the opto LED needs. The drive pins are not strapping pins.
   - The firmware drops all outputs on a sensor fault or a stalled tick.
   - What happens if a triac fails short? Only the stock thermostat/fuse stops the heater.
3. **Triac drive.**
   - MOC3083 from a 3.3 V GPIO through 150 Ω: simulated 8–12 mA, against IFT 5 mA max.
   - 360 Ω opto series resistors.
   - Only the sensitive-gate Z0109 has a gate–MT1 resistor (330 Ω), per onsemi AN-3004.
   - Snubbers: why each one is there is in `sim/REPORT.md`. The valve turns off near the line peak and rings to ~2× it (≤ 477 V at 132 V, with the ceramic derated), hence 800 V parts.
4. **Thermal.**
   - Heater triac ~11 W at 11.4 A; R_th(j-c) 1.7 °C/W, bolted to the chassis. Is that enough?
   - LDO: TLV1117LV33 at ~0.2 W average.
   - The case interior runs at 60–70 °C.
5. **Measurement chain.**
   - NTC divider: mid-scale near 93 °C, 24 mV/°C.
   - Self-heating 0.33 mW (constant, cancelled by calibration).
   - The probe's insulation to the earthed boiler (SELV ground floats).
   - Pressure: 2/3 divider, 100 nF, ~230 Hz corner; the C6's ADC accuracy (factory eFuse calibration plus two-point install calibration).
6. **Simplifications.** Parts were removed on purpose (see "Deliberately omitted"). Is anything there a mistake?
7. **Layout.**
   - The heater path: copper pours; ~2 mm at the TO-220 middle pin, then 4.5–6 mm, both layers.
   - Autorouted SELV: grounding, decoupling placement, the module's exposed pad.
   - Anything the DRC can't see.

## Deliberately omitted (to keep it simple)
- No zero-cross detector (crystal tick; zero-cross optos do the switching).
- No external ADC, no thermocouple amplifier, no 5 V monitor.
- No MOSFET drivers, no pull-downs on the drive lines, no gate resistors on the two non-sensitive triacs.
- No RESET or BOOT buttons (a BOOT test pad underneath instead), no USB ESD chip.
- No TVS diodes or series resistors on the sensor and switch leads.
- No electrolytic capacitors.

## Known limitations (stated, not hidden)
- **The simulation models are behavioural:**
  - diodes fitted to datasheet Vf;
  - triacs as latching switches with a holding current;
  - LDO as a dropout curve;
  - pump modelled as 1.2 H / 220 Ω plus a diode; valve as 4.4 H / 300 Ω.
- **The coil inductances are estimates**, not measurements.
- **Some datasheet figures weren't confirmed:** the BTA24's V_TO, r_d and IGT were not checked against the official ST datasheet (R_th 1.7 °C/W was). The Z0109's IH of 10 mA comes from distributor data.
- **Footprints:** the ESP32-C6 footprint is Espressif's own KiCad library; the rest are KiCad library defaults. Check the RECOM RAC03E footprint (drawn from its datasheet, lib/Recom.pretty) and the MKDS 3 heater terminal against the parts actually bought.
- **No EMC analysis.** No thermal measurement.

## How to reproduce
`python3 controller.py` (schematic → netlist, BOM, checks), `python3 layout.py --mains {mains}` (PCB,
DRC), `python3 sim/run.py` (simulations). Tool versions: SKiDL 3.0, KiCad 7.0.11, Freerouting 2.1.0,
ngspice 42.
"""


def readable_netlist(parts_json, bom_csv):
    spec = json.load(open(parts_json))
    vals = {}
    for r in csv.DictReader(open(bom_csv)):
        for ref in r["Refs"].split():
            vals[ref] = f'{r["Value"]} ({r["Part / MPN"]})'
    nets = {}
    for ref, p in spec["parts"].items():
        for pin, net in p["pins"].items():
            if net:
                nets.setdefault(net, []).append(f"{ref}.{pin}")
    mains = set(spec["mains_nets"])
    out = [f"# Readable netlist ({spec['mains_v']} V build)", "",
           "Each net, then the part pins on it. Part values are in the table at the end.", ""]
    for title, pick in (("Mains side (hazardous voltage)", lambda n: n in mains), ("SELV side (low voltage)", lambda n: n not in mains)):
        out += [f"## {title}", ""]
        for n in sorted(k for k in nets if pick(k)):
            out.append(f"- **{n}**: " + ", ".join(sorted(nets[n])))
        out.append("")
    out += ["## Parts", "", "| Ref | Value / part | Footprint |", "|---|---|---|"]
    for ref in sorted(spec["parts"], key=lambda r: (re.sub(r"\d", "", r), int(re.sub(r"\D", "", r) or 0))):
        out.append(f"| {ref} | {vals.get(ref, spec['parts'][ref]['value'])} | {spec['parts'][ref]['footprint'].split(':')[-1]} |")
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mains", type=int, default=120)
    a = ap.parse_args()
    v = a.mains
    b = os.path.join(HERE, "build")
    dst = os.path.join(b, f"audit-{v}v")
    shutil.rmtree(dst, ignore_errors=True)
    for d in ("schematic", "pcb", "pcb/gerbers", "sim", "docs", "firmware"):
        os.makedirs(os.path.join(dst, d), exist_ok=True)
    stem = os.path.join(b, f"coffee-controller-{v}v")
    cp = lambda src, rel: os.path.exists(src) and shutil.copy(src, os.path.join(dst, rel))
    open(os.path.join(dst, "AUDIT_BRIEF.md"), "w").write(BRIEF.format(mains=v))
    cp(os.path.join(HERE, "controller.py"), "schematic/controller.py")
    cp(stem + ".net", "schematic/")
    open(os.path.join(dst, "schematic/netlist_readable.md"), "w").write(
        readable_netlist(os.path.join(b, f"parts-{v}v.json"), os.path.join(b, f"bom-{v}v.csv")))
    cp(os.path.join(b, f"bom-{v}v.csv"), "bom.csv")
    cp(stem + ".kicad_pcb", "pcb/board.kicad_pcb")
    cp(stem + ".kicad_dru", "pcb/board.kicad_dru")
    cp(stem + "-drc.rpt", "pcb/drc.rpt")
    cp(stem + ".pdf", "pcb/board.pdf")
    cp(stem + ".png", "pcb/render-all-layers.png")
    cp(os.path.join(HERE, "layout.py"), "pcb/layout.py")
    for f in ("top", "bottom"):
        cp(f"{stem}-{f}.svg", f"pcb/render-{f}.svg")
    g = stem + "-fab"
    if os.path.isdir(g):
        for f in os.listdir(g):
            shutil.copy(os.path.join(g, f), os.path.join(dst, "pcb/gerbers", f))
    vdir = os.path.join(HERE, "versions", "v10-two-sided")
    cp(os.path.join(vdir, "board.glb"), "pcb/board.glb")
    cp(os.path.join(vdir, "3d.png"), "pcb/render-3d.png")
    for f in ("run.py", "waves.py", "REPORT.md", "waves.json"):
        cp(os.path.join(HERE, "sim", f), "sim/")
    for f in ("CIRCUIT.md", "INSTALL.md"):
        cp(os.path.join(ROOT, "docs", f), "docs/")
    cp(os.path.join(ROOT, "firmware/esp32/src/main.rs"), "firmware/main.rs")
    cp(os.path.join(b, f"schematic-{v}v.svg"), "schematic/schematic.svg")
    z = os.path.join(b, f"coffee-controller-audit-{v}v.zip")
    with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as zf:
        for dp, _, fs in os.walk(dst):
            for f in fs:
                p = os.path.join(dp, f)
                zf.write(p, os.path.relpath(p, os.path.dirname(dst)))
    n = sum(len(fs) for _, _, fs in os.walk(dst))
    print(f"wrote {dst}/ ({n} files) and {z} ({os.path.getsize(z) // 1024} KB)")


if __name__ == "__main__":
    main()
