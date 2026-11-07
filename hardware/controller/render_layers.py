#!/usr/bin/env python3
"""Silkscreen and copper layers of a board as SVGs, for the 3D viewer (out/view/3d.html) to lay over the
STEP model (KiCad 7's STEP export has no silk or copper).

    source ~/opt/kienv.sh && python3 render_layers.py board.kicad_pcb out_dir
    source ~/opt/kienv.sh && python3 render_layers.py --all        # every versions/*/ -> out/view/layers/<v>/

Writes <out>/{silk,tracks,pour}-{top,bottom}.svg and layers.json. KiCad's own SVG plotter draws them
(black and white, page coordinates in mm); each viewBox is then cropped to the board outline, so every
image covers exactly the board. `tracks` is the copper with the zones taken out (tracks, vias, pads);
`pour` is all the copper, zones included. The viewer colours the black fills.
"""
import glob, json, os, re, subprocess, sys, tempfile

import pcbnew

SIDES = {"top": ("F.SilkS", "F.Cu"), "bottom": ("B.SilkS", "B.Cu")}
PX_PER_MM = 40


def plot(pcb, layer, out, box):
    subprocess.run(["kicad-cli", "pcb", "export", "svg", "-l", layer, "--black-and-white", "--exclude-drawing-sheet",
                    "--page-size-mode", "1", "--drill-shape-opt", "0", "-o", out, pcb], check=True, capture_output=True)
    x0, y0, x1, y1 = box
    svg = open(out).read()
    svg = re.sub(r'width="[^"]+" height="[^"]+" viewBox="[^"]+"',
                 f'width="{(x1 - x0) * PX_PER_MM:.0f}" height="{(y1 - y0) * PX_PER_MM:.0f}" '
                 f'viewBox="{x0:.4f} {y0:.4f} {x1 - x0:.4f} {y1 - y0:.4f}"', svg, count=1)
    open(out, "w").write(svg)


def export(pcb, out):
    os.makedirs(out, exist_ok=True)
    b = pcbnew.LoadBoard(pcb)
    bb = b.GetBoardEdgesBoundingBox()
    box = tuple(pcbnew.ToMM(v) for v in (bb.GetLeft(), bb.GetTop(), bb.GetRight(), bb.GetBottom()))
    thick = pcbnew.ToMM(b.GetDesignSettings().GetBoardThickness())
    for z in list(b.Zones()):
        b.Remove(z)
    with tempfile.TemporaryDirectory() as tmp:
        bare = os.path.join(tmp, "nozones.kicad_pcb")
        b.Save(bare)
        for side, (silk, cu) in SIDES.items():
            plot(pcb, silk, os.path.join(out, f"silk-{side}.svg"), box)
            plot(pcb, cu, os.path.join(out, f"pour-{side}.svg"), box)
            plot(bare, cu, os.path.join(out, f"tracks-{side}.svg"), box)
    x0, y0, x1, y1 = box
    json.dump(dict(x0=x0, y0=y0, x1=x1, y1=y1, thickness=thick),
              open(os.path.join(out, "layers.json"), "w"))
    return box


if __name__ == "__main__":
    here = os.path.dirname(os.path.abspath(__file__))
    if sys.argv[1:] == ["--all"]:
        for pcb in sorted(glob.glob(os.path.join(here, "versions", "*", "board.kicad_pcb"))):
            v = os.path.basename(os.path.dirname(pcb))
            out = os.path.join(here, "..", "..", "out", "view", "layers", v)
            # one process per board: pcbnew won't load a second board after zones were removed from the first
            r = subprocess.run([sys.executable, __file__, pcb, out], capture_output=True, text=True)
            print(v, r.stdout.strip() if r.returncode == 0 else "FAILED: " + r.stderr.strip().splitlines()[-1], flush=True)
    else:
        print(export(sys.argv[1], sys.argv[2]))
