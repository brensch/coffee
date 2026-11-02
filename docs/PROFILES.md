# Profiles on a Gaggia Classic Pro

Every built-in and imported community profile, brewed once on the emulated E24 by
`tools/profile_check.py`: warm machine, **each profile's own grind** (table at the bottom), starting
at the profile's own temperature. Re-run it after calibrating the model to your machine. The
numbers below come from the uncalibrated model.

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
  around 4 ml/s at 9 bar (Ulka E5 curve), so these run slower than on Decent's machine. Their large
  yields (≈130 g) are intended.
- **Low-temperature profiles (82–86 °C)**, such as 80's Espresso, Extractamundo, Turbo, Damian's Q
  and Forge Dark, brew fine. But on a single boiler, getting *down* to that temperature takes
  minutes, or a cooling flush.
- **Per-step temperatures** in Decent profiles (Extractamundo programs a drop) aren't
  representable. The first step's temperature is used for the whole shot.

## Results

**Profile check: 55 profiles (54 OK, 1 CHECK)**

| Profile | By | Grade | Time s | Yield g | Peak bar | P err | F err | Phases | Notes |
|---|---|---|---|---|---|---|---|---|---|
| 7g basket | Decent | OK | 23.9 | 19 | 7.7 | 0.13 | 0.15 | 3/3 |  |
| 80's Espresso | Decent | OK | 25.2 | 36 | 9.7 | 0.16 | 0.19 | 4/4 |  |
| 9 Bar Espresso | GaggiMate | OK | 24.9 | 38 | 10.1 | 0.19 | – | 1/1 |  |
| A-Flow / default-dark | Decent | OK | 53.8 | 36 | 9.4 | 0.06 | 0.12 | 6/6 |  |
| A-Flow / default-like-dflow | Decent | OK | 46.9 | 36 | 10.0 | 0.01 | 0.11 | 6/6 |  |
| A-Flow / default-medium | Decent | OK | 59.2 | 42 | 10.1 | 0.05 | 0.15 | 6/6 |  |
| A-Flow / default-very-dark | Decent | OK | 46.0 | 32 | 10.2 | 0.21 | 0.16 | 6/6 |  |
| Adaptive v3 | Decent | OK | 30.9 | 36 | 9.6 | 0.27 | 0.42 | 7/7 |  |
| Advanced spring lever | Decent | OK | 35.7 | 32 | 10.2 | 0.08 | 0.08 | 4/5 |  |
| Best overall pressure profile | Decent | OK | 25.7 | 36 | 8.5 | 0.02 | 0.50 | 3/3 |  |
| Best practice (light roast) | Decent | OK | 29.1 | 31 | 9.8 | 0.16 | – | 5/5 |  |
| Blooming |  | OK | 46.5 | 40 | 8.9 | 0.05 | 0.24 | 4/4 |  |
| Blooming Allongé | Decent | OK | 79.5 | 135 | 8.2 | – | 0.14 | 6/6 |  |
| Blooming Espresso | Decent | OK | 62.6 | 42 | 8.5 | – | 0.11 | 6/6 |  |
| Classic 9 bar |  | OK | 29.4 | 36 | 9.0 | 0.04 | 0.32 | 4/4 |  |
| Classic Italian espresso | Decent | OK | 25.0 | 36 | 9.2 | 0.04 | 0.49 | 2/2 |  |
| Cremina lever machine | Decent | OK | 74.9 | 110 | 9.0 | 0.06 | – | 5/5 |  |
| Cremina lever machine | GaggiMate | OK | 39.0 | 38 | 8.9 | 0.10 | – | 5/5 |  |
| Damian's LM Leva | Decent | OK | 33.9 | 42 | 9.4 | 0.08 | – | 6/6 |  |
| Damian's LM Leva | GaggiMate | OK | 30.5 | 38 | 10.0 | 0.11 | – | 6/6 |  |
| Damian's LRv2 | Decent | OK | 25.6 | 36 | 9.9 | 0.24 | – | 6/7 |  |
| Damian's LRv3 | Decent | OK | 27.8 | 36 | 10.0 | 0.17 | – | 6/8 |  |
| Damian's Q | Decent | OK | 29.8 | 36 | 9.1 | – | 0.06 | 3/3 |  |
| Default | Decent | OK | 25.2 | 36 | 10.1 | 0.13 | 0.50 | 3/3 |  |
| E61 classic gently up to 10 bar | Decent | OK | 25.2 | 36 | 10.2 | 0.17 | 0.52 | 3/3 |  |
| E61 espresso machine | Decent | OK | 24.1 | 36 | 10.1 | 0.20 | – | 1/1 |  |
| E61 rocketing up to 10 bar | Decent | OK | 28.1 | 36 | 10.3 | 0.18 | 0.53 | 2/2 |  |
| E61 with fast preinfusion to 9 bar | Decent | OK | 24.6 | 36 | 10.2 | 0.20 | 0.50 | 2/2 |  |
| Easy blooming - active pressure decline | Decent | OK | 30.4 | 45 | 8.1 | 0.09 | 0.36 | 12/12 |  |
| Espresso Forge Dark | Decent | OK | 29.8 | 42 | 9.5 | 0.17 | 0.24 | 3/4 |  |
| Espresso Forge Light | Decent | OK | 54.0 | 42 | 9.9 | 0.08 | 0.18 | 4/5 |  |
| Extractamundo Dos! | Decent | OK | 24.1 | 40 | 6.0 | – | – | 4/4 |  |
| Flow profile for milky drinks | Decent | OK | 32.9 | 21 | 8.3 | – | 0.11 | 2/2 |  |
| Flow profile for straight espresso | Decent | OK | 27.0 | 36 | 8.5 | – | 0.13 | 2/2 |  |
| Gentle and sweet | Decent | OK | 24.7 | 36 | 6.2 | 0.03 | 0.51 | 3/3 |  |
| Gentle flat 2.5 ml per second | Decent | OK | 29.1 | 36 | 8.5 | – | 0.09 | 2/2 |  |
| Gentle preinfusion flow profile | Decent | OK | 28.4 | 36 | 8.6 | – | 0.08 | 2/2 |  |
| Gentler but still traditional 8.4 bar | Decent | OK | 27.7 | 36 | 9.9 | 0.18 | 0.17 | 2/3 |  |
| Hybrid pour over espresso | Decent | OK | 29.8 | 19 | 9.7 | – | 0.10 | 2/2 |  |
| I got your back | Decent | OK | 29.2 | 36 | 9.4 | – | 0.07 | 3/6 |  |
| Innovative long preinfusion | Decent | OK | 40.8 | 36 | 9.5 | – | 0.14 | 2/2 |  |
| Italian Australian espresso | Decent | OK | 27.0 | 36 | 10.0 | 0.19 | 0.17 | 2/2 |  |
| Londonium | Decent | OK | 29.6 | 42 | 9.9 | 0.21 | – | 6/7 |  |
| Low pressure lever machine at 6 bar | Decent | OK | 29.8 | 36 | 6.8 | 0.08 | 0.18 | 3/3 |  |
| Magic (double ristretto) |  | OK | 30.6 | 28 | 9.1 | 0.02 | 0.36 | 4/4 |  |
| Preinfuse then 45ml of water | Decent | OK | 22.8 | 26 | 8.6 | – | 0.10 | 2/2 |  |
| Rao Allongé | Decent | OK | 37.2 | 135 | 8.3 | – | 0.10 | 2/2 |  |
| Traditional lever machine | Decent | OK | 27.6 | 36 | 10.2 | 0.22 | 0.50 | 3/3 |  |
| Trendy 6 bar low pressure shot | Decent | OK | 24.2 | 36 | 7.7 | 0.12 | 0.50 | 2/2 |  |
| Turbo flow |  | OK | 16.4 | 45 | 5.6 | – | 0.10 | 2/2 |  |
| TurboBloom | Decent | OK | 15.5 | 39 | 6.5 | 0.09 | 0.23 | 6/6 |  |
| TurboTurbo | Decent | OK | 14.0 | 35 | 6.2 | 0.02 | – | 5/6 |  |
| Two spring lever machine to 9 bar | Decent | OK | 27.9 | 36 | 10.2 | 0.22 | 0.49 | 3/3 |  |
| Weiss advanced spring lever | Decent | OK | 48.8 | 32 | 10.3 | 0.08 | 0.25 | 4/4 |  |
| Adaptive v2 | GaggiMate | CHECK | 31.2 | 40 | 9.5 | 1.90 | 1.23 | 6/6 | pressure tracking off by 1.9 bar; flow tracking off by 1.2 ml/s |

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
| A-Flow / default-dark | Decent | ×2.0 |
| A-Flow / default-like-dflow | Decent | ×1.5 |
| A-Flow / default-medium | Decent | ×1.75 |
| A-Flow / default-very-dark | Decent | ×2.0 |
| Adaptive v2 | GaggiMate | ×1.0 |
| Adaptive v3 | Decent | ×1.0 |
| Advanced spring lever | Decent | ×1.5 |
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
