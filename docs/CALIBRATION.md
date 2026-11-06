# Measuring the machine

Every model parameter, how to measure it, and with what. Most come out of a few
logs. Record them with `coffee-emu capture --port /dev/ttyACM0 --out <name>.clog`
while the firmware runs, then use `coffee-emu fit --log … --params …` (see
[MODELS.md](MODELS.md)).

Tools: a plug-in power meter, a multimeter, a fast probe thermometer, a 0.1 g
scale, a blind basket and a stopwatch.

## Before power-on (unplugged, multimeter)

| Parameter | How | Notes |
|---|---|---|
| `heater.power_w` | Element resistance R. P = V²/R at your mains V. | Cross-check with the power meter during heat-up. |
| `mains.voltage_v`, `frequency_hz` | Multimeter on the outlet. | Set the engine's `mains_hz` to match (60 in the US). It is the tick rate and sets the volume of one pump stroke. |

## Sensor calibration (engine config, once at install)

The engine converts the two ADC channels itself, so these go in the engine
config (`PUT /api/config`, or the config file), not the model.

### Temperature: thermocouple, two points (room temperature and boiling water)

The K-type probe's EMF (MCP3421) and the cold junction (TMP117 beside the probe's socket) are
converted with the NIST ITS-90 type-K functions (`coffee_io::typek`), after dividing out the input
network's gain (`thermocouple.input_gain`, 0.993: the 2 × 1k filter against the ADC's input).
Then a linear correction `T = thermocouple.cal_gain × T_tc + thermocouple.cal_offset_c`. That
correction absorbs the probe's alloy tolerance (standard K: ±2.2 °C or 0.75 %), the
TMP117's offset (±0.1 °C), and how well the probe is coupled to the boiler.

Both points are taken **on the bench, board on USB, before the probe is fitted** (INSTALL §3):
no mains, no heater, nothing live, and the probe is in your hand.
1. **Room point.** Probe, board and a reference thermometer side by side at room temperature for
   10 minutes. Note the thermometer's reading `R1` and the engine's raw `temp_c` `M1` (with gain 1
   and offset 0). With probe and board at the same temperature the EMF is ~0, so this point is
   mostly the TMP117 itself: `M1` within ~0.3 °C of `R1` is normal.
2. **Boiling point.** Hold the probe's tip in a pot at a rolling boil for 2 minutes (keep the
   board and the lead's plug away from the steam). Read `M2`. The true temperature is the
   boiling point at your altitude, not 100 °C:
   `R2 ≈ 100 − 0.0033 × altitude_m` °C. That is about 1 °C per 300 m, so
   Denver (1600 m) boils at about 94.7 °C. On a day of unusual weather, use the
   local station pressure: `R2 = 100 + 0.0277 × (p_hPa − 1013.25)`.
3. `cal_gain = (R2 − R1) / (M2 − M1)` and `cal_offset_c = R1 − cal_gain × M1`.
   Typical values are within 1 ± 0.02 and ±1.5 °C. Anything far outside that points to a
   probe that isn't K-type, or wires extended with copper somewhere (a second, unmeasured
   junction: the probe's own wires must reach the board's socket).
4. Then fit the probe (INSTALL §4): screwed into the brew-thermostat seat, a little thermal paste
   on the thread, hand-tight plus a nip. The boiler-wall offset that remains
   is what the water observer handles. Validate it with log test 5.

The boiling point is close to the 93 °C brew range, so the calibration is most
accurate where it matters. The steam setpoint (140 °C) is an extrapolation;
expect ±1–2 °C there.

Fault thresholds (engine config, range-checked): `open_uv` (15 mV: an open probe reads ~90 mV
through the board's 10 MΩ bias), `reversed_uv` (−1.5 mV: wires swapped), `stale_s` (1 s without a
conversion: TC_ADC), `cj_stale_s` and `cj_min_c`/`cj_max_c` (the TMP117: COLD_JUNCTION).

### Pressure: zero and span

The transducer reads `bar = span_gain × (nominal − zero_offset_bar)`.

- **Zero (install).** Machine cold and unpressurised: open the steam wand, or
  run a brief flush and let the valve vent. Read `pressure_bar` with
  `zero_offset_bar = 0` and enter that as `zero_offset_bar`.
- **Power-on self-zero.** Each boot, the engine averages the first 0.5 s. It
  adopts that average as the zero only if the boiler is cold (probe < 50 °C)
  *and* the reading is within 0.3 bar of the install zero. A hot boiler holds
  2–4 bar behind the closed valve, and a large offset means a stuck valve or a
  bad sensor, not drift. Otherwise the install zero stays.
- **Span.** Blind basket against a reference gauge (a portafilter gauge from a
  Gaggia OPV kit): note the gauge `G` and the engine's `P` at the OPV plateau,
  then `span_gain = G / P`. Without a reference, leave it at 1. The
  transducer's 1 % span tolerance is smaller than the OPV's own spread.

## Logs to record (power on)

| # | Test | Duration | Fits | Also check |
|---|---|---|---|---|
| 1 | **Cold heat-up, idle.** Machine cold since the night before, portafilter locked in. | 30 min | `thermal.boiler_mass_kg, loss_boiler_w_k, h_boiler_water_w_k, portafilter_*`, `thermocouple.tau_s`, `thermocouple.offset_c` | Power meter: average watts at idle (should match `loss_*` × ΔT). |
| 2 | **Idle pressure.** Same log: watch pressure while heating and afterwards. | (in 1) | `hydraulics.seep_ml_s_per_bar`, `compliance_ml_per_bar` | Does it climb to 2–4 bar and bleed off? |
| 3 | **Free flow.** No portafilter, pump at 100% into a jug for 10 s. Weigh it. | 10 s ×3 | `pump.wear` (curve scale at ~0 bar) | Expect roughly 10 ml/s (the Ulka's free-flow rate; fit it from your own jug). |
| 4 | **Blind basket.** Pump 100% for 15 s. | 15 s | `hydraulics.opv_crack_bar, opv_k_ml_s_per_bar, compliance_ml_per_bar` | Steady pressure = the OPV setting. |
| 5 | **Flush temperature.** No portafilter. Flush 100 ml while the probe thermometer sits in the stream. | ×3 | `thermal.h_boiler_water_w_k`, mixing | Brew-water temperature vs `brew.water_temp`. |
| 6 | **Shots with the scale**, normal dose, 2–3 grind settings. | 5–10 shots | `puck.r_per_g, compression_per_bar, erosion_per_ml`, `cup.drip_tau_s` | Thermometer in the first drips of one shot. |
| 7 | **Scale latency.** Drop a 20 g weight on the scale while logging. | 1 min | `scale.latency_s` | The firmware sees the step; compare timestamps. |
| 8 | **Steam.** Steam mode, open the wand for 30 s. | 3 min | `hydraulics.wand_vapor_g_s_per_bar` | Weigh the tank before and after. |
| 9 | **Portafilter warm-up.** From cold, probe thermometer touching the inside of the (empty, locked-in) basket; note its reading every minute for 20 min. | 20 min | `thermal.h_boiler_portafilter_w_k`, `loss_portafilter_w_k`; engine `group_tau_s`, `group_ready_band_c` | This sets when "Ready" is honest. The engine's group estimate should track your readings. |

Fit in this order: 1–2 (thermal), then 3–4 (hydraulics), then 5 (heat transfer),
then 6 (puck). Re-run `coffee-emu test` after each fit. Then copy the fitted
`h_boiler_water_w_k` into the engine's `brew_boost_h_w_k` and
`water_heat_capacity_j_k` (from `boiler_capacity_ml` × 4.186).

## Can't be measured in v1 (estimated, flagged in the UI)

- **True water temperature:** the engine estimates it. Validate with test 5.
- **Puck saturation, OPV flow, reservoir level:** emulator-only. They show as
  "?" on the Machine page.
- **Grid phase:** there is no zero-cross detector. The zero-cross SSRs switch at
  crossings on their own, so there is nothing to tune.

### Scale (built-in drip-tray scale)

Two load cells, each with its own HX711 channel; weight = Σ (raw − `scale.zero[i]`) ×
`scale.mg_per_count[i]` (engine config). The engine tares at the start of every shot, so the zero
only matters for the displayed weight.
1. Empty tray in place: note both raw readings (`coffee status` shows them) and store them as
   `scale.zero`.
2. Put a known weight (e.g. 200 g) over the **left** cell only, read raw 1; then over the right
   cell, raw 2. `mg_per_count[i] = 200000 / (raw_i − zero_i)`; a negative result just means that
   cell is mounted the other way round.
3. Check with the weight in the middle: the reading should be the weight ± 0.5 g.

