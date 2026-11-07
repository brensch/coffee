#!/usr/bin/env python3
"""Assembly views for checking the holder by eye: out/assembly.glb (board seated, retaining bar in) and
out/exploded.glb (board 60 mm above its slot as it slides in, bar out to the side). Uses the cad venv (trimesh):
    ~/opt/cad-venv/bin/python assemble.py
"""
import os, trimesh
HERE = os.path.dirname(os.path.abspath(__file__))
BOARD_H, ZB = 77.0, 8.0          # keep in step with holder.py
board_glb = os.path.join(HERE, "..", "controller", "versions", "v39-front-plate", "board.glb")
for name, lift, bar_dx in (("assembly", 0.0, 0.0), ("exploded", 60.0, 40.0)):
    sc = trimesh.Scene()
    h = trimesh.load(os.path.join(HERE, "out", "holder.stl")); h.visual.face_colors = [239, 138, 60, 255]
    sc.add_geometry(h, node_name="holder")
    bar = trimesh.load(os.path.join(HERE, "out", "holder_bar.stl")); bar.apply_translation([bar_dx, 0, 0])
    bar.visual.face_colors = [90, 90, 96, 255]; sc.add_geometry(bar, node_name="bar")
    b = trimesh.load(board_glb)
    b.apply_translation([0, BOARD_H + lift, ZB])     # board GLB: y = -board y, z up from its back face
    for g in b.dump():
        sc.add_geometry(g)
    sc.export(os.path.join(HERE, "out", f"{name}.glb"))
print("wrote out/assembly.glb, out/exploded.glb")
