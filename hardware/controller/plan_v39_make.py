"""Generator for plan_v39.json: v38 turned 180 degrees for the front-plate holder (hardware/holder), then tuned.
Run from hardware/controller: python3 plan_v39_make.py   (picture: $PLACE_PNG_DIR/place.png)

Why turned: the holder (user's sketch, 2026-11-07) is a back plate and a front plate joined at the sides. The
board slides in from the top and stands on the floor. The mains spades sit along the TOP edge and poke out
through a comb in the front plate; the IRM pokes out through a channel open to the top; the low-voltage plugs
sit along the BOTTOM edge and their leads leave down through the open bottom. Turning v38 by 180 degrees gives
exactly that, and keeps every proven local arrangement (triac channels, optos across the band, TC filter).
It also puts the thermocouple terminal in the bottom-right corner: heat rises, so the bottom edge is the
coolest place on a vertical board, and the terminal ends up below every heat source.

Then:
- The 3.3 V regulator (U5, C6, C3) moves from beside the TC block to beside the ESP32 module it feeds.
- The TC block tightens around the terminal: TMP117 (U7) centred on the terminal's two pins, just past its body.
"""
import json, os, subprocess
HC = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("PLACE_PNG_DIR", "/tmp")
base = json.load(open(f"{HC}/plan_v38.json"))
W, H = base["W"], base["H"]
V = 90


def turn(p):
    x, y, rot, side = p
    return [round(W - x, 2), round(H - y, 2), (rot + 180) % 360, side]


P = {r: turn(p) for r, p in base["place"].items()}
def put(r, x, y, rot=0, side="F"): P[r] = [round(x, 2), round(y, 2), rot, side]

# ---- regulator beside the module (v38 had it under the TC block; turned, it sat just above the terminal). The free
# patch right of the module, below the LED row: close to the IRM's DC pins (short 5 V run), ~20 mm from the TC block
put("U5", 42.4, 62.6, 0)                 # SOT-223 lying flat
put("C6", 48.8, 60.9, 0)                 # +5V in
put("C3", 48.8, 64.3, 0)                 # +3V3 out
# ---- the terminal 1.7 mm further in from the right edge: turned, its body reached the groove strip (courtyard to x 79.1),
# and at the bottom it slides past the whole groove on the way in
P["J5"][0] = round(P["J5"][0] - 1.7, 2)
# ---- TMP117 centred on the terminal's pins (after the turn they're at jx and jx + 5.08), just past its body
jx = P["J5"][0]
put("U7", jx + 2.54, H - 11.8, 180)
put("C14", jx - 0.6, H - 11.8, 180)      # beside U7's supply pin

BAND = [round(H - base["mains"]["band"][1], 2), round(H - base["mains"]["band"][0], 2)]   # [36, 43]
bc = sum(BAND) / 2
q = dict(base)
q["place"] = P
m = dict(base["mains"]); m["band"] = BAND
m["slots"] = [[P[u][0] - 2.35, bc - 0.5, P[u][0] + 2.35, bc + 0.5] for u in ("U8", "U9")]
q["mains"] = m
q["gnd_rect"] = [0.3, BAND[1] + 0.3, W - 0.3, H - 0.3]        # SELV is now below the band
q["label_pos"] = {k: [round(W - v[0], 2), round(H - v[1], 2), v[2], v[3]] for k, v in base["label_pos"].items()}
for k in ("MN", "ML", "MP", "M3"):                               # spade letters below their tabs (inside, under the comb)
    q["label_pos"][k][1] = round(P[{"MN": "J12", "ML": "J13", "MP": "J14", "M3": "J15"}[k]][1] + 4.4, 2)
for k in [k for k in q["label_pos"] if k.startswith("L")]:      # LED letters below the turned LED row (resistors are above it)
    q["label_pos"][k][1] = round(P["D8"][1] + 2.4, 2)
q["bottom_text"] = [["coffee v39", round(W - 60.0, 2), round(H - 50.0, 2), 1.0]]
q["logo"] = [round(W - base["logo"][0], 2), round(H - base["logo"][1], 2), base["logo"][2]]
json.dump(q, open(f"{HC}/plan_v39.json", "w"), indent=1)
env = dict(os.environ, PLACE_JSON="plan_v39.json", BUILD_DIR=os.environ.get("BUILD_DIR", f"{HC}/build/v39"))
r = subprocess.run(f"source ~/opt/kienv.sh && python3 check_place.py {OUT}/place.png", shell=True, cwd=HC, env=env,
                   capture_output=True, text=True, executable="/bin/bash")
print("\n".join(l for l in (r.stdout + r.stderr).splitlines() if not l.startswith("Warn") and "swig" not in l))
