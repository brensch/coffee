# Board holder (v39)

One printed part that sticks to the inside of the machine's steel side panel with six magnets. It is a back plate
and a front plate joined by two side walls, open at the top. The controller board slides down between them in
two edge grooves and stands on the floor. A printed bar through both side walls, just above the board's top
edge, stops it being pulled up.

```
          mains wires leave UP                        retaining bar
   ┌──║───────╥──────────╥────────╥───────────────────┐ ═══════
   │  ║ [3]   ║   [P]    ║  [L]   ║   [N]  ┌────────┐ │   comb: a slot per spade tab
   │  ╨───────╨──────────╨────────╨        │        │ │
   │                                       │  IRM   │ │   supply channel (open to the top)
   │      front plate covers the board     │ supply │ │
   │      (optos, triacs, fuse, module)    │        │ │
   │           [LED window]                └────────┘ │
   │                                         ┌──────┐ │   terminal column (screwdriver access)
   │                                         │  TC  │ │
   └──[USB]──[press]──[scale]──[front]──[SSR]┴──────┴─┘
          low-voltage leads leave DOWN (open bottom)
```

## Files
- `holder.py` builds it (build123d): `~/opt/holder-venv/bin/python holder.py`. It writes `out/holder.stl` and
  `out/holder_bar.stl` (print both) plus `.step` files. Every opening in the front plate is computed from the
  board. The fit check sweeps every part and pin tail along its path into the holder and also checks:
  - the plugs, thermocouple wires, receptacles and the bar;
  - screwdriver access to the terminal;
  - which mains pads the front plate leaves uncovered (only the four spade tabs).
  It checks the model, not a print: holes in a real FDM print come out ~0.1–0.2 mm small, which the comb
  slots (0.6 mm a side over TE's maximum receptacle width) and grooves (0.25 mm a side) allow for.
- `assemble.py` writes `out/assembly.glb` and `out/exploded.glb`; open `out/view.html?f=assembly&v=iso`.

## Print
- **Material: ASA (or ABS).** Not PETG: its heat-deflection temperature (~70 °C) is right at the 60–70 °C
  inside the case, and the pump triac's tab runs near 100 °C 5.5 mm under the front plate. Not PLA.
- **Stand it on its floor** (the bottom end) on the bed: the walls, both plates and the grooves then print
  as vertical walls with no supports (the front plate reaches the bed, and its strip under the board ties
  it to the right wall). The LED window and the bar holes are small enough to bridge.
- 0.2 mm layers, 4 walls, 30 % infill. About 51 cm³ (~65 g). The bar prints flat.

## Magnets
- **Six 10 × 3 mm neodymium discs**, the commonest size there is. Press them into the pockets on the back face
  with a drop of CA glue, all with the same pole facing out.
- **Heat:** plain N-grade magnets (N35–N52; N52 is the least heat-tolerant) lose strength for good above
  ~80 °C, and the case runs 60–70 °C. Buy an **H or SH grade** (N35H, N42SH: rated 120–150 °C).
- **Test the panel first:** hold a magnet against the inside of your side panel. If it doesn't stick hard
  (some panels are stainless), use high-temperature double-sided tape instead.

## Fit it
Fit the four mains receptacles with the board **out of the holder**, a finger behind each tab: pulling one off
takes up to ~76 N (TE 108-2043), which would bend the board with nothing behind it.
1. Stick the holder to the inside of the side panel, open end up.
2. With the board in your hand: push the four mains receptacles onto the spades, plug the low-voltage leads
   into the bottom edge, and clamp the thermocouple's two wires in its screw terminal (TC+ chromel, the
   non-magnetic wire, in pin 1).
3. Slide the board down the grooves: the receptacles and their wires go down the comb slots, the low-voltage
   leads (and the antenna pigtail) out of the open bottom, until the board stands on the floor.
4. Push the retaining bar through both side walls above the board's top edge.

To take the board out: pull the bar, slide the board up with everything still plugged in, then unplug it in
your hand. USB-C (first flash, recovery) is only reachable with the board out.

## Water
The front plate keeps things from landing on the board, but its top is open. **Don't mount it under the
water-tank fill, the OPV return or the solenoid valve's exhaust.** Water that does get in runs down and out of
the open bottom; drain slots in the floor stop it pooling behind the board. A spray of conformal coating on
the assembled board (masking the plugs, the terminal and the spade tabs) is cheap insurance.

## Space it needs
The holder is 86.5 × 85 mm on the panel and 17 mm deep. The IRM supply stands 31 mm off the panel, and the spade
receptacles reach ~40 mm (TE's 23.7 mm receptacle on the 14 mm tab), ~45–50 mm with their wires bent upward.
**Check there's 50 mm of free depth at the spot you choose**, clear of the boiler, the pump and anything that
moves.
