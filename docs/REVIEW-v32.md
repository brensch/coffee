# Independent review of v32 (mains on board), and what v33 changed

An independent reviewer (a fresh agent given only the repo, 2026-11-06) checked v32's mains section from
the sources and the routed board, re-deriving the numbers instead of trusting the existing checks. It
modified nothing. Its findings and what happened to each:

## Must-fix

**M1. The pump triac could never re-fire after its first stroke. Fixed in v33.**
- **The mechanism.** The Ulka pump has a diode in series with its coil. While the triac is off, the 22 nF
  snubber cap (C15) charges through the pump on the forward half-wave to the line peak. The diode then
  blocks the only discharge path, so ~170 V DC sits across the triac. The MOC3083 is a zero-cross driver:
  onsemi's VINH is "MT1−MT2 voltage above which device will not trigger, typ 12, max 20 V". So it never
  fires again.
- **Reproduced.** In SPICE, after one stroke the triac voltage stays at 167–171 V at every following zero
  crossing. Leakage can't save it: the triac's µA-level IDRM would take ~0.2 s to drain 22 nF, against a
  16.7 ms window.
- **Why the old check missed it.** sim/run.py only modelled turn-off. AN-3004's circuit assumes a load that
  conducts on both half-waves.
- **The valve isn't affected.** It has no diode, so its triac voltage follows the line. The reviewer's
  zero-cross simulation shows it re-firing every half-cycle.
- **Fix: R54**, 51 kΩ across the pump triac (AC_L_F to PUMP_SW), τ 1.1 ms.
  - The part is a Vishay CRCW251251K0JNEG: 500 V working (187 V peak, 37 %), 1 W (171 mW while the pump is off).
  - **The value came from a second review round.** My first value, 100k, drained the cap after an
    isolated stroke (1.2 V). The reviewer then checked the pump running every cycle, the tight case: the
    triac drops out ~243° into the cycle with 137 V on the cap, and only ~5 ms are left before the next
    positive zero crossing. 100k still left 13–14 V there, over the MOC3083's 12 V typical inhibit (and
    the datasheet gives no minimum), so a typical part would skip strokes.
  - **51k gives 1.8 V running every cycle,** and 5 V even with 2.5× the assumed coil inductance.
    (47k, the reviewer's suggestion, has no 500 V 2512 in JLC stock.)
  - **sim/run.py now has re-arm checks:** both after one stroke and running every cycle, at 120 and 132 V,
    with v32's circuit and the rejected 100k as references. 79/79 pass.
- **Alternatives rejected:**
  - **A random-phase driver (MOC3052)** would need no new part. But it fires at any phase, so strokes
    split at window edges, while the engine counts exactly one stroke per enabled mains cycle. It also
    needs ~20 mA of LED drive.
  - **2 × 47k 200 V parts in series:** their mid node made the pump column unroutable at the 2 mm mains spacing.

## Should-fix

- **S1. Bottom-layer copper under the triacs.** Declined.
  - The T435's junction is 87 °C at 70 °C ambient, and 97 °C at a 1.4 A peak allowance (limit 115 °C).
  - That rests on an assumed 60 °C/W on the shared ~1.8 cm² top AC_L_F pour.
  - A bottom pour needs mains thermal vias, a new layout feature, for margin the design already has.
  - INSTALL §6 measures the tabs on the first install (< 100 °C). If that fails, the fix is a stick-on
    heatsink on the tab, not a new board.
- **S2. Stale descriptions.** Fixed:
  - the `controller.py` docstring (it said "no mains on this board");
  - the firmware pin-table comments and docs/ARCHITECTURE.md (rewritten with the thermocouple firmware);
  - CIRCUIT, HARDWARE, INSTALL, README and SWAP-SAFETY were rewritten for v32/v33.

## Nits

- **The opto slots don't extend past the pad rows**, so surface creepage goes round their ends. Kept:
  the 8.03 mm copper gap between the rows already exceeds the 6 mm requirement on its own. The slots
  only lengthen the direct path under the body. CIRCUIT.md no longer claims more than that.
- **Mains-to-mains spacing is 2.0 mm minimum** (2.91 mm L–N). That's enough for functional insulation at
  pollution degree 2. A breakdown there is a functional failure: a load runs, or the fuse opens. It
  isn't a shock path, because the mains-to-SELV isolation is 7.83 mm. Conformal coating stays optional.
- **The fuse's 50 A breaking capacity** is normal for an internal supplementary fuse behind the house
  breaker. Accepted.
- **N and L receptacles swapped:** nothing on the board works (the triacs see neutral), and the IRM has
  its own internal protection. The silkscreen labels each tab. Accepted.
- **USB while mains is live:** a new INSTALL §6 rule: never with the case open and the machine plugged in.

## Checked and found fine

The reviewer checked the following and found them fine:
- **Mains-to-SELV:** 7.83 mm (needs ~3.2 mm at PD2, ~5 mm at PD3). No SELV copper on B.Cu in the
  mains half.
- **The fuse:** F1 protects everything after L.
- **Earth:** no PE needed on the board (the SELV is double-insulated, the board sits in a plastic tray).
- **Opto LED current:** 9.5–11.1 mA.
- **The opto gate path:** matches onsemi Fig. 12, including the 330 Ω for an inductive load.
- **Pinouts:** the MOC3083 and DPAK pinouts.
- **Triac ratings:** both triacs, (dI/dt)c and the T410's turn-off kick.
- **Snubber energy:** 0.74 mJ per stroke.
- **The IRM:** derating and inrush.
- **Footprints:** the FASTON (TE C-63824 holes 1.40 mm, 5.08 mm apart) and the IRM (Mean Well drawing).
- **DRC and JLC limits.**

Not verified: the Nano2 443 land pattern (Littelfuse's site refused the fetch; the footprint comes from
the KiCad library's own drawing), and the DPAK/SMDIP rotations in JLC's preview. `jlc_cpl.py` solved
them against JLC's own footprints, but look at the preview when ordering.

## Re-review of the v33 fix

The same reviewer checked R54 on the re-routed board:
- **The value:** 100k was marginal with the pump running every cycle. Fixed (51k, above).
- **Placement:** R54 is vertical at (8.5, 14.5): 4.6 mm from H3, 6.8 mm from the board edge, 5.0 mm from
  the N tab. Isolation is still 7.83 mm.
- **R54's own failure modes:**
  - **Open:** back to M1. The pump doesn't run, which is obvious and safe.
  - **Short:** the pump runs uncommanded, the same as a shorted triac. The firmware's uncommanded-stroke
    check catches that. Thick film usually fails open, and R54 runs at 17 % of its power rating.
- **Regressions:** none. The full re-route moved only R54 and kept 0 unconnected and a clean DRC. The
  closest L–N approach on F.Cu is now 2.04 mm (v32: 3.42 mm), still over the 2.0 mm mains rule and fine
  for functional insulation at 120 V. The new PUMP_SW track on B.Cu passes the N and L tab pads at
  2.0 mm.
