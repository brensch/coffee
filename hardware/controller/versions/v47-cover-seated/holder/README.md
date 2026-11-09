# Board holder (v45, for the 71.2 × 77 mm v45 board)

Three printed parts. The **frame** stays on the inside of the machine's steel side panel (seven magnets). The board lies
flat on its back plate. The **cover** goes on over the board from the front. Two **bars** slide across the full width
through the frame's struts and lock the cover.

- **Frame:** back plate with a close recess for the board, small pockets for the through-hole pin tails, a lip over the
  board's top edge, a solid roof just above the board, and struts straight out of the sides at the top and the bottom,
  each with a hole running across.
- **Board:** tilts in with its top edge hooked under the lip, then swings down into the recess. Its bottom edge rests on
  the recess wall. The four top pockets are short slots so the pins can move while it hooks in.
- **Receptacles:** ordinary straight, fully insulated 6.3 mm receptacles push straight onto the board's four spades.
  They stick out under the roof, above the cover's top edge, and the wires leave straight out.
- **Cover:** shaped to the board and closed across its whole top edge, over the supply too, so nothing falls in from
  above. 13.2 mm off the panel over the low parts, 22.2 mm over the thermocouple terminal, 29.9 mm over the supply;
  45-degree steps.
- **Spring finger:** a thin leaf printed on the cover's inside. Drawn free it reaches 0.8 mm past the board's face;
  seated, the board bends it back. That is what holds the board flat on the plate (about 1.2 N, a beam estimate, not a
  measurement).
- **Seating:** the cover's rim lands on the frame's recess walls (0.2 mm clear of the board itself), and the bars hold it
  there, each bar braced against the front face of its holes.
- **Bars:** one across the cover's top, one across its bottom edge. Their leading end is ramped: as a bar slides in it
  pushes the cover back against the finger. A head at one end, a groove for a small cable tie at the other, so they
  can't work out.

## Files
- `holder.py` builds all three (build123d): `~/opt/holder-venv/bin/python holder.py`. It writes
  `out/holder_frame.stl`, `out/holder_cover.stl`, `out/holder_bars.stl` (+ `.step`; the bars lie on their sides, ready to print;
  `holder_bars_assembled.stl` is the same two in place, for the viewer) and checks, by solid geometry:
  - the board, every pin tail, the plugged-in low-voltage plugs, leads and thermocouple wires against the frame;
  - **tilt-in:** a search through every pose of the board's section with its pin tails, for a connected path from free
    to seated under the lip;
  - the receptacles (TE's maximum size, including the funnel) and wires against the frame and the cover;
  - the cover seated and coming straight on; the bars against everything and bearing on the cover;
  - the finger reaching past the board, the ramp and the spring travel beating the bars' slack, the supply fitting under
    the closed top, the lip, the wall under the board, the magnets clear of the pin pockets;
  - **coverage:** sight-lines onto the board from the front, each of which must hit the cover;
  - **one solid body** per part (frame 1, cover 1, bars 2) and **overhangs** in each part's print orientation.
- `assemble.py` writes `out/assembly.glb` and `out/exploded.glb`; open `out/view.html?f=assembly&v=iso`.

## Print
- **ASA (or ABS).** Not PETG (heat-deflection ~70 °C, the case temperature) and not PLA.
- **Frame:** on its back plate. **Cover:** standing on its **top** edge (the closed top is the flat first layer).
  **Bars:** on their sides, as exported. No supports needed (checked).
- 0.2 mm layers, 4 walls, 25 % infill, **at least 6 solid bottom layers on the frame**. Behind the mains pins' pockets
  there is 1.2 mm of plastic to the steel panel, and it has to be solid. Look at the pockets after printing: no gaps.
- ASA shrinks about 0.5 %; if the first print is tight, scale the frame up 0.5 %.

## Magnets
- **Seven 10 × 3 mm neodymium discs, H or SH grade** (N35H, N42SH). Set them into the pockets on the frame's back with
  **high-temperature epoxy** (not CA), same pole out. Plain N52 loses strength for good above ~80 °C.
- They don't affect the board. **Test the panel is magnetic first** (some are stainless); otherwise use
  high-temperature double-sided tape. They sit on a 3 × 3 grid, less the two spots behind the supply. Brace the frame with a hand when pulling receptacles.

## Fit it
1. Stick the frame to the inside of the side panel, roof up.
2. With the board in your hand: plug the low-voltage leads into its bottom edge and clamp the thermocouple's two wires
   in its screw terminal (TC+ chromel, the non-magnetic wire, in pin 1).
3. Tilt the board with its bottom edge toward you, set its top pins into their slots, slide it up so the top edge goes
   under the lip, and swing the bottom down into the recess.
4. Push the four receptacles straight onto the spades.
5. Put the cover on from the front, between the struts. It sits about 0.8 mm proud of the frame until the bars push it
   down: the spring finger is touching the board.
6. Slide the two bars across through the struts, ramped end first. The ramp pushes the cover back as the bar goes
   in. Pull a small cable tie through each groove. Wires leave straight
   out under the roof, low-voltage leads and the antenna pigtail out of the bottom.

To take it apart: cut the ties, slide the bars out, take the cover off, pull the receptacles (hold the board down near
the top edge), lift the bottom of the board and slide it down out of the lip. USB-C (first flash, recovery) needs the
board out.

**Never run mains with the side panel off.** The spades' shanks and pads are under the roof, not sealed, and can be
reached from the front.

## Space it needs
78.4 mm wide across the struts, 87.6 mm tall. Off the panel: 13.2 mm over most of the
front, 29.9 mm over the supply, about 34.6 mm at the top bar, 36.1 mm at the receptacles' ends, 41.1 mm at the roof's
edge. Not yet measured on the machine: room in front for the mains wires' bend, and about 90 mm beside the holder to
slide the bars in.
