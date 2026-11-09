#!/usr/bin/env python3
"""Assembly views for checking the holder by eye: out/assembly.glb (everything fitted) and out/exploded.glb (cover out
45 mm, bars slid out 40 mm sideways). Straight receptacles and their wires are drawn as the boxes holder.py checks.
    ~/opt/holder-venv/bin/python assemble.py
"""
import os, sys, trimesh
HERE = os.path.dirname(os.path.abspath(__file__)); sys.path.insert(0, HERE)
import holder as H
board_glb = os.path.join(HERE, "..", "controller", "versions", "v45-closed-top", "board.glb")
COL = {"holder_frame": [239, 138, 60, 255], "holder_cover": [70, 140, 190, 255], "holder_bars_assembled": [90, 90, 96, 255]}
for name, cover_dz, bar_dx in (("assembly", 0.0, 0.0), ("exploded", 45.0, -40.0)):
    sc = trimesh.Scene()
    for part, d in (("holder_frame", (0, 0, 0)), ("holder_cover", (0, 0, cover_dz)), ("holder_bars_assembled", (bar_dx, 0, 0))):
        m = trimesh.load(os.path.join(HERE, "out", f"{part}.stl")); m.apply_translation(d)
        m.visual.face_colors = COL[part]; sc.add_geometry(m, node_name=part)
    b = trimesh.load(board_glb); b.apply_translation([0, H.BOARD_H, H.ZB])
    for g in b.dump():
        sc.add_geometry(g)
    for n, x0, y0, z0, x1, y1, z1 in H.receptacles():
        z1 = min(z1, H.ROOF_Z + 15)
        m = trimesh.creation.box(extents=[x1 - x0, y1 - y0, z1 - z0]); m.apply_translation([(x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2])
        m.visual.face_colors = [200, 60, 40, 255]; sc.add_geometry(m, node_name=n)
    sc.export(os.path.join(HERE, "out", f"{name}.glb"))
print("wrote out/assembly.glb, out/exploded.glb")
