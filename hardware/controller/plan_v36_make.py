"""Generator for plan_v36.json (v36: mains input L/N on the left edge, outputs P/3 on the bottom edge).
Run from hardware/controller: python3 plan_v36_make.py   (picture: $PLACE_PNG_DIR/place.png)"""
import json, os, subprocess
HC = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("PLACE_PNG_DIR", "/tmp")
base = json.load(open(f"{HC}/plan_v35.json"))
W, H = 55.7, 61.0
BAND = [31.0, 38.0]
bc = sum(BAND) / 2
P = {}
def put(r, x, y, rot=0, side="F"): P[r] = [round(x, 2), round(y, 2), rot, side]
V = 90
# ================= IRM underneath (rot 90): AC pins L_F (x0-10.75, y0), N (x0, y0); DC GND (x0-10.75, y0-38.5), +5V (x0-2.75, ...)
x0, y0 = 20.25, 52.0
put("PS1", x0, y0, 90, "B")
# ================= mains input: L over N, vertical tabs on the left edge
put("J13", 4.0, 43.5, 90); put("J12", 4.0, 54.5, 90)
put("F1", 11.5, 44.7, 270)          # AC_L in at the top (from L), AC_L_F out at the bottom (by the IRM's L pin)
put("R54", 16.0, 44.5, 270)         # pump bleeder: AC_L_F top, PUMP_SW bottom
# ================= channels: triac (gate lead under opto pin 4), output tab under the triac, column under opto pin 6
for (q, u, rr, rs, cs, j, xq) in (("Q5", "U8", "R49", "R51", "C15", "J14", 27.0), ("Q6", "U9", "R50", "R52", "C16", "J15", 44.9)):
    put(q, xq, bc + 12.81, 90)   # tab 2 mm below the opto's gate pin
    put(u, xq + 4.82, bc, 270)
    xc = xq + 7.36
    put(rr, xc, 42.6, V); put(rs, xc, 48.8, 270); put(cs, xc, 56.5, 270)
    put(j, xq - 0.7, 57.5, 0)
put("R53", 38.95, 47.0, V)           # valve gate-MT1
# ================= signal plugs on the top edge
for r, x in (("J5", 4.08), ("J2", 12.68), ("J7", 23.78), ("J10", 35.51), ("J6", 45.36)): put(r, x, 3.2, 180)
put("J4", W - 3.2, 15.2, 90)        # USB: right edge, below the plugs
put("D2", 44.0, 14.0, V); put("R13", 43.0, 19.0, V); put("R14", 44.6, 19.0, V)
# thermocouple front end under J5
put("U7", 4.4, 8.3, 270); put("C14", 1.4, 8.3, V)
put("U6", 4.4, 13.3, V); put("C13", 1.4, 13.3, V)
put("R43", 2.9, 17.8, V); put("R44", 5.9, 17.8, V); put("C12", 4.4, 21.3, 0); put("R45", 1.4, 21.3, V)
# heater FET under J2
put("Q1", 12.0, 8.4, 0); put("R4", 15.5, 8.4, V)
# front panel parts under J7
for i, r in enumerate(("R18", "R19", "R30", "R31", "R32")): put(r, 19.4 + 1.6 * i, 8.3, V)
put("R20", 27.4, 8.3, V); put("R21", 29.0, 8.3, V)
# scale + I2C pull-ups under J10
put("R36", 31.4, 8.3, V); put("R11", 34.2, 8.3, V); put("R46", 35.8, 8.3, V)
# pressure divider under J6
put("R16", 42.6, 8.3, V); put("C9", 44.2, 8.3, V); put("R17", 45.8, 8.3, V)
# module and its parts
put("U4", 30.0, 20.0, 180)
put("C7", 37.8, 18.5, V); put("C4", 37.8, 22.0, V)
put("R10", 39.5, 14.0, V); put("C5", 41.1, 14.0, V)
put("R24", 39.6, 22.5, V); put("R25", 41.2, 22.5, V); put("C11", 42.8, 22.5, V)
# power: LDO below the IRM's +5V pin (x0-2.75, y0-38.5)
put("U5", 18.0, 21.5, V); put("C6", 20.6, 15.3, V); put("C3", 12.9, 21.0, V)
put("TP1", 2.5, 27.0, 0, "B")
# LED row (bottom-left of the signal half)
leds = [("D1", "R12"), ("D3", "R37"), ("D4", "R38"), ("D5", "R39"), ("D6", "R40"), ("D7", "R41"), ("D8", "R42")]
LX = [1.6 + 1.9 * i for i in range(7)]
for x, (d, r) in zip(LX, leds):
    put(d, x, 25.5, V); put(r, x, 28.8, V)
# output FETs above the optos' SELV pins
put("Q2", 29.3, 27.0, 0); put("R5", 26.3, 27.0, V); put("R47", 35.8, 27.15, V)
put("Q3", 45.8, 27.0, 0); put("R6", 42.8, 27.0, V); put("R48", 52.3, 27.15, V)
q = dict(base)
q["W"], q["H"] = W, H
q["place"] = P
m = dict(base["mains"]); m["band"] = BAND; m["band_axis"] = "y"
m["slots"] = [[P[u][0] - 2.35, bc - 0.5, P[u][0] + 2.35, bc + 0.5] for u in ("U8", "U9")]
q["mains"] = m
q["gnd_rect"] = [0.3, 0.3, W - 0.3, BAND[0] - 0.3]
q["pours"] = []
q["label_pos"] = {f"L{i}": [round(x, 2), 23.4, 0, c] for i, (x, c) in enumerate(zip(LX, "SPCSHPV"))}
q["label_pos"].update({"ML": [6.6, 43.5, 90, "L"], "MN": [6.6, 54.5, 90, "N"],
                       "MP": [P["J14"][0], P["J14"][1] - 2.6, 0, "P"], "M3": [P["J15"][0], P["J15"][1] - 2.6, 0, "3"]})
q["bottom_text"] = [["coffee v36", 44.0, 47.0, 1.0]]
q["logo"] = [40.0, 18.5, 20.0]          # the bean on the back, right of the IRM
q["fp_silk_off"] = ["PS1"]
json.dump(q, open(f"{HC}/plan_v36.json", "w"), indent=1)
env = dict(os.environ, PLACE_JSON="plan_v36.json")
r = subprocess.run(f"source ~/opt/kienv.sh && python3 check_place.py {OUT}/place.png", shell=True, cwd=HC, env=env,
                   capture_output=True, text=True, executable="/bin/bash")
print("\n".join(l for l in (r.stdout + r.stderr).splitlines() if not l.startswith("Warn") and "swig" not in l))
