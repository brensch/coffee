#!/usr/bin/env python3
"""3D model of the board: STEP (all parts) -> GLB for the web viewer.

    source ~/opt/kienv.sh && python3 render3d.py board.kicad_pcb out_dir
Needs the KiCad 3D models (STEP) under $MODELS (default ~/opt/kicad3d) and
~/opt/cad-venv with cascadio + trimesh. Parts with no KiCad model (the
TR5 fuse, the disc varistor) get simple stand-in shapes. The ESP32-C6 model comes from
Espressif's KiCad library (${KICAD8_3RD_PARTY} path), expected at $MODELS/espressif.3dshapes.
"""
import os, subprocess, sys, json
import pcbnew

MODELS = os.environ.get("MODELS", os.path.expanduser("~/opt/kicad3d"))
pcb, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
b = pcbnew.LoadBoard(pcb)


def local_model(path):
    """KiCad model path -> path under $MODELS (KiCad library .wrl -> .step; Espressif STEP as is)."""
    if "espressif.3dshapes/" in path:
        return "espressif.3dshapes/" + path.split("espressif.3dshapes/", 1)[1]
    return path.split("}/", 1)[-1].replace(".wrl", ".step")


missing = []
for fp in b.GetFootprints():
    for i in range(len(fp.Models())):
        rel = local_model(fp.Models()[i].m_Filename)
        if not os.path.exists(os.path.join(MODELS, rel)):
            missing.append(fp.GetReference())
# Point the model paths at the local STEP files (text edit: pcbnew's Python hands out copies).
txt = open(pcb).read()
import re
txt = re.sub(r'\(model "(\$\{KICAD[0-9A-Z_]+\}/[^"]+)"', lambda m: '(model "%s/%s"' % (MODELS, local_model(m.group(1))), txt)
tmp = os.path.join(out, "_3d.kicad_pcb")
open(tmp, "w").write(txt)
step = os.path.join(out, "board.step")
r = subprocess.run(["kicad-cli", "pcb", "export", "step", "-f", "-o", step, tmp], capture_output=True, text=True)
os.remove(tmp)
for ext in (".kicad_pro", ".kicad_prl"):
    p = tmp.replace(".kicad_pcb", ext)
    if os.path.exists(p):
        os.remove(p)
# stand-ins for parts without models: (ref, kind, size)
extra = []
for fp in b.GetFootprints():
    ref = fp.GetReference()
    if ref not in missing:
        continue
    pads = list(fp.Pads())   # stand-ins sit on the pad centroid, whatever the rotation
    x = sum(pcbnew.ToMM(p.GetPosition().x) for p in pads) / len(pads)
    y = sum(pcbnew.ToMM(p.GetPosition().y) for p in pads) / len(pads)
    vertical = round(fp.GetOrientationDegrees()) % 180 == 90
    if ref.startswith("U"):     # ESP32-C6-WROOM-1U: 18 x 19.2 x 3.2 shielded can
        extra.append(dict(ref=ref, kind="box", x=pcbnew.ToMM(fp.GetPosition().x), y=pcbnew.ToMM(fp.GetPosition().y), sx=18.0, sy=19.2, sz=3.2))
    elif ref.startswith("F"):   # TR5 fuse: d8.5 x 8 mm
        extra.append(dict(ref=ref, kind="cyl", x=x, y=y, r=4.25, sz=8.0))
    elif ref.startswith("RV"):  # 7 mm disc varistor standing on its leads
        extra.append(dict(ref=ref, kind="disc", x=x, y=y, sx=4.2 if vertical else 7.5, sy=7.5 if vertical else 4.2, sz=9.0))
json.dump(extra, open(os.path.join(out, "standins.json"), "w"))
py = os.path.expanduser("~/opt/cad-venv/bin/python")
subprocess.run([py, "-c", r'''
import sys, json, cascadio, trimesh, numpy as np
step, glb, extra = sys.argv[1], sys.argv[2], json.load(open(sys.argv[3]))
cascadio.step_to_glb(step, glb, 0.05, 0.3)
scene = trimesh.load(glb)
scene.apply_scale(1000.0)   # cascadio writes metres; work in mm (z up, board top at z=1.6)
for e in extra:
    if e["kind"] == "box":
        m = trimesh.creation.box(extents=[e["sx"], e["sy"], e["sz"]]); colour = [190, 190, 195, 255]
    elif e["kind"] == "disc":
        m = trimesh.creation.box(extents=[e["sx"], e["sy"], e["sz"]]); colour = [30, 80, 160, 255]
    else:
        m = trimesh.creation.cylinder(radius=e["r"], height=e["sz"]); colour = [40, 40, 40, 255]
    # KiCad STEP: x right, y up (board y flipped), z up from the board bottom; board top at z=1.6
    m.apply_translation([e["x"], -e["y"], 1.6 + e["sz"] / 2])
    m.visual.face_colors = colour
    scene.add_geometry(m, node_name=e["ref"])
scene.export(glb)
''', step, os.path.join(out, "board.glb"), os.path.join(out, "standins.json")], check=True)
print(f"wrote {out}/board.step, board.glb ({len(missing)} parts with stand-ins: {', '.join(sorted(set(missing)))})")
