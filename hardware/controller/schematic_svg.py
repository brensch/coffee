#!/usr/bin/env python3
"""Draw a net-label schematic (SVG) from build/parts-<V>v.json.

Each part is a box with its named pins; every pin carries a tag with its net name, so
connections are read by matching names (the usual 'net label' schematic style). Parts are
grouped into functional blocks. Mains nets are drawn in red.

    python3 schematic_svg.py [--mains 120]   ->  build/schematic-<V>v.svg
"""
import argparse, html, json, os

HERE = os.path.dirname(os.path.abspath(__file__))

BLOCKS = [
    ("MAINS: spade tabs (N, L, P, 3), fuse, isolated 5 V supply", ["J12", "J13", "J14", "J15", "F1", "PS1"], True),
    ("MAINS: pump channel (opto-triac, T435, snubber)", ["U8", "Q5", "R49", "R51", "C15", "R47"], True),
    ("MAINS: valve channel (opto-triac, T410, snubber)", ["U9", "Q6", "R50", "R52", "R53", "C16", "R48"], True),
    ("ESP32-C6 module, I2C bus, status LED", ["U4", "C3", "C4", "R10", "C5", "R11", "R46", "TP1", "D1", "R12"], False),
    ("3.3 V regulator, USB (flashing / bench power)", ["U5", "C6", "C7", "D2", "J4", "R13", "R14"], False),
    ("Outputs: heater SSR (J2), pump and valve opto drives", ["J2", "Q1", "Q2", "Q3", "R4", "R5", "R6"], False),
    ("Thermocouple: ADC + cold junction", ["J5", "U6", "U7", "R43", "R44", "R45", "C12", "C13", "C14"], False),
    ("Pressure transducer", ["J6", "R16", "R17", "C9"], False),
    ("Front panel: switches + lamps", ["J7", "R18", "R19", "R20", "R21", "R28", "R29", "R30", "R31", "R32"], False),
    ("Indicator LEDs", ["D3", "D4", "D5", "D6", "D7", "D8", "R37", "R38", "R39", "R40", "R41", "R42"], False),
    ("Drip-tray scale (dual HX711)", ["J10", "R36"], False),
    ("Rail sense (+5V)", ["R24", "R25", "C11"], False),
]
POWER = {"GND", "+5V", "+3V3", "VBUS"}


def part_svg(ref, p, x, y, mains_nets):
    pins = [(n, p["pin_names"].get(n, n), p["pins"].get(n)) for n in sorted(p["pins"], key=lambda k: (len(k), k))]
    pins = [t for t in pins if not t[1].startswith(("NC", "EPAD", "GND2"))] if ref == "U4" else pins
    if ref == "U4":   # only the connected pins of the module
        pins = [t for t in pins if t[2]]
    half = (len(pins) + 1) // 2
    left, right = pins[:half], pins[half:]
    rows = max(len(left), len(right), 1)
    w, h = 132, 34 + rows * 18
    is_mains = bool(set(p.get("mains") or []))
    stroke = "#c2372f" if is_mains else "#3a3a38"
    s = [f'<g><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="#fff" stroke="{stroke}" stroke-width="1.4"/>',
         f'<text x="{x + w / 2}" y="{y + 15}" text-anchor="middle" font-weight="700" font-size="12">{ref}</text>',
         f'<text x="{x + w / 2}" y="{y + 28}" text-anchor="middle" font-size="10" fill="#55534f">{html.escape(p["value"][:22])}</text>']
    def pin(side, i, num, name, net):
        py = y + 42 + i * 18
        px = x if side == "L" else x + w
        stub = -14 if side == "L" else 14
        col = "#c2372f" if net in mains_nets else ("#1f5fae" if net in POWER else "#3a3a38")
        s.append(f'<line x1="{px}" y1="{py}" x2="{px + stub}" y2="{py}" stroke="{stroke}" stroke-width="1.2"/>')
        s.append(f'<text x="{px + (6 if side == "L" else -6)}" y="{py + 3.5}" text-anchor="{"start" if side == "L" else "end"}" font-size="10">{html.escape(name)}<tspan fill="#8a877f"> {num}</tspan></text>')
        label = net or "nc"
        tw = 6.2 * len(label) + 10
        tx = px + stub - (tw if side == "L" else 0)
        fill = "#fdecea" if net in mains_nets else ("#e8f0fb" if net in POWER else ("#f2f1ee" if net else "#fff"))
        s.append(f'<rect x="{tx}" y="{py - 8}" width="{tw}" height="16" rx="8" fill="{fill}" stroke="{col}" stroke-width="0.8"/>')
        s.append(f'<text x="{tx + tw / 2}" y="{py + 3.5}" text-anchor="middle" font-size="10" fill="{col}" font-family="ui-monospace,Menlo,monospace">{html.escape(label)}</text>')
    for i, (n, nm, net) in enumerate(left):
        pin("L", i, n, nm, net)
    for i, (n, nm, net) in enumerate(right):
        pin("R", i, n, nm, net)
    s.append("</g>")
    return "\n".join(s), w, h


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mains", type=int, default=120)
    a = ap.parse_args()
    spec = json.load(open(os.path.join(HERE, "build", f"parts-{a.mains}v.json")))
    parts, mains_nets = spec["parts"], set(spec["mains_nets"])
    placed = {r for _, refs, _ in BLOCKS for r in refs}
    rest = [r for r in parts if r not in placed and not r.startswith("H")]
    blocks = BLOCKS + ([("Other", rest, False)] if rest else [])
    W, cell_w, margin = 1900, 375, 30
    out, y = [], 70
    for title, refs, hazard in blocks:
        refs = [r for r in refs if r in parts]
        if not refs:
            continue
        cols = max(1, (W - 2 * margin) // cell_w)
        x0, bx, by, rowh, items = margin + 20, 0, 0, 0, []
        for r in refs:
            svg, w, h = part_svg(r, parts[r], x0 + 110 + bx * cell_w, y + 40 + by, mains_nets)
            items.append(svg)
            rowh = max(rowh, h)
            bx += 1
            if bx >= cols - 1:
                bx, by, rowh = 0, by + rowh + 26, 0
        block_h = by + rowh + 60
        col = "#c2372f" if hazard else "#8a877f"
        out.append(f'<rect x="{margin}" y="{y}" width="{W - 2 * margin}" height="{block_h}" rx="10" fill="{"#fffaf9" if hazard else "#fbfbfa"}" stroke="{col}" stroke-dasharray="{"6 4" if hazard else "0"}"/>')
        out.append(f'<text x="{margin + 14}" y="{y + 24}" font-size="15" font-weight="700" fill="{col}">{html.escape(title)}{" · MAINS" if hazard else ""}</text>')
        out += items
        y += block_h + 24
    H = y + 30
    head = (f'<text x="{margin}" y="38" font-size="22" font-weight="700">coffee controller v32 · {a.mains} V · net-label schematic</text>'
            f'<text x="{margin}" y="58" font-size="12" fill="#55534f">Pins carry net-name tags; equal names are connected. Red = mains (hazardous). Blue = power rails. Source: hardware/controller/controller.py</text>')
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" font-family="system-ui,Segoe UI,sans-serif">'
           f'<rect width="100%" height="100%" fill="#ffffff"/>{head}{"".join(out)}</svg>')
    path = os.path.join(HERE, "build", f"schematic-{a.mains}v.svg")
    open(path, "w").write(svg)
    print("wrote", path)


if __name__ == "__main__":
    main()
