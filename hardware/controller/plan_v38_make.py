"""Generator for plan_v38.json: the board laid out for the slide-in holder (hardware/holder).
Run from hardware/controller: python3 plan_v38_make.py   (picture: $PLACE_PNG_DIR/place.png)

What changed from v37, and why:
- Everything on the front face, the IRM included: the back is flat (only through-hole pin tails), so the
  holder is a flat plate with magnets, and the board slides into it on its two side edges.
- RAIL strips: nothing within RAIL of the left and right edges on either face; the holder's grooves run there.
- Plugs and the thermocouple terminal on the top edge (all low-voltage cables leave upward); the four
  spades along the bottom edge (all mains wires leave downward). The band between them runs across the
  middle; the IRM stands on the left and straddles it (DC pins above, AC pins below).
- J5 is a screw terminal (Phoenix MKDS 1,5/2-5,08), wire entry facing up; U7 sits at its pads.
- The independent review's layout points (docs/REVIEW-v37.md): R54 >= 2 mm from Q5's tab (S1); PS1 out
  of the "pins of one part" exemption (S1's rule hole); R53 out of the pocket under the trunk.
"""
import json, os, subprocess
HC = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("PLACE_PNG_DIR", "/tmp")
base = json.load(open(f"{HC}/plan_v37.json"))
W, H = 80.0, 77.0
RAIL = 2.5                      # clear strip on the left and right edges (holder groove 1.5 + clearance)
BAND = [34.0, 41.0]             # isolation band (y), 7 mm
bc = sum(BAND) / 2
P = {}
def put(r, x, y, rot=0, side="F"): P[r] = [round(x, 2), round(y, 2), rot, side]
V = 90

# ================= top edge: plugs and the thermocouple terminal, all facing up (rot 180)
put("J5", 9.0, 4.6, 180)        # MKDS 2-pin, wire entry at the top edge
put("J2", 20.5, 3.2, 180)       # heater SSR
put("J7", 32.0, 3.2, 180)       # front panel
put("J10", 44.0, 3.2, 180)      # scale
put("J6", 54.5, 3.2, 180)       # pressure
put("J4", 65.5, 4.0, 180)       # USB-C (first flash / recovery), mouth up
# thermocouple front end under J5: cold junction U7 at the terminal's pads, the ADC and its filter column below
TC = 1.0                         # the cluster sits TC lower than v37's: J5's body reaches y 10.3
put("U7", 9.0, 11.6 + TC, 0); put("C14", 12.05, 11.6 + TC, 0)   # +3V3 pad (1) beside U7's V+ pin
put("U6", 6.4, 16.2 + TC, V); put("C13", 6.15, 13.3 + TC, 0)
put("R44", 3.9, 13.2 + TC, 270); put("C12", 3.9, 16.4 + TC, V); put("R43", 3.9, 19.6 + TC, V); put("R45", 9.6, 19.8 + TC, 180)
put("R11", 9.2, 16.2 + TC, V); put("R46", 10.8, 16.2 + TC, V)
# heater FET under J2
put("Q1", 18.6, 9.2, 0); put("R4", 22.0, 9.2, V)
# power: LDO between the IRM's DC pins and the module
put("U5", 21.0, 16.0, V); put("C6", 15.8, 17.6, V); put("C3", 26.6, 17.6, V)
# front panel parts under J7 (pins L->R: LS LC LP GND STEAM BREW)
for r, x in (("R32", 28.8), ("R31", 30.4), ("R30", 32.0)): put(r, x, 9.0, V)
for r, x in (("R19", 33.9), ("R18", 35.5)): put(r, x, 9.0, 270)
for r, x in (("R21", 37.1), ("R20", 38.7)): put(r, x, 9.0, 270)
# scale: SCK pull-down under the SCK pin
put("R36", 45.25, 9.0, 270)
# pressure divider under J6
put("R17", 52.5, 9.0, V); put("C9", 54.1, 9.0, V); put("R16", 55.7, 9.0, 270)
# USB parts under J4
put("D2", 61.0, 12.0, V); put("R13", 64.6, 12.0, V); put("R14", 66.2, 12.0, V)
# module and its parts
put("U4", 50.0, 20.5, 180)
put("C7", 59.8, 18.5, V); put("C4", 59.8, 22.0, V)
put("R10", 61.9, 16.5, V); put("C5", 63.5, 16.5, V)
put("R24", 61.9, 22.5, V); put("R25", 63.5, 22.5, V); put("C11", 65.1, 22.5, V)
# LED row right of the IRM, above the band: valve pump heater | power coffee steam | status
leds = [("D8", "R42"), ("D7", "R41"), ("D6", "R40"), ("D3", "R37"), ("D4", "R38"), ("D5", "R39"), ("D1", "R12")]
LX = [30.6 + 1.9 * i for i in range(7)]
for x, (d, r) in zip(LX, leds):
    put(d, x, 26.0, V); put(r, x, 29.3, V)
put("TP1", 70.0, 28.0, 0, "B")

# ================= IRM: front face, standing on the left, across the band. Origin = AC/N pin.
x0, y0 = 14.0, 65.0          # body y 22.6..68.6: clear of the TC cluster above and the spade row below
put("PS1", x0, y0, 90, "F")
# ================= mains: fuse beside the IRM, then the pump and valve channels; spades along the bottom edge
SPY = H - 4.5                    # spade row (y)
put("J12", 12.0, SPY, 0)         # N, under the IRM's N pin
put("F1", 31.5, 52.0, 90)        # AC_L in at the bottom (from L below), AC_L_F out at the top: the trunk runs right
put("J13", 31.5, SPY, 0)         # L, under the fuse
for (q, u, rr, rs, cs, j, xq) in (("Q5", "U8", "R49", "R51", "C15", "J14", 44.0), ("Q6", "U9", "R50", "R52", "C16", "J15", 62.0)):
    put(q, xq, bc + 12.81, 90)   # tab 2 mm below the opto's gate pin (as v33-v37)
    put(u, xq + 4.82, bc, 270)
    xc = xq + 7.36
    put(rr, xc, bc + 8.1, V); put(rs, xc, bc + 14.3, 270); put(cs, xc, bc + 22.0, 270)
    put(j, xq - 2.0, SPY, 0)     # output tab under the triac's MT1
put("R54", 37.3, 50.0, 270)      # pump bleeder, left of Q5: >= 2 mm from its tab (review S1)
put("R53", 62.0, 58.6, 180)      # valve gate-MT1, under Q6 between the pins it bridges (J15 is far below now)
# opto drive FETs above the optos' SELV pins
put("Q2", 47.5, 30.0, 0); put("R5", 44.4, 30.0, V); put("R47", 53.9, 30.0, V)
put("Q3", 65.5, 30.0, 0); put("R6", 62.4, 30.0, V); put("R48", 71.9, 30.0, V)

q = dict(base)
q["W"], q["H"] = W, H
q["place"] = P
m = dict(base["mains"]); m["band"] = BAND; m["band_axis"] = "y"
m["slots"] = [[P[u][0] - 2.35, bc - 0.5, P[u][0] + 2.35, bc + 0.5] for u in ("U8", "U9")]
m["parts"] = [r for r in m["parts"] if r != "PS1"]     # review S1: PS1's courtyard covered the mains half
q["mains"] = m
q["gnd_rect"] = [0.3, 0.3, W - 0.3, BAND[0] - 0.3]
q["pours"] = []
q["label_pos"] = {f"L{i}": [round(x, 2), 23.9, 0, c] for i, (x, c) in enumerate(zip(LX, "VPHPCSS"))}
q["label_pos"].update({"MN": [P["J12"][0], SPY - 4.4, 0, "N"], "ML": [P["J13"][0], SPY - 4.4, 0, "L"],
                       "MP": [P["J14"][0], SPY - 4.4, 0, "P"], "M3": [P["J15"][0], SPY - 4.4, 0, "3"]})
q["bottom_text"] = [["coffee v38", 60.0, 50.0, 1.0]]
q["logo"] = [24.0, 45.0, 20.0]          # the bean on the back, behind the IRM
q["fp_silk_off"] = ["PS1"]
q["rail_mm"] = RAIL
json.dump(q, open(f"{HC}/plan_v38.json", "w"), indent=1)
env = dict(os.environ, PLACE_JSON="plan_v38.json", BUILD_DIR=os.environ.get("BUILD_DIR", f"{HC}/build/v38"))
r = subprocess.run(f"source ~/opt/kienv.sh && python3 check_place.py {OUT}/place.png", shell=True, cwd=HC, env=env,
                   capture_output=True, text=True, executable="/bin/bash")
print("\n".join(l for l in (r.stdout + r.stderr).splitlines() if not l.startswith("Warn") and "swig" not in l))
