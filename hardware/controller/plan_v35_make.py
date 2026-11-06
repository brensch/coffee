"""Generator for plan_v35.json (the v35 placement, from scratch). Run from hardware/controller: python3 plan_v35_make.py"""
import json, subprocess, sys, os
HC = "/home/brensch/coffee/hardware/controller"
S = os.environ.get("PLACE_PNG_DIR", "/tmp")
base = json.load(open(f"{HC}/plan_v34.json"))
W, H = 58.0, 70.0
BAND = [35.0, 42.0]
P = {}
def put(r, x, y, rot=0, side="F"): P[r] = [round(x, 2), round(y, 2), rot, side]
V = 90   # vertical 0603/0805/2512
# ================= big parts
x0, y0 = 24.0, 52.0                  # IRM on the underside, rot 90: AC pins (L_F x0-10.75, N x0) at y0, DC pins (GND x0-10.75, +5V x0-2.75) at y0-38.5
put("PS1", x0, y0, 90, "B")
put("U4", 30.0, 20.5, 180)           # module: CMD/LED pins left, 3V3/EN/V5/DOUT right, P_ADC/switches/USB/I2C on top
bc = sum(BAND) / 2
put("U8", 36.0, bc, 270); put("U9", 52.3, bc, 270)    # optos straddle the band, SELV pins up
# ================= top edge: signal plugs, left -> right J5 TC | J2 SSR | J7 FRONT | J10 SCALE | J6 PRESSURE
for r, x in (("J5", 4.68), ("J2", 14.48), ("J7", 26.78), ("J10", 39.71), ("J6", 50.76)): put(r, x, 3.2, 180)
# ================= bottom edge: mains spades N L P 3
for r, x in (("J12", 6.0), ("J13", 18.0), ("J14", 30.5), ("J15", 46.5)): put(r, x, H - 4.0, 0)
# ================= right edge: USB (recovery), below the plugs
put("J4", W - 3.2, 17.0, 90)
put("D2", 48.0, 13.5, V); put("R13", 47.0, 18.0, V); put("R14", 48.6, 18.0, V)
# ================= thermocouple front end under J5 (TMP117 at J5's pads)
put("U7", 4.5, 8.3, 270); put("C14", 1.5, 8.3, V)
put("U6", 4.5, 13.5, V); put("C13", 1.5, 13.5, V)
put("R43", 3.0, 18.0, V); put("R44", 6.0, 18.0, V); put("C12", 4.5, 21.5, 0); put("R45", 1.5, 21.5, V)
# ================= heater FET under J2
put("Q1", 13.0, 8.5, 0); put("R4", 16.5, 8.5, V)
# ================= front panel parts under J7 (switch series R, LED R)
for i, r in enumerate(("R18", "R19", "R30", "R31", "R32")): put(r, 22.6 + 1.6 * i, 8.3, V)
put("R20", 33.4, 8.3, V); put("R21", 35.0, 8.3, V)      # switch pull-ups toward the module
# ================= scale + I2C pull-ups under J10
put("R36", 38.5, 8.3, V); put("R11", 41.5, 8.3, V); put("R46", 43.1, 8.3, V)
# ================= pressure divider under J6
put("R16", 48.8, 8.3, V); put("C9", 50.4, 8.3, V); put("R17", 52.0, 8.3, V)
# ================= power: LDO next to the IRM's +5V pin (21.25, 13.5)
put("U5", 17.0, 20.0, V); put("C6", 21.9, 18.0, V); put("C3", 12.1, 20.0, V)
put("C7", 37.8, 18.5, V); put("C4", 37.8, 22.0, V)
put("R10", 39.5, 14.0, V); put("C5", 41.1, 14.0, V)
put("R24", 39.5, 25.5, V); put("R25", 41.1, 25.5, V); put("C11", 42.7, 25.5, V)
put("TP1", 6.0, 26.0, 0, "B")
# ================= LED row (bottom-left of the signal half), labels above
leds = [("D1", "R12"), ("D3", "R37"), ("D4", "R38"), ("D5", "R39"), ("D6", "R40"), ("D7", "R41"), ("D8", "R42")]
for i, (d, r) in enumerate(leds):
    put(d, 9.0 + 1.9 * i, 28.0, V); put(r, 9.0 + 1.9 * i, 31.3, V)
# ================= output FETs above the optos' SELV pins
put("Q2", 33.5, 29.5, 0); put("R5", 30.5, 29.5, V); put("R47", 39.5, 30.0, V)
put("Q3", 49.8, 29.5, 0); put("R6", 46.8, 29.5, V); put("R48", 55.8, 30.0, V)
# ================= mains: channels under the optos (gate lead under opto pin 4), snubbers beside
# one column per channel under opto pin 6: R49/R50 (O6 up, AC_L_F down) | R51/R52 (AC_L_F up) | C15/C16 (SNUB up, SW down)
for (rr, rs, cs, x) in (("R49", "R51", "C15", 38.5), ("R50", "R52", "C16", 54.5)):
    put(rr, x, 46.6, V); put(rs, x, 52.7, 270); put(cs, x, 60.4, 270)
put("Q5", 31.2, 51.5, 90); put("Q6", 47.5, 51.5, 90)
put("R53", 47.5, 59.8, 180)
put("R54", 27.0, 60.0, 0)
put("F1", 15.5, 58.0, 180)
q = dict(base)
q["W"], q["H"] = W, H
q["place"] = P
m = dict(base["mains"]); m["band"] = BAND; m["band_axis"] = "y"
bc = (BAND[0] + BAND[1]) / 2
m["slots"] = [[P[u][0] - 2.35, bc - 0.5, P[u][0] + 2.35, bc + 0.5] for u in ("U8", "U9")]
q["mains"] = m
q["gnd_rect"] = [0.3, 0.3, W - 0.3, BAND[0] - 0.3]
q["pours"] = []
q["label_pos"] = {f"L{i}": [round(9.0 + 1.9 * i, 2), 25.8, 0, c] for i, c in enumerate("SPCSHPV")}
q["bottom_text"] = [["coffee v35", 46.0, 28.0, 1.0]]
for r, t in (("J12", "N"), ("J13", "L"), ("J14", "P"), ("J15", "3")):   # top-side letters where you plug in
    q["label_pos"][f"M{t}"] = [P[r][0], P[r][1] - 2.6, 0, t]
labels = dict(base["labels"]); labels["J7"] = ["FRONT", ["BREW", "STM", "GND", "LP", "LC", "LS"]]; labels["J10"] = ["SCALE", ["D1", "SCK", "GND", "D2", "3V3"]]
q["labels"] = labels
q["logo"] = [48.5, 52.0, 20.0]   # the bean, big, on the back (lower right: clear of the IRM and the spades)
q["fp_silk_off"] = ["PS1"]   # its underside outline would cross the opto slots; the square pin-1 pad shows orientation
json.dump(q, open(f"{HC}/plan_v35.json", "w"), indent=1)
env = dict(os.environ, PLACE_JSON="plan_v35.json")
out = subprocess.run(f"source ~/opt/kienv.sh && python3 check_place.py {S}/place.png", shell=True, cwd=HC, env=env, capture_output=True, text=True, executable="/bin/bash")
print("\n".join(l for l in (out.stdout + out.stderr).splitlines() if not l.startswith("Warn") and "swig" not in l)[-3000:])
