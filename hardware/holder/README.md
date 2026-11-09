# Board holder (v42, for the v39 board)

Three printed parts. The **frame** stays on the inside of the machine's steel side panel (six magnets); the board
slides up into it. The **cover** goes on over the board from the front. Two **bars** slide across the full width
through ears on the frame and lock the cover; the cover's two fingers hold the board up.

- **Frame:** back plate, side walls with the board's grooves, a channel that grips the board's top edge, a solid roof
  just above the board, and an ear with a cross hole on each side at the top and the bottom.
- **Receptacles:** ordinary straight, fully insulated 6.3 mm receptacles push straight onto the board's four spades.
  They stick out under the roof, above the cover's top edge, and the wires leave straight out.
- **Cover:** shaped to the board. 17 mm off the panel over the low parts, 26 mm over the thermocouple terminal, 34 mm
  over the supply and along its top band; 45-degree steps, so it prints without supports.
- **Bars:** one across the cover's top band, one across its bottom edge. A head at one end, a small snap bump at the other.

## Files
- `holder.py` builds all three (build123d): `~/opt/holder-venv/bin/python holder.py`. It writes
  `out/holder_frame.stl`, `out/holder_cover.stl`, `out/holder_bars.stl` (+ `.step`) and checks, by solid geometry:
  - the board, every pin tail on its back and the plugged-in low-voltage plugs, leads and thermocouple wires, sliding
    up into the frame and seated;
  - the receptacles (TE's maximum size) and wires against the frame and the cover;
  - the cover seated and coming straight on; the bars against everything and bearing on the cover;
  - the fingers reaching under the board, the top stop and lip;
  - **coverage:** sight-lines onto the board from the front, each of which must hit the cover;
  - **overhangs** in each part's print orientation.
- `assemble.py` writes `out/assembly.glb` and `out/exploded.glb`; open `out/view.html?f=assembly&v=iso`.

## Print
- **ASA (or ABS).** Not PETG (heat-deflection ~70 °C, the case temperature) and not PLA.
- **Frame:** on its back plate. **Cover:** standing on its bottom edge. **Bars:** flat. No supports needed (checked).
- 0.2 mm layers, 4 walls, 25 % infill.

## Magnets
- **Six 10 × 3 mm neodymium discs, H or SH grade** (N35H, N42SH). Press them into the pockets on the frame's back with
  a drop of CA glue, same pole out. Plain N52 loses strength for good above ~80 °C.
- They don't affect the board. **Test the panel is magnetic first** (some are stainless); otherwise use
  high-temperature double-sided tape.

## Fit it
1. Stick the frame to the inside of the side panel, roof up.
2. With the board in your hand: plug the low-voltage leads into its bottom edge and clamp the thermocouple's two wires
   in its screw terminal (TC+ chromel, the non-magnetic wire, in pin 1).
3. Slide the board up into the frame from below until its top edge goes into the channel under the roof, and hold it
   there (nothing holds it up until the cover is on).
4. Push the four receptacles straight onto the spades. The channel over the tabs takes the push.
5. Put the cover on from the front: its fingers tuck under the board's bottom edge.
6. Slide the two bars across through the ears until they click. Wires leave straight out under the roof, low-voltage
   leads and the antenna pigtail out of the bottom.

To take it apart: slide the bars out, take the cover off (hold the board), pull the receptacles, slide the board down.
USB-C (first flash, recovery) needs the board out.

## Space it needs
93.5 mm wide across the ears, 88 mm tall. Off the panel: 17 mm over most of the front, 34 mm over the supply and the
top band, 39.6 mm at the receptacles' ends, 44.6 mm at the roof's edge. Leave room in front for the mains wires' bend.
