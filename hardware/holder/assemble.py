#!/usr/bin/env python3
"""Assembly views for checking the holder by eye: out/assembly.glb (board seated, pegs in) and out/exploded.glb
(board 70 mm below, coming in from the bottom; pegs out in front). Uses the cad venv (trimesh):
    ~/opt/cad-venv/bin/python assemble.py
"""
import os, trimesh
HERE = os.path.dirname(os.path.abspath(__file__))
BOARD_H, ZB = 77.0, 8.0          # keep in step with holder.py
board_glb = os.path.join(HERE, "..", "controller", "versions", "v39-front-plate", "board.glb")
for name, drop, peg_dz in (("assembly", 0.0, 0.0), ("exploded", -70.0, 40.0)):
    sc = trimesh.Scene()
    h = trimesh.load(os.path.join(HERE, "out", "holder.stl")); h.visual.face_colors = [239, 138, 60, 255]
    sc.add_geometry(h, node_name="holder")
    pg = trimesh.load(os.path.join(HERE, "out", "holder_peg.stl")); pg.apply_translation([0, 0, peg_dz])
    pg.visual.face_colors = [90, 90, 96, 255]; sc.add_geometry(pg, node_name="pegs")
    b = trimesh.load(board_glb)
    b.apply_translation([0, BOARD_H + drop, ZB])     # board GLB: y = -board y, z up from its back face
    for g in b.dump():
        sc.add_geometry(g)
    sc.export(os.path.join(HERE, "out", f"{name}.glb"))
print("wrote out/assembly.glb, out/exploded.glb")
