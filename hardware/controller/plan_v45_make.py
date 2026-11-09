"""Generator for plan_v45.json: v44 with the IRM supply 3.8 mm further from the spades, so the holder's cover can close
over the top of it. The thermocouple amplifier's parts make way. Run from hardware/controller: python3 plan_v45_make.py"""
import json, os, subprocess
HC = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("PLACE_PNG_DIR", "/tmp")
q = json.load(open(f"{HC}/plan_v44.json"))
p = q["place"]
p["PS1"][1] = round(p["PS1"][1] + 3.8, 2)
for r in ("U6", "C13"):
    p[r][1] = round(p[r][1] + 0.45, 2)
p["C12"][1] = round(p["C12"][1] + 0.2, 2)
p["R45"] = [60.7, 59.8, 270, "F"]
p["R43"] = [69.0, 65.2, 0, "F"]
q["bottom_text"] = [["coffee v45"] + t[1:] for t in q["bottom_text"]]
json.dump(q, open(f"{HC}/plan_v45.json", "w"), indent=1)
env = dict(os.environ, PLACE_JSON="plan_v45.json", BUILD_DIR=os.environ.get("BUILD_DIR", f"{HC}/build/v45"))
os.makedirs(env["BUILD_DIR"], exist_ok=True)
r = subprocess.run(f"source ~/opt/kienv.sh && python3 check_place.py {OUT}/place.png", shell=True, cwd=HC, env=env, capture_output=True, text=True, executable="/bin/bash")
print("\n".join(l for l in (r.stdout + r.stderr).splitlines() if not l.startswith("Warn") and "swig" not in l))
