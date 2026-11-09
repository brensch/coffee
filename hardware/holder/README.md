# Board holder (v40 holder, for the v39 board)

One printed part that sticks to the inside of the machine's steel side panel with six magnets, plus two pegs.
The back plate rises above the board and turns into a roof that overhangs outward. Below it, a deep solid front
wall covers the whole board: no window, no comb, no channel. The four mains wires leave straight out of their
receptacles through a forward-facing slot between the front wall's top and the roof, so drips from above hit
the roof and run off outside the front wall. Everything else leaves through the open bottom.

```
 side section (panel on the left)            front
 ┌──────────────────── roof ─┐               ┌──────────── roof ──────────┐
 │   ┌─channel─┐             │               │  [3]   [P] [L]      [N]    │ <- wire slot (faces forward)
 │   │ ▐[tab]━━━━ receptacle ━━━━ wire out    ├────────────────────────────┤
 │   │ ▐       │    ┌──front wall            │                            │
 │   │ ▐ board │    │                        │   front wall: solid        │
 │   │ ▐       │    │                        │                            │
 │   │ ▐ supply│    │                        │                            │
 │   │ ▐       │    │                        └─[peg]──────────────[peg]───┘
 │ magnets     │    │                            low-voltage leads leave down
 └─peg═══════════════╪══                       
```

## Files
- `holder.py` builds it (build123d): `~/opt/holder-venv/bin/python holder.py`. It writes `out/holder.stl` and
  `out/holder_peg.stl` (both pegs; print the file once) plus `.step` files. The fit check, by solid intersection:
  - every part, every pin tail on the back, the plugged-in low-voltage plugs and leads, and the thermocouple
    wires, sliding up into the holder from below and seated;
  - each spade receptacle going on straight through the wire slot;
  - the pegs against everything;
  - a lip in front of the board's top edge (it takes the receptacles' pull-off) and a stop above it;
  - which mains pads are visible through the wire slot (only the four spade tabs' own pads).
- `assemble.py` writes `out/assembly.glb` and `out/exploded.glb`; open `out/view.html?f=assembly&v=iso`.

## Print
- **Material: ASA (or ABS).** Not PETG (its heat-deflection temperature, ~70 °C, is the case temperature) and not PLA.
- **Print it upside down, standing on its roof**: the roof is the first layer, and the back plate, side walls,
  front wall, grooves and ribs all rise straight up from it (the open bottom ends up on top). The one overhang
  is the front wall's top edge, which starts mid-air above the wire slot: an 80 mm bridge between the side
  walls. Most printers bridge that fine; otherwise let the slicer put support in the wire slot only.
- 0.2 mm layers, 4 walls, 25 % infill. About 93 cm³ (~115 g). Print the pegs flat.

## Magnets
- **Six 10 × 3 mm neodymium discs, H or SH grade** (N35H, N42SH: 120–150 °C). Plain N52 is the least
  heat-tolerant and the case runs 60–70 °C. Press them into the pockets on the back with a drop of CA glue,
  same pole out.
- They don't affect the board: no part on it cares about a static field, and the supply's transformer is
  20–30 mm away through the board.
- **Test the panel first:** if a magnet doesn't stick hard to the inside of your side panel (some are
  stainless), use high-temperature double-sided tape instead.

## Fit it
1. Stick the holder to the inside of the side panel, roof up.
2. With the board in your hand: plug the low-voltage leads into its bottom edge, and clamp the thermocouple's
   two wires in its screw terminal (TC+ chromel, the non-magnetic wire, in pin 1).
3. Slide the board up into the holder from below, along the side grooves, until its top edge goes into the
   channel under the roof. The low-voltage leads (and the antenna pigtail) hang out of the open bottom.
4. Push the two pegs through the slots in the front wall, under the board's bottom edge, into the back plate.
   The board now stands on them.
5. Push the four mains receptacles onto the spades through the wire slot, straight in. The channel above the
   tabs and the ribs behind the board take the push. Dress the wires out of the slot.

To take it out: pull the four receptacles off through the slot (the channel holds the board), pull the pegs,
slide the board down. USB-C (first flash, recovery) is only reachable with the board out.

## Water
The roof keeps drips off; the only opening at the top end faces forward under it. Water that does get in runs
down and out of the open bottom. Still, **don't mount it under the water-tank fill, the OPV return or the
solenoid valve's exhaust.** Conformal coating on the board is optional extra insurance.

## Space it needs
86.5 mm wide, 96.5 mm tall and 52 mm deep off the panel (44.5 mm at the front wall, 52 mm at the roof's edge).
The receptacles are inside; the mains wires leave forward from the slot, so leave room in front of it for their
bend. **Check there's ~60 mm of free depth** where you mount it, clear of the boiler, the pump and anything
that moves.
