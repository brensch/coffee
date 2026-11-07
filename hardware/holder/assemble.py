#!/usr/bin/env python3
"""Assembly views for checking the holder by eye: out/assembly.glb (board seated) and out/exploded.glb (board
lifted 60 mm above its slot, as it slides in). Uses the cad venv (trimesh):
    ~/opt/cad-venv/bin/python assemble.py
"""
import os, numpy as np, trimesh
HERE = os.path.dirname(os.path.abspath(__file__))
BOARD_H, ZB = 77.0, 8.0          # keep in step with holder.py
board_glb = os.path.join(HERE, "..", "controller", "versions", "v38-slide-in", "board.glb")
for name, lift in (("assembly", 0.0), ("exploded", 60.0)):
    sc = trimesh.Scene()
    h = trimesh.load(os.path.join(HERE, "out", "holder.stl"))
    h.visual.face_colors = [235, 235, 238, 255]
    sc.add_geometry(h, node_name="holder")
    b = trimesh.load(board_glb)
    b.apply_translation([0, BOARD_H + lift, ZB])     # board GLB: y = -board y, z up from its back face
    for g in b.dump():                               # the board's parts, transforms applied
        sc.add_geometry(g)
    sc.export(os.path.join(HERE, "out", f"{name}.glb"))
print("wrote out/assembly.glb, out/exploded.glb")
