# Models and calibration

Implementation: [`crates/coffee-physics/src/machine.rs`](../crates/coffee-physics/src/machine.rs). Parameters: `models/*.toml`. The machine steps once per mains cycle, as a positive and a negative half-cycle of 20 substeps each (~0.4 ms at 60 Hz).

## Components and coupling

| Component | Model | Source / status |
|---|---|---|
| Mains | RMS voltage and frequency. Heater power V²/R; pump strokes scale with V and V². | Physics |
| Pump | Ulka E5 average flow vs pressure. One sinusoidal stroke per commanded cycle, in its positive half only (the pump's diode). Wear and air-lock faults; an empty reservoir stops flow. | [Ulka E family datasheet](https://cdn.shopify.com/s/files/1/0632/3673/5225/files/Pump_E_Family_Data_Sheet.pdf): 650 cc/min at 0 bar … 0 at 15 bar |
| Over-pressure valve | Soft-knee relief, `k · w · softplus((P − P_crack)/w)`. | Estimate: fit from a blind-basket log |
| Pressure node | Liquid compliance (ml/bar), floored at the water's saturation pressure. Includes thermal expansion of trapped water. A steam headspace forms when water is drawn below capacity. | Steam tables (Antoine), spot-checked in tests |
| Thermal | Boiler body (with element) ↔ water ↔ portafilter, all losing heat to ambient. Cold inflow mixes into the water. Brew water exchanges heat with the portafilter on the way out (ε-NTU). Steam leaving removes latent heat. Exposed element (low water) cuts body-to-water heat transfer. | **Estimates**: boiler mass, heat transfer and losses need fitting |
| Puck | Headspace fill, then a wetting front (resistance grows with wetted depth, no drips), then Darcy flow. Resistance rises with pressure (compaction), swells early and erodes with throughput. Channelling multiplier. | Shape from Darcy/poroelastic literature ([Barista Hustle](https://www.baristahustle.com/?p=74659), [arXiv 2512.21528](https://arxiv.org/pdf/2512.21528)); numbers are estimates |
| Cup / scale | Drip lag, dissolved solids, Bluetooth scale latency, quantisation and noise. | Estimates |
| Sensors | NTC: probe lag, the real part's beta curve and divider, placement offset. Its node voltage is averaged over the cycle, with noise and 1 mV quantisation. Faults: `ntc.fault = ok, open, short, detached, stuck`, plus EMI bursts. Pressure: transducer lag, noise and zero drift, the ×2/3 divider, cycle-averaged mV. Faults: disconnected, stuck. | Datasheets |
| Protection | Bimetal cutout with hysteresis, one-shot fuse. | Estimates |

The model is deliberately simple where we lack data. **Full mixing** of cold inflow into the boiler water is pessimistic: real boilers stratify somewhat. That's the first thing to check against a real thermometer in the cup.

## Calibration workflow

`fit` and `replay` decode the logged millivolts with the *base* model's sensor curves, applied to the log and the simulation alike. So fitting `thermistor.*` or `pressure_sensor.*` moves only the simulated side.

Record logs with `coffee-emu capture --port /dev/ttyACM0 --out x.clog` from the firmware, or `coffee-engine --log x.clog` on the PC.

1. **Heat-up log**, cold start with the machine idle. Fit `thermal.boiler_mass_kg,thermal.loss_boiler_w_k,thermal.h_boiler_water_w_k,thermistor.tau_s` (add `thermistor.beta` if the part strays from its datasheet).
2. **Blind basket**, pump at full power. Fit `hydraulics.opv_crack_bar,hydraulics.opv_k_ml_s_per_bar,hydraulics.compliance_ml_per_bar,pump.wear`.
3. **Shots with a scale.** Fit `puck.r_per_g,puck.compression_per_bar,puck.erosion_per_ml,cup.drip_tau_s` with `--w-weight 1`.
4. **Water in the cup** (thermometer, or a Scace device if you can borrow one). Compare against `brew.water_temp` and adjust `thermal.screen_ua_w_k` and the mixing assumptions.

Example:

```sh
coffee-emu fit --log logs/heatup.clog --model models/gaggia_classic_pro_e24_120v.toml \
  --params thermal.boiler_mass_kg,thermal.loss_boiler_w_k,thermal.h_boiler_water_w_k --w-pressure 0 --out models/my_e24.toml
coffee-emu replay --log logs/heatup.clog --model models/my_e24.toml  # sim vs real export
```

After fitting, copy the fitted heat-transfer value into the engine's `brew_boost_h_w_k` so the body boost matches your machine, then re-run `coffee-emu test`.
