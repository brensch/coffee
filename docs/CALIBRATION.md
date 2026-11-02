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
| `mains.voltage_v`, `frequency_hz` | Multimeter on the outlet. | The firmware also measures frequency from zero crossings. |
| `pressure_sensor.offset_bar` | Read pressure with the machine cold and the valve open. | Should be 0 ± 0.05 bar. |

## Logs to record (power on)

| # | Test | Duration | Fits | Also check |
|---|---|---|---|---|
| 1 | **Cold heat-up, idle.** Machine cold since the night before, portafilter locked in. | 30 min | `thermal.boiler_mass_kg, loss_boiler_w_k, h_boiler_water_w_k, portafilter_*`, `thermocouple.tau_s` | Power meter: average watts at idle (should match `loss_*` × ΔT). |
| 2 | **Idle pressure.** Same log: watch pressure while heating and afterwards. | (in 1) | `hydraulics.seep_ml_s_per_bar`, `compliance_ml_per_bar` | Does it climb to 2–4 bar and bleed off? |
| 3 | **Free flow.** No portafilter, pump at 100% into a jug for 10 s. Weigh it. | 10 s ×3 | `pump.wear` (curve scale at ~0 bar) | Expect ~10–11 ml/s at 230 V. |
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
- **Zero-cross pulse offset:** needs an oscilloscope (sets the firmware's
  `OUTPUT_LEAD_US`). The default of 1.5 ms works for typical opto detectors.
