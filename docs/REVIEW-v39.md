# Independent review of v39 (board + holder), and what was done

An independent reviewer (a fresh agent given only the repo, 2026-11-07) checked the v39 board, its order files
and the printed holder. It re-derived the numbers from the routed board, JLC's own footprint data and the
build123d model, and modified nothing. The docs being rewritten at the same time (WIRING, INSTALL, BUY,
HARDWARE, CIRCUIT, PARTS, SWAP-SAFETY, README) were out of its scope; the holder's README was in.

**Verdict: no must-fix on the board.** Both v37 board/order-file findings are fixed, and every mains spacing
meets the board's 2.0 mm rule. The problems were in the holder and its instructions; all are fixed below.

## Should-fix (holder), all done

**1. The front plate couldn't be printed as stated. Fixed.**
- **Problem:** the terminal column (full height) and the supply channel cut the plate off from the right wall
  everywhere. The plate's lowest layer was 3 mm above the bed, so printed floor-down its edge hung in air.
- **Fix:** the plate now reaches the floor (Y −3). It starts on the bed, and its strip below the board's
  edge ties it to the right wall. The GH plugs and leads pass behind that strip (they reach ≤ 4.6 mm off
  the board; the plate is at 5.5).

**2. The comb slots were smaller than a real receptacle. Fixed.**
- **Problem:** TE drawing C-520140 gives a total width of 10.39 mm max and a length of 23.7 mm, against the
  modelled 9.5 × 5.0 mm (slots 10.5 wide), with prints running 0.1–0.2 mm small.
- **Fix:** slots are now 11.6 mm wide (J13/J14 merge). The fit check models TE's maximum receptacle,
  reaching 30 mm off the board, both seated and sliding in. The depth needed is now stated as ~50 mm.

**3. Water came in at the top and was trapped behind the board. Partly fixed.**
- **Fix:** drain slots through the floor behind the board, so nothing pools around the J4/J5 pin tails.
- **Not fixed:** the top stays open. A roof over the comb would sit in the receptacles' wire path.
- **Instead:** the README says where not to mount it (under the tank fill, the OPV return, the solenoid
  exhaust) and recommends conformal coating.

**4. Pulling receptacles off in place would bend the board. Fixed (procedure).**
- **Problem:** first withdrawal is up to 17 lbf (~76 N, TE 108-2043). Held only by the two grooves, the
  reviewer's plate model gives a 1.0–1.3 mm bow and ~1,000–1,200 µε at the C15/C16 2220 X2 MLCCs.
- **Fix:** receptacles now go on and come off with the board out of the holder, a finger behind each tab.
  The comb is open at the top, so the board slides in with them on (fit-checked). Changed in the holder
  README, WIRING.md and INSTALL.md.

**5. Material and magnets. Fixed.**
- ASA (or ABS), not PETG: PETG's heat-deflection temperature (~70 °C) is right at the case temperature, and
  Q5's tab runs ~100 °C 5.5 mm under the plate.
- Magnets: H or SH grade; N52 is the least heat-tolerant, so it's no longer the fallback.
- "Test the panel is magnetic first" (stainless 304 isn't), with tape as the fallback.

**6. Antenna pigtail and USB-C weren't designed for. Fixed (procedure).**
- The pigtail leaves through the open bottom with the low-voltage leads.
- USB-C is only reachable with the board out. A USB-C overmold would reach into the floor.

## Notes and what happened to each
- **Pump triac heat:**
  - Q5's tab copper is ~0.46 cm², which matches ST's 70 °C/W for 0.5 cm². `sim/run.py` assumes 60 °C/W.
  - At 70 °C ambient the reviewer gets Tj ≈ 90–102 °C, against 125 °C. That's OK, but 3–5 °C worse than
    the sim says.
  - Left as is. The sim's assumption should be corrected the next time it runs.
- **Thermocouple:**
  - U7 sits between the terminal and the IRM, 7.2 mm from the pin row, so it may read a few tenths of a °C
    warm. The single offset calibration covers it.
  - Every other heat source is ≥ 29 mm away or above the block.
  - The RC filter and the MCP3421's slow conversion handle the supply's switching noise. Accepted.
- **Magnet next to the supply's AC pin:** the top-right magnet sat at the PS1.1 (AC_N) pin's pocket edge.
  Moved to (72, 52). Free to do.
- **Retaining bar:** plain sliding fit. The holes are now a snug push fit (+0.15 mm), so pump vibration
  can't walk the bar out.
- **Edge rails:** J4, the GH headers and J5 sit flush with the bottom edge, so expect JLC to add edge rails
  when ordering. A small extra cost; accept it.
- **Silkscreen:** the pin labels are on the back silk, facing the panel once mounted. Accepted: they're read
  with the board in hand, which is how it's wired now.
- **"Covered" means not visible from the front.** The 5.5 mm gap above the board is open across its width,
  so a dropped screw could fall onto the triac pads. The README's "never touch it plugged in" stands.
- **Opto slots** add nothing to corner-pin creepage. The 8.03 mm pad-to-pad distance stands on its own.
  Accepted.

## Checked and found fine
- **REVIEW-v37 M1 (spade-tab CPL):** fixed. JLC's C575074 footprint lands both legs at 0.00 mm on all four tabs.
- **REVIEW-v37 S1 (R54 and the exemption hole):** fixed.
  - R54 to Q5's tab is now 2.20 mm, and PS1 is out of the exemption.
  - The remaining exemptions (U8, U9, Q5, Q6, F1) hide nothing: every different-potential mains pair is
    ≥ 2.00 mm.
  - The tightest are the triacs' own pins at 2.00 mm. Snubber to gate is 2.38 mm; J14 to J13 2.62 mm;
    L or L_F to N ≥ 4 mm.
- **Mains to SELV:** 7.83 mm. The 36–43 mm band is copper-free on both layers, and the 180° turn put nothing
  across it.
- **Fuse:** AC_L touches only J13 and F1.1, so F1 protects everything after L. N goes only to PS1.1.
- **Tracks and vias:** all mains tracks are 0.8 mm (≤ ~0.7 A load); the only mains vias are the two gate vias.
- **DRC, netlist, order files:** DRC is clean (0 violations besides library notices, 0 unconnected), and
  the netlist matches. The BOM and CPL designators match in both orders, and the SMD order leaves out
  exactly J5, J12–J15 and PS1.
- **CPL polarity:**
  - Worst offsets are 0.37 mm on the triacs; everything else is smaller.
  - J5: pin 1 is TC_P, and its entry faces the bottom edge in both footprints.
  - PS1: fits at 0.00 mm with the right handedness.
- **Gerbers and drill files:** consistent with the board.
- **Repo checks:** isolation, pin functions and swap safety all pass.
- **holder.py:** reproduces "fit: OK". Only the spade tabs are uncovered, the pin tails clear the back
  plate, and the height overrides are conservative.
