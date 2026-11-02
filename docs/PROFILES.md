# Profiles on a Gaggia Classic Pro

Every imported community profile, brewed once on the emulated E24 (warm machine, default grind,
starting at the profile's own temperature) by `tools/profile_check.py`. Re-run it after calibrating the
model to your machine; the numbers below come from the uncalibrated model.

**Columns:** time and yield of the shot, peak pressure, mean pressure/flow tracking error (ignoring each phase's
first 1.5 s, moments where the profile's own flow cap is holding pressure down, and moments where the pump is already
at full stroke rate), and how many phases ran (a weight stop can end a shot before the last phase).

## What the flags mean

- **Allongés (Blooming, Rao):** ask for 4.5+ ml/s at 8–9 bar. A Classic's vibratory pump tops out around 4 ml/s
  at 9 bar (Ulka E5 curve), so these run slower than on Decent's machine. Their large yields (≈130 g) are intended.
- **A-Flow family, spring levers:** pressure lands 0.7–1.6 bar under target with the default puck. These profiles
  expect a finer grind/denser puck; on the model's default grind the puck can't hold that pressure at the flows asked for.
- **Adaptive v2 (GaggiMate):** uses "maintain current flow", which the importer approximates with the previous
  setpoint, so its flow phase asks for more than the pump delivers at pressure.
- **Low-temperature profiles (82–86 °C)** such as 80's Espresso, Extractamundo, Turbo, Damian's Q and Forge Dark
  brew fine, but on a single boiler getting *down* to that temperature takes minutes (or a cooling flush).

| Profile | By | Grade | Time s | Yield g | Peak bar | P err | F err | Phases | Notes |
|---|---|---|---|---|---|---|---|---|---|
| 7g basket | Decent | OK | 23.9 | 17 | 7.7 | 0.12 | 0.16 | 3/3 |  |
| 80's Espresso | Decent | OK | 26.8 | 36 | 9.7 | 0.15 | 0.19 | 4/4 |  |
| 9 Bar Espresso | GaggiMate | OK | 24.9 | 38 | 10.1 | 0.19 | – | 1/1 |  |
| Adaptive v3 | Decent | OK | 30.9 | 36 | 9.6 | 0.27 | 0.43 | 7/7 |  |
| Best overall pressure profile | Decent | OK | 26.3 | 36 | 8.5 | 0.03 | 0.51 | 3/3 |  |
| Best practice (light roast) | Decent | OK | 30.3 | 27 | 9.5 | 0.16 | 0.61 | 5/5 |  |
| Blooming Espresso | Decent | OK | 62.4 | 37 | 8.7 | – | 0.17 | 6/6 |  |
| Classic Italian espresso | Decent | OK | 25.0 | 36 | 9.2 | 0.06 | 0.49 | 2/2 |  |
| Cremina lever machine | Decent | OK | 74.9 | 110 | 9.0 | 0.06 | – | 5/5 |  |
| Cremina lever machine | GaggiMate | OK | 39.0 | 38 | 8.9 | 0.10 | – | 5/5 |  |
| Damian's LM Leva | Decent | OK | 35.6 | 42 | 9.4 | 0.08 | – | 6/6 |  |
| Damian's LM Leva | GaggiMate | OK | 31.8 | 38 | 10.0 | 0.11 | – | 6/6 |  |
| Damian's LRv2 | Decent | OK | 30.3 | 36 | 10.1 | 0.16 | – | 5/7 |  |
| Damian's LRv3 | Decent | OK | 30.4 | 36 | 10.1 | 0.15 | – | 6/8 |  |
| Damian's Q | Decent | OK | 27.9 | 36 | 7.7 | – | 0.07 | 3/3 |  |
| Default | Decent | OK | 25.7 | 36 | 10.0 | 0.13 | 0.51 | 3/3 |  |
| E61 classic gently up to 10 bar | Decent | OK | 24.4 | 36 | 10.2 | 0.17 | 0.50 | 3/3 |  |
| E61 espresso machine | Decent | OK | 24.1 | 36 | 10.1 | 0.20 | – | 1/1 |  |
| E61 rocketing up to 10 bar | Decent | OK | 23.8 | 36 | 10.2 | 0.12 | 0.50 | 2/2 |  |
| E61 with fast preinfusion to 9 bar | Decent | OK | 24.6 | 36 | 10.2 | 0.20 | 0.50 | 2/2 |  |
| Easy blooming - active pressure decline | Decent | OK | 33.7 | 45 | 8.1 | 0.07 | 0.32 | 12/12 |  |
| Espresso Forge Dark | Decent | OK | 32.6 | 42 | 9.4 | 0.15 | 0.22 | 3/4 |  |
| Espresso Forge Light | Decent | OK | 53.0 | 42 | 9.9 | 0.08 | 0.18 | 4/5 |  |
| Extractamundo Dos! | Decent | OK | 51.5 | 40 | 4.8 | – | – | 4/4 |  |
| Flow profile for milky drinks | Decent | OK | 33.2 | 28 | 5.3 | – | 0.14 | 2/2 |  |
| Flow profile for straight espresso | Decent | OK | 27.3 | 36 | 8.6 | – | 0.11 | 2/2 |  |
| Gentle and sweet | Decent | OK | 29.9 | 36 | 6.2 | 0.03 | 0.51 | 3/3 |  |
| Gentle flat 2.5 ml per second | Decent | OK | 32.3 | 36 | 9.7 | – | 0.09 | 2/2 |  |
| Gentle preinfusion flow profile | Decent | OK | 28.5 | 33 | 9.4 | – | 0.08 | 2/2 |  |
| Gentler but still traditional 8.4 bar | Decent | OK | 28.3 | 36 | 9.9 | 0.17 | 0.17 | 2/3 |  |
| Hybrid pour over espresso | Decent | OK | 30.2 | 25 | 9.3 | – | 0.09 | 2/2 |  |
| I got your back | Decent | OK | 29.2 | 36 | 9.4 | – | 0.07 | 3/6 |  |
| Innovative long preinfusion | Decent | OK | 39.1 | 36 | 9.5 | – | 0.14 | 2/2 |  |
| Italian Australian espresso | Decent | OK | 27.2 | 36 | 10.0 | 0.19 | 0.16 | 2/2 |  |
| Londonium | Decent | OK | 32.9 | 42 | 10.1 | 0.15 | – | 5/7 |  |
| Low pressure lever machine at 6 bar | Decent | OK | 40.5 | 33 | 6.9 | 0.07 | 0.16 | 3/3 |  |
| Preinfuse then 45ml of water | Decent | OK | 22.8 | 25 | 8.8 | – | 0.09 | 2/2 |  |
| Traditional lever machine | Decent | OK | 27.6 | 36 | 10.2 | 0.22 | 0.50 | 3/3 |  |
| Trendy 6 bar low pressure shot | Decent | OK | 29.2 | 36 | 7.7 | 0.10 | 0.50 | 2/2 |  |
| TurboBloom | Decent | OK | 33.5 | 39 | 6.5 | 0.04 | 0.06 | 6/6 |  |
| TurboTurbo | Decent | OK | 31.7 | 35 | 6.5 | 0.04 | – | 6/6 |  |
| Two spring lever machine to 9 bar | Decent | OK | 27.9 | 36 | 10.2 | 0.22 | 0.49 | 3/3 |  |
| A-Flow / default-dark | Decent | CHECK | 36.6 | 36 | 7.2 | 1.65 | 0.10 | 6/6 | pressure tracking off by 1.6 bar |
| A-Flow / default-like-dflow | Decent | CHECK | 37.2 | 36 | 9.3 | 0.69 | 0.09 | 6/6 | pressure tracking off by 0.7 bar |
| A-Flow / default-medium | Decent | CHECK | 40.6 | 42 | 9.3 | 0.74 | 0.08 | 6/6 | pressure tracking off by 0.7 bar |
| A-Flow / default-very-dark | Decent | CHECK | 29.8 | 32 | 9.2 | 1.64 | 0.07 | 6/6 | pressure tracking off by 1.6 bar |
| Adaptive v2 | GaggiMate | CHECK | 31.2 | 40 | 9.5 | 0.78 | 4.64 | 6/6 | pressure tracking off by 0.8 bar; flow tracking off by 4.6 ml/s; asks for more flow than the pump gives at pressure |
| Advanced spring lever | Decent | CHECK | 25.9 | 32 | 10.0 | 0.72 | 0.16 | 5/5 | pressure tracking off by 0.7 bar |
| Blooming Allongé | Decent | CHECK | 96.8 | 135 | 8.7 | – | 0.87 | 6/6 | flow tracking off by 0.9 ml/s; asks for more flow than the pump gives at pressure |
| Rao Allongé | Decent | CHECK | 61.9 | 129 | 8.7 | – | 1.88 | 2/2 | flow tracking off by 1.9 ml/s; asks for more flow than the pump gives at pressure |
| Weiss advanced spring lever | Decent | CHECK | 25.8 | 32 | 10.0 | 0.73 | 0.36 | 4/4 | pressure tracking off by 0.7 bar |

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
