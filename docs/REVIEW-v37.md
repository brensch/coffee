# Independent review of v37 (pre-order)

An independent reviewer (a fresh agent given only the repo, 2026-11-07) checked the v37 board from the
sources, the routed board and the order files. It re-derived the numbers rather than trusting the existing
checks, and it modified nothing. Nobody had reviewed v35–v37 before this: the board was laid out from
scratch in v35, and v36/v37 moved the mains input, the fuse and the bleeder.

**Verdict: don't order v37 as it stands.** One must-fix is in the full-assembly order files (M1), and one
layout clearance is below the board's own rule (S1). Both are confirmed independently of the reviewer: the
CPL rows and the R54–Q5 gap were re-measured on the board.

## Must-fix

**M1. The CPL places three of the four spade tabs wrong (full-assembly order only).**
- Placing JLC's own footprint for C575074 (two pads, numbered 1 and 2, at ±2.54 mm) at each CPL row:
  - J14 is right.
  - J15 is one pitch off: (49.28, 57.50) should be (44.20, 57.50).
  - J12 and J13 are at rot 0, which lays the tab across their vertical hole pairs. They should be rot 90
    at (4.00, 54.50) and (4.00, 43.50).
- **Cause:** the board footprint numbers both tab pads "1". `jlc_cpl.py` matches pads by number, so it
  fitted only one pad and reported a 0.00 mm offset. **Fix:** `PAD_MAP["C575074"] = {"2": "1"}`.
- v36's CPL has the same rows. The SMD-only order leaves the tabs out and isn't affected.

**M2. WIRING.md step 8 is unsafe to follow (fix before install, not before ordering).**
- It still has you take the mains off the power rocker and wire the rocker as the J7.1 standby input.
  Since v35 the firmware has no power input and no standby, so the heater would stay under firmware
  control whenever the machine is plugged in.
- The J7 and J10 tables also give the old 7-pin pinouts. A scale lead built to the old J10 table puts
  3.3 V on the HX711 board's GND.
- **v37 pinouts:** J7: 1 BREW, 2 STEAM, 3 GND, 4–6 power/coffee/steam LEDs. J10: 1 DOUT1, 2 SCK, 3 GND,
  4 DOUT2, 5 +3V3.

## Should-fix

**S1. R54 sits 0.92 mm from Q5's tab, below the board's 2.0 mm rule between mains potentials.**
- R54 pad 2 (PUMP_SW) has its right edge at x 23.18; Q5's tab (AC_L_F) starts at x 24.10. That gap
  carries the full off-state voltage.
- **Why DRC passed:** the "pins of one part" 0.5 mm exemption includes PS1. Since v35, PS1's underside
  courtyard (x 5.8–31.7, y 9.65–55.85) covers most of the mains half, so the 2.0 mm rule hasn't applied
  there since v35.
- If it breaks down, the pump runs uncommanded (the same as a shorted triac), which the firmware
  detects. Still, it's below the board's own rule, in a steamy case.
- **Fix:** move R54 about 1.2 mm left, and drop PS1 from the exemption (its own mains pins are 10.75 mm
  apart).
- Every other mains pair the reviewer measured is at least 2.0 mm, apart from the deliberate
  same-potential pairs:

| Pair | Gap |
|---|---|
| L–N | 2.39 mm |
| AC_L_F–N | 2.21 mm |
| N–PUMP_SW (B.Cu) | 2.03 mm |
| L_F–P, L_F–3 (triac pins) | 2.00 mm |

**S2. Mechanical retention isn't designed, and adding a hole is a PCB change.**
- There are no mounting holes since v35, and there's no housing design in the repo. BUY, HARDWARE,
  INSTALL and WIRING still call for M3 standoffs at H1–H3, which no longer exist.
- Pulling a receptacle off takes roughly 20–60 N, which loads the IRM's four 1 mm pins. J15 and C16
  overhang the IRM by 13–20 mm, so pushing a receptacle onto J15 flexes the board at the C15/C16 2220 X2
  caps, which crack easily under flex.
- No holes is acceptable only if the housing pockets the IRM, supports the underside under all four
  tabs and clamps the board down.
- **Cheap insurance:** an NPTH at about (40.8, 34.4) in the isolation band, between the optos, has
  4.6 mm to any courtyard or track. It takes a nylon screw or peg only; a metal screw there would bridge
  mains to SELV at about 2.2 mm. Mark it "NYLON" on the silkscreen.

**S3. Stale purchasing and build docs.**
- BUY #10 lists GHR-07V-S; v37 needs GHR-06V-S (J7) and GHR-05V-S (J10).
- PARTS lists J10 as SM07B.
- INSTALL §3 tests the inputs by shorting J7.1–J7.4, but J7.4 is now an LED drive.
- The OTA gate is described as "power rocker off, steam on". It is now steam on, then brew held 3 s.

## Notes

- **Triac heat:** the AC_L_F pour that CIRCUIT.md and the v32 S1 decision rely on is gone. Each DPAK now
  has only its pad, about 0.4–0.5 cm², which ST rates at 70 °C/W. The pump triac at 0.26–0.46 W reaches
  88–102 °C at 70 °C ambient, against Tj max 125 °C: OK, with less margin than v32 assumed. Q5 sits over
  the IRM, so INSTALL §6's tab-temperature check matters more.
- **Isolation minimum** is 7.70 mm (F1 pad 2 to the GND pour), not the 7.83 mm the docs quote. The limit
  is 6.0 mm.
- **AC_L to AC_L_F** is 0.51 mm across the fuse. It only sees voltage after the fuse has blown.
- **Opto slots** are 1.0 mm wide, exactly JLC's minimum non-plated slot.
- **The IRM's GND pin** is fully flooded on both layers, so hand-soldering it needs a hot iron.
- **Stock:** R54 (C2078757) has 880, Q5 1,678, Q6 1,320, F1 1,367.
- **`build/schematic-120v.svg`** still shows SW_POWER and IO14 in use.

## Checked and found fine

- **Netlist vs board:** every pad matches `parts-120v.json`, with 0 unrouted. KiCad DRC is clean apart
  from library-table notices.
- **Gerbers:** byte-identical to the fab output, and the drill counts match.
- **Fuse:** F1 protects everything after L, and no mains load current goes through a via.
- **Mains tracks:** 0.8 mm (0.6 mm on a few valve-feed segments) carries about 2 A at a 10 °C rise,
  against 0.67 A of load. The tracks' fusing I²t is above the fuse's.
- **Mains to SELV:** 7.70 mm on both layers. The band is copper-free, and there's no SELV copper on B.Cu
  in the mains half.
- **IRM-10-5 footprint:** matches Mean Well's drawing. The flipped footprint puts AC/L on AC_L_F and −V
  on GND.
- **Mains circuit:** unchanged since v33 (R54 51k 500 V; snubbers 100 Ω 1 W with 22 nF X2; R53 1k; the
  opto gate path).
- **Pin map:** J7 (6-pin), J10 (5-pin) and IO14 not connected all match `firmware/esp32/src/main.rs`.
- **CPL, every other polarized part:** each lands on JLC's own pads. Worst offsets are 0.04–0.37 mm (the
  LEDs via the existing pad map, D2, the optos, the triacs, U4–U7, J4).
- **PS1** is correctly left out of the JLC BOM/CPL and listed for hand-soldering.

## Docs out of date with v37

CIRCUIT.md (still v33), WIRING.md (step 8, J7/J10, M3 tray), INSTALL.md (H1–H3, J7 test, OTA gate),
HARDWARE.md, BUY.md (#10, #19), PARTS.md, SWAP-SAFETY.md, README.md, the `controller.py` docstring, the
`firmware/esp32/src/main.rs` header comment and `build/schematic-120v.svg`.
