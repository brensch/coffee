# Independent review of the v42 holder, and what v44 did about it

An independent reviewer (a fresh agent, 2026-11-08) checked the v42/v43 holder geometry; the main points were verified
arithmetically. v44 redraws the holder, so each finding is closed by the new design or carried over below.

| # | Finding | v44 |
|---|---|---|
| M1 | Bars can't print flat (head proud both sides, exported in place, overhangs never run on them) | One-sided flush head; `holder_bars.stl` is the two bars lying flat on Z = 0; overhang check runs on them |
| M2 | 0.0 mm cover clearance between the ears | 0.3 mm each side |
| M3 | Board unsupported before the cover goes on | Recess bottom wall holds it up |
| S1 | Receptacle funnel is 8.26 mm across, modelled 5 | Modelled at 10.4 × 8.3; the cover's top edge sits 1 mm below it |
| S2 | J13/J14 are 0.11 mm apart at TE's maximum width | Open: the receptacles for the 18 AWG wire have to be checked against the part actually bought |
| S3 | Lips need 45° undersides | 45° underside with a 0.7 mm land; no shallow overhangs (checked) |
| S4 | CA instead of high-temperature epoxy; magnets may peel at 76 N | README: epoxy, brace by hand when pulling receptacles |
| S5 | Snap bump unproven | Cable tie through a groove in the bar end |
| S6 | Cover needs a brim; 0.35 mm sliver below the bar hole | README: brim. Now 1.35 mm below the hole |
| S7 | Cover top open | Left top edge slopes shut (45° inside); coverage check passes |
| S8 | Mains shanks and pads reachable under the roof | README: no mains with the side panel off |
| S9 | Wire bend room and bar-insertion clearance unmeasured | Open: measure on the machine (about 90 mm beside the holder) |

Trims taken up: roof and back plate cut to the frame width.

The reviewer's point about supports was right: my earlier "no supports" claim was wrong for the bars and the lips.
v44's overhang check covers every part in its print orientation.

New in v44: the board has to get in under the lip with its pin tails in pockets, so `holder.py` searches the board's
poses for a path. Without the slots on the top row of pins there is none.
