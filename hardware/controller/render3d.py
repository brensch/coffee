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


# Models KiCad's footprint names but kicad-packages3D never published: a part with the same body.
# HRO TYPE-C-31-M-12 -> GCT USB4105: both 16-pin top-mount receptacles, fab body +-4.47 x +-3.65/3.67 mm
# on the same origin, shell tabs in the same places.
SAME_BODY = {"Connector_USB.3dshapes/USB_C_Receptacle_HRO_TYPE-C-31-M-12.step":
             "Connector_USB.3dshapes/USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal.step"}


def local_model(path):
    """KiCad model path -> path under $MODELS (KiCad library .wrl -> .step; Espressif STEP as is)."""
    if "espressif.3dshapes/" in path:
        return "espressif.3dshapes/" + path.split("espressif.3dshapes/", 1)[1]
    rel = path.split("}/", 1)[-1].replace(".wrl", ".step")
    return SAME_BODY.get(rel, rel)


missing = []
for fp in b.GetFootprints():
    if fp.GetReference().startswith(("TP", "H")):
        continue                            # bare pads / holes: nothing to draw
    if len(fp.Models()) == 0:
        missing.append(fp.GetReference())   # no 3D model in the footprint at all
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
# stand-ins for parts without models: a box over the courtyard (see below)
extra = []
_fitted = {r: v.get("populate", "Y") == "Y" for r, v in json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "build", "parts-120v.json")))["parts"].items()}
for fp in b.GetFootprints():
    ref = fp.GetReference()
    if ref not in missing or not _fitted.get(ref, True):    # bare pads (J4 programming) get no stand-in
        continue
    pads = list(fp.Pads())   # stand-ins sit on the pad centroid, whatever the rotation
    x = sum(pcbnew.ToMM(p.GetPosition().x) for p in pads) / len(pads)
    y = sum(pcbnew.ToMM(p.GetPosition().y) for p in pads) / len(pads)
    # v32: a box over the part's own courtyard (less 0.25 mm a side), at a height for its kind. (Before,
    # every model-less "U" got the ESP32 module's 18 x 19.2 mm can: a 2 x 2 mm TMP117 drew as a slab.)
    fp.BuildCourtyardCaches()
    cb = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd).BBox()
    x0, y0, x1, y1 = (pcbnew.ToMM(v) for v in (cb.GetLeft(), cb.GetTop(), cb.GetRight(), cb.GetBottom()))
    sx, sy = max(x1 - x0 - 0.5, 0.5), max(y1 - y0 - 0.5, 0.5)
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    HEIGHT = {"PS": 21.5,   # Mean Well IRM-05: 45.7 x 25.4 x 21.5 mm
              "J": 6.0, "F": 3.0, "RV": 9.0, "C": 1.2, "R": 0.6, "D": 1.0, "Q": 1.8, "U": 1.0}
    kind = next(k for k in sorted(HEIGHT, key=len, reverse=True) if ref.startswith(k)) if any(ref.startswith(k) for k in HEIGHT) else None
    sz = HEIGHT.get(kind, 2.0)
    colour = "disc" if kind == "J" else "box"
    extra.append(dict(ref=ref, kind=colour, x=cx, y=cy, sx=sx, sy=sy, sz=sz, bottom=fp.IsFlipped()))   # v35: the IRM hangs underneath
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
    m.apply_translation([e["x"], -e["y"], (-e["sz"] / 2) if e.get("bottom") else (1.6 + e["sz"] / 2)])
    m.visual.face_colors = colour
    scene.add_geometry(m, node_name=e["ref"])
scene.export(glb)
''', step, os.path.join(out, "board.glb"), os.path.join(out, "standins.json")], check=True)
print(f"wrote {out}/board.step, board.glb ({len(missing)} parts with stand-ins: {', '.join(sorted(set(missing)))})")
