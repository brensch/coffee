# Board holder (v38)

A flat printed cradle that sticks to the inside of the machine's steel side panel with six magnets. The
controller board slides down into it, plugs up, and stays put by gravity. Nothing to screw, nothing to clip.

```
        low-voltage leads leave UP (thermocouple, SSR, front panel, scale, pressure)
   ┌───┬──────────────────────────────────────────────┬───┐
   │ ║ │  [TC] [SSR] [front] [scale] [press] [USB]     │ ║ │   ║ = grooves: the board's left and right
   │ ║ │                                               │ ║ │       edges ride in them (2.5 mm strips kept
   │ ║ │  ┌──────┐    ESP32, LEDs, low-voltage parts   │ ║ │       clear of parts on the board)
   │ ║ │  │ IRM  │ ═════════ isolation band ═══════════│ ║ │
   │ ║ │  │ PSU  │    fuse, triacs, snubbers (mains)   │ ║ │
   │ ║ │  └──────┘                                     │ ║ │
   │ ║ │   [N]       [L]   [P]          [3]            │ ║ │
   └───┴──────────────────────────────────────────────┴───┘
        mains wires leave DOWN (cable-tie slots in the floor)
```

## Files
- `holder.py` builds it (build123d) and checks the fit: every part and every through-hole pin tail on the
  board is swept along its path into the holder, and anything that would touch is reported. Run it with
  `~/opt/holder-venv/bin/python holder.py`. It writes `out/holder.stl` (print this) and `out/holder.step`.
- `assemble.py` writes `out/assembly.glb` and `out/exploded.glb`; `out/view.html?f=assembly&v=iso` shows them.

## Print
- **Material: PETG or ASA.** Not PLA: the case runs 60–70 °C.
- Back face down on the bed. No supports: the groove lips are 1.6 mm overhangs.
- 0.2 mm layers, 4 walls, 30 % infill. About 34 cm³ (~45 g).

## Magnets
- **Six 10 × 3 mm neodymium discs**, the commonest size there is, sold in packs everywhere.
- Press them into the pockets on the back face, with a drop of CA glue, all with the same pole facing out.
- **Heat:** ordinary N-grade magnets (N35–N52) start to lose strength for good above ~80 °C, and the
  inside of the case runs 60–70 °C. If you can find 10 × 3 mm in an **SH grade** (rated 150 °C), buy that.
  Otherwise use N52, and mount the holder away from the boiler.
- They hold the holder on the panel well. To pull a spade receptacle off, hold the holder with your other
  hand rather than relying on the magnets.

## Fit it
1. Push the four spade receptacles and the GH plugs onto the board on the bench first, or later in the
   machine. Either way, push spades on with the board seated: the ledge behind the spade row takes the push.
2. Slide the board down into the grooves until its bottom edge stands on the floor.
3. Put the holder on the inside of the side panel, plugs up. Dress the mains wires downward through the
   cable-tie slots and the low-voltage leads upward. The two bundles never cross.

## Space it needs (measure your machine)
The holder is 86.5 × 74 mm on the panel. From the panel outward, the stack is:

| Layer | mm from the panel |
|---|---|
| Back plate | 4 |
| Air for the pin tails | 4 |
| The board | 1.6 |
| IRM power supply (tallest part) | 31 |
| Spade receptacles and the wire bend | ~35–40 |

**Before printing, check there's 40 mm of free depth at the spot you choose,** clear of the boiler, the
pump and anything that moves. Gaggiuino and GaggiMate builds use the back corner behind the water funnel.
