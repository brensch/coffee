# Profiles on a Gaggia Classic Pro

Every built-in and imported community profile, brewed once on the emulated E24 by
`tools/profile_check.py`: warm machine, **each profile's own grind** (table at the bottom), starting
at the profile's own temperature. Re-run it after calibrating the model to your machine. The
numbers below come from the uncalibrated US 120 V model (9 bar OPV).

**Columns**
- Time and yield of the shot, and peak pressure.
- Mean pressure and flow tracking error. These ignore the moments the controller can't do
  anything about:
  - the first 1.5 s of each phase;
  - the profile's own flow or pressure cap holding the shot back;
  - the pump already at full stroke;
  - pressure above target with the pump off (it can only bleed away through the puck).
- How many phases ran. A weight stop can end a shot before the last phase.

## What the flags mean

- **Adaptive v2 (GaggiMate)** is the only flagged profile. It fills at full pump for 5 s, which
  leaves the puck at 9 bar. Then it asks for 3 bar and then 0.1 bar, which only the slow bleed
  through the puck can deliver. Its "maintain current flow" phase is also approximated by the
  importer. It still brews a normal 40 g shot in 31 s.
- **Decent limiters are soft.** A Decent step's `max_flow_or_pressure` acts across a range above
  the limit, so the importer uses limit + range/2. Extractamundo Dos!'s 1.0 ml/s limit with a
  3.0 range is documented to end at 2.5–4 ml/s, which a hard cap would make impossible.
- **Allongés (Blooming, Rao)** ask for 4.5+ ml/s at 8–9 bar. A Classic's vibratory pump tops out
  around 4 ml/s at 9 bar (Ulka E5 curve), and the US 9 bar OPV bleeds about 1 ml/s of that back to
  the tank, so these run slower than on Decent's machine. Their large
  yields (≈130 g) are intended.
- **A-Flow on a US machine** wants about 4 ml/s *and* a 10 bar peak. The US OPV caps a Classic near
  9 bar, so at their old fine grinds the flow fell about 1 ml/s short. Their grinds are now coarser
  (×0.7–1.0) so the flow steps track; the cost is a lower peak (about 5–7.5 bar) and shorter shots.
- **Low-temperature profiles (82–86 °C)**, such as 80's Espresso, Extractamundo, Turbo, Damian's Q
  and Forge Dark, brew fine. But on a single boiler, getting *down* to that temperature takes
  minutes, or a cooling flush.
- **Per-step temperatures** in Decent profiles (Extractamundo programs a drop) aren't
  representable. The first step's temperature is used for the whole shot.

## Results

**Profile check: 55 profiles (54 OK, 1 CHECK)** (US 120 V model)

|---|---|---|---|---|---|---|---|---|---|
| 7g basket | Decent | OK | 23.8 | 19 | 8.0 | 0.06 | 0.17 | 3/3 |  |
| 80's Espresso | Decent | OK | 25.3 | 36 | 8.6 | 0.07 | 0.21 | 4/4 |  |
| 9 Bar Espresso | GaggiMate | OK | 25.4 | 38 | 8.7 | 0.35 | – | 1/1 |  |
| A-Flow / default-dark | Decent | OK | 34.6 | 36 | 7.3 | – | 0.12 | 6/6 |  |
| A-Flow / default-like-dflow | Decent | OK | 31.6 | 36 | 7.5 | – | 0.11 | 6/6 |  |
| A-Flow / default-medium | Decent | OK | 34.9 | 42 | 7.6 | – | 0.12 | 6/6 |  |
| A-Flow / default-very-dark | Decent | OK | 25.8 | 32 | 5.2 | – | 0.12 | 6/6 |  |
| Adaptive v3 | Decent | OK | 30.7 | 34 | 8.3 | 0.27 | 0.50 | 7/7 |  |
| Advanced spring lever | Decent | OK | 42.5 | 32 | 8.9 | 0.03 | 0.11 | 5/5 |  |
| Best overall pressure profile | Decent | OK | 25.5 | 36 | 8.6 | 0.02 | 0.58 | 3/3 |  |
| Best practice (light roast) | Decent | OK | 30.4 | 33 | 8.4 | 0.06 | 0.45 | 5/5 |  |
| Blooming |  | OK | 46.4 | 40 | 8.7 | 0.05 | 0.18 | 4/4 |  |
| Blooming Allongé | Decent | OK | 79.5 | 135 | 8.0 | – | 0.13 | 6/6 |  |
| Blooming Espresso | Decent | OK | 62.5 | 43 | 8.5 | – | 0.12 | 6/6 |  |
| Classic 9 bar |  | OK | 29.5 | 36 | 8.7 | 0.34 | 0.23 | 4/4 |  |
| Classic Italian espresso | Decent | OK | 25.3 | 36 | 8.7 | 0.35 | 0.57 | 2/2 |  |
| Cremina lever machine | Decent | OK | 74.9 | 110 | 8.7 | 0.04 | – | 5/5 |  |
| Cremina lever machine | GaggiMate | OK | 39.0 | 38 | 8.7 | 0.06 | – | 5/5 |  |
| Damian's LM Leva | Decent | OK | 33.9 | 42 | 8.7 | 0.03 | – | 6/6 |  |
| Damian's LM Leva | GaggiMate | OK | 31.1 | 38 | 8.7 | 0.06 | – | 6/6 |  |
| Damian's LRv2 | Decent | OK | 25.4 | 36 | 8.5 | 0.06 | – | 5/7 |  |
| Damian's LRv3 | Decent | OK | 27.9 | 36 | 8.6 | 0.11 | – | 6/8 |  |
| Damian's Q | Decent | OK | 29.7 | 36 | 8.8 | – | 0.13 | 3/3 |  |
| Default | Decent | OK | 25.3 | 36 | 8.7 | 0.11 | 0.57 | 3/3 |  |
| E61 classic gently up to 10 bar | Decent | OK | 26.0 | 36 | 8.7 | 0.41 | 0.55 | 3/3 |  |
| E61 espresso machine | Decent | OK | 24.6 | 36 | 8.7 | 0.35 | – | 1/1 |  |
| E61 rocketing up to 10 bar | Decent | OK | 29.6 | 36 | 8.8 | – | 0.57 | 2/2 |  |
| E61 with fast preinfusion to 9 bar | Decent | OK | 25.2 | 36 | 8.7 | 0.35 | 0.58 | 2/2 |  |
| Easy blooming - active pressure decline | Decent | OK | 30.2 | 45 | 8.3 | 0.09 | 0.30 | 12/12 |  |
| Espresso Forge Dark | Decent | OK | 29.9 | 42 | 8.5 | 0.09 | 0.26 | 3/4 |  |
| Espresso Forge Light | Decent | OK | 54.6 | 42 | 8.6 | 0.08 | 0.16 | 4/5 |  |
| Extractamundo Dos! | Decent | OK | 24.0 | 40 | 6.1 | – | – | 4/4 |  |
| Flow profile for milky drinks | Decent | OK | 32.8 | 21 | 8.4 | – | 0.15 | 2/2 |  |
| Flow profile for straight espresso | Decent | OK | 26.8 | 36 | 8.6 | – | 0.19 | 2/2 |  |
| Gentle and sweet | Decent | OK | 24.6 | 36 | 6.2 | 0.02 | 0.50 | 3/3 |  |
| Gentle flat 2.5 ml per second | Decent | OK | 28.9 | 36 | 8.4 | – | 0.12 | 2/2 |  |
| Gentle preinfusion flow profile | Decent | OK | 28.3 | 36 | 8.6 | – | 0.11 | 2/2 |  |
| Gentler but still traditional 8.4 bar | Decent | OK | 27.6 | 36 | 8.7 | 0.09 | 0.15 | 2/3 |  |
| Hybrid pour over espresso | Decent | OK | 29.6 | 19 | 8.8 | – | 0.36 | 2/2 |  |
| I got your back | Decent | OK | 29.5 | 36 | 8.7 | – | 0.20 | 3/6 |  |
| Innovative long preinfusion | Decent | OK | 40.7 | 35 | 8.7 | – | 0.45 | 2/2 |  |
| Italian Australian espresso | Decent | OK | 27.2 | 36 | 8.7 | 0.07 | 0.22 | 2/2 |  |
| Londonium | Decent | OK | 27.6 | 42 | 8.5 | 0.09 | – | 6/7 |  |
| Low pressure lever machine at 6 bar | Decent | OK | 29.9 | 36 | 7.1 | 0.04 | 0.23 | 3/3 |  |
| Magic (double ristretto) |  | OK | 30.7 | 28 | 8.8 | 0.20 | 0.22 | 4/4 |  |
| Preinfuse then 45ml of water | Decent | OK | 22.8 | 26 | 8.6 | – | 0.14 | 2/2 |  |
| Rao Allongé | Decent | OK | 37.4 | 135 | 7.6 | – | 0.14 | 2/2 |  |
| Traditional lever machine | Decent | OK | 28.0 | 36 | 8.7 | 0.20 | 0.58 | 3/3 |  |
| Trendy 6 bar low pressure shot | Decent | OK | 24.2 | 36 | 7.5 | 0.06 | 0.62 | 2/2 |  |
| Turbo flow |  | OK | 16.3 | 45 | 5.7 | – | 0.14 | 2/2 |  |
| TurboBloom | Decent | OK | 15.3 | 39 | 7.3 | 0.22 | 0.21 | 6/6 |  |
| TurboTurbo | Decent | OK | 13.9 | 35 | 6.3 | 0.02 | – | 5/6 |  |
| Two spring lever machine to 9 bar | Decent | OK | 28.2 | 36 | 8.7 | 0.20 | 0.57 | 3/3 |  |
| Weiss advanced spring lever | Decent | OK | 49.1 | 32 | 8.9 | 0.03 | 0.22 | 4/4 |  |
| Adaptive v2 | GaggiMate | CHECK | 32.2 | 40 | 8.7 | 0.25 | 4.99 | 6/6 | flow tracking off by 5.0 ml/s; asks for more flow than the pump gives at pressure |

## Grind per profile

Grind is a puck-resistance multiplier: 1.0 is a normal 9 bar espresso grind, above 1 is finer, below 1 is coarser. It is shown under the profile name on the Machine page.

How each value was set:
- Most come from the profile's design point (`tools/fit_grind.py`): the pressure and flow it is built to run at, plus the grind advice in the author's notes where there is any (e.g. Turbo ending at 3-4.5 ml/s, Hybrid "grind very finely").
- A-Flow, the spring levers and Extractamundo didn't track their targets at those values. Their grinds come from brewing each one in the emulator at several grinds and keeping the best (`tools/sweep_grind.py`).
- The adaptive profiles are set to 1.0 because they adjust to whatever grind you use.

| Profile | By | Grind |
|---|---|---|
| 7g basket | Decent | ×0.92 |
| 80's Espresso | Decent | ×0.94 |
| 9 Bar Espresso | GaggiMate | ×1.0 |
| A-Flow / default-dark | Decent | ×1.0 |
| A-Flow / default-like-dflow | Decent | ×0.85 |
| A-Flow / default-medium | Decent | ×0.85 |
| A-Flow / default-very-dark | Decent | ×0.7 |
| Adaptive v2 | GaggiMate | ×1.0 |
| Adaptive v3 | Decent | ×1.0 |
| Advanced spring lever | Decent | ×1.75 |
| Best overall pressure profile | Decent | ×0.97 |
| Best practice (light roast) | Decent | ×0.82 |
| Blooming | coffee | ×1.0 |
| Blooming Allongé | Decent | ×0.56 |
| Blooming Espresso | Decent | ×0.89 |
| Classic 9 bar | coffee | ×1.0 |
| Classic Italian espresso | Decent | ×1.0 |
| Cremina lever machine | Decent | ×1.0 |
| Cremina lever machine | GaggiMate | ×1.0 |
| Damian's LM Leva | Decent | ×0.95 |
| Damian's LM Leva | GaggiMate | ×0.95 |
| Damian's LRv2 | Decent | ×0.78 |
| Damian's LRv3 | Decent | ×0.89 |
| Damian's Q | Decent | ×1.16 |
| Default | Decent | ×0.98 |
| E61 classic gently up to 10 bar | Decent | ×1.04 |
| E61 espresso machine | Decent | ×1.0 |
| E61 rocketing up to 10 bar | Decent | ×1.2 |
| E61 with fast preinfusion to 9 bar | Decent | ×1.0 |
| Easy blooming - active pressure decline | Decent | ×0.89 |
| Espresso Forge Dark | Decent | ×0.92 |
| Espresso Forge Light | Decent | ×1.04 |
| Extractamundo Dos! | Decent | ×0.7 |
| Flow profile for milky drinks | Decent | ×1.63 |
| Flow profile for straight espresso | Decent | ×0.98 |
| Gentle and sweet | Decent | ×0.82 |
| Gentle flat 2.5 ml per second | Decent | ×0.78 |
| Gentle preinfusion flow profile | Decent | ×0.89 |
| Gentler but still traditional 8.4 bar | Decent | ×0.97 |
| Hybrid pour over espresso | Decent | ×1.2 |
| I got your back | Decent | ×1.0 |
| Innovative long preinfusion | Decent | ×1.1 |
| Italian Australian espresso | Decent | ×0.99 |
| Londonium | Decent | ×0.78 |
| Low pressure lever machine at 6 bar | Decent | ×0.82 |
| Magic (double ristretto) | coffee | ×1.3 |
| Preinfuse then 45ml of water | Decent | ×0.98 |
| Rao Allongé | Decent | ×0.44 |
| Traditional lever machine | Decent | ×1.0 |
| Trendy 6 bar low pressure shot | Decent | ×0.82 |
| Turbo flow | coffee | ×0.35 |
| TurboBloom | Decent | ×0.44 |
| TurboTurbo | Decent | ×0.44 |
| Two spring lever machine to 9 bar | Decent | ×1.0 |
| Weiss advanced spring lever | Decent | ×2.0 |
