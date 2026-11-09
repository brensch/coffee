"""Generator for plan_v44.json: v39 narrowed from 80 to 71.2 mm (the board now sits flat in the holder's recess).
Nothing sat left of x 7.4 and the 2.5 mm side strips for the old grooves are gone, so every part moves 6.8 mm left
and the outline shrinks; relative placement is unchanged. Run from hardware/controller: python3 plan_v44_make.py"""
import json, os, subprocess
HC = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("PLACE_PNG_DIR", "/tmp")
q = json.load(open(f"{HC}/plan_v39.json"))
DX, W = 6.8, 71.2
for p in q["place"].values():
    p[0] = round(p[0] - DX, 2)
q["W"] = W
q["mains"]["slots"] = [[a - DX, b, c - DX, d] for a, b, c, d in q["mains"]["slots"]]
q["gnd_rect"] = [0.3, q["gnd_rect"][1], W - 0.3, q["gnd_rect"][3]]
for v in q["label_pos"].values():
    v[0] = round(v[0] - DX, 2)
q["bottom_text"] = [["coffee v44", round(t[1] - DX, 2)] + t[2:] for t in q["bottom_text"]]
q["logo"][0] = round(q["logo"][0] - DX, 2)
q.pop("rail_mm", None)
json.dump(q, open(f"{HC}/plan_v44.json", "w"), indent=1)
env = dict(os.environ, PLACE_JSON="plan_v44.json", BUILD_DIR=os.environ.get("BUILD_DIR", f"{HC}/build/v44"))
r = subprocess.run(f"source ~/opt/kienv.sh && python3 check_place.py {OUT}/place.png", shell=True, cwd=HC, env=env, capture_output=True, text=True, executable="/bin/bash")
print("\n".join(l for l in (r.stdout + r.stderr).splitlines() if not l.startswith("Warn") and "swig" not in l))
