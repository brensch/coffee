//! Replay a real (or emulated) engine log open-loop through the model and fit
//! model parameters so the simulated sensors match the logged ones.
//!
//! The log is the binary `.clog` written by `coffee-engine --log` and streamed by
//! the firmware. Tick i holds the sensors the engine saw and the actuators it
//! commanded; replay feeds the actuators to the model and compares sensors.

use anyhow::{Context, Result, bail};
use coffee_io::{ads1115, input, max31855};
use coffee_physics::{Actuators, Machine, ModelConfig};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Row {
    pub t: f64,
    pub act: Actuators,
    pub inputs: u8,
    pub tc: Option<f64>,
    pub p: Option<f64>,
    pub scale: Option<f64>,
}

/// Read a `.clog` (from `coffee-engine --log` or `coffee-emu capture`). Raw
/// sensor words are converted with the model's own sensor parameters.
pub fn load_log(path: &Path, cfg: &ModelConfig) -> Result<Vec<Row>> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let frames = coffee_node::tlog::read(&data);
    if frames.is_empty() {
        bail!("{} contains no log chunks", path.display());
    }
    let t0 = frames[0].0.t_us;
    Ok(frames
        .iter()
        .map(|(s, o)| {
            let v_adc = ads1115::counts_to_volts(s.pressure_adc) as f64;
            Row {
                t: (s.t_us - t0) as f64 * 1e-6,
                act: Actuators {
                    heater: o.heater,
                    pump: o.pump,
                    valve: o.valve,
                },
                inputs: s.inputs,
                tc: max31855::decode(s.max31855)
                    .ok()
                    .map(|r| r.thermocouple_c as f64),
                p: (v_adc > 0.15).then(|| counts_to_bar(cfg, s.pressure_adc)),
                scale: (s.inputs & input::SCALE_OK != 0).then(|| s.scale_mg as f64 / 1000.0),
            }
        })
        .collect())
}

#[derive(Clone, Debug, Default)]
pub struct Simulated {
    pub tc: Vec<Option<f64>>,
    pub p: Vec<Option<f64>>,
    pub scale: Vec<Option<f64>>,
}

fn counts_to_bar(cfg: &ModelConfig, c: i16) -> f64 {
    let ps = &cfg.pressure_sensor;
    let v = ads1115::counts_to_volts(c) as f64 / ps.divider;
    (v - ps.v_zero) / (ps.v_full - ps.v_zero) * ps.range_bar
}

pub fn simulate(cfg: &ModelConfig, rows: &[Row], seed: u64) -> Simulated {
    let mut cfg = cfg.clone();
    if let Some(t0) = rows.iter().find_map(|r| r.tc) {
        cfg.sim.initial_temp_c = t0;
    }
    let mut m = Machine::new(cfg.clone(), seed);
    let mut out = Simulated::default();
    for r in rows {
        m.switch_brew = r.inputs & input::BREW != 0;
        m.switch_steam = r.inputs & input::STEAM != 0;
        m.switch_water = r.inputs & input::WATER != 0;
        let s = m.sensor_frame();
        out.tc.push(
            max31855::decode(s.max31855)
                .ok()
                .map(|x| x.thermocouple_c as f64),
        );
        out.p.push(Some(counts_to_bar(&cfg, s.pressure_adc)));
        out.scale
            .push((s.inputs & input::SCALE_OK != 0).then(|| s.scale_mg as f64 / 1000.0));
        m.step_half_cycle(r.act);
    }
    out
}

#[derive(Clone, Copy, Debug)]
pub struct Weights {
    pub temp: f64,
    pub pressure: f64,
    pub weight: f64,
}

fn rmse(a: &[Option<f64>], b: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let mut s = 0.0;
    let mut n = 0usize;
    for (x, y) in a.iter().zip(b) {
        if let (Some(x), Some(y)) = (x, y) {
            s += (x - y) * (x - y);
            n += 1;
        }
    }
    (n > 0).then(|| (s / n as f64).sqrt())
}

/// Per-signal RMSE (C, bar, g) and the weighted normalised cost.
pub fn score(sim: &Simulated, rows: &[Row], w: Weights) -> (f64, [Option<f64>; 3]) {
    let et = rmse(&sim.tc, rows.iter().map(|r| r.tc));
    let ep = rmse(&sim.p, rows.iter().map(|r| r.p));
    let ew = rmse(&sim.scale, rows.iter().map(|r| r.scale));
    let mut cost = 0.0;
    let mut wsum = 0.0;
    for (e, wt, unit) in [
        (et, w.temp, 1.0),
        (ep, w.pressure, 0.2),
        (ew, w.weight, 1.0),
    ] {
        if let Some(e) = e
            && wt > 0.0
        {
            cost += wt * e / unit;
            wsum += wt;
        }
    }
    (
        if wsum > 0.0 {
            cost / wsum
        } else {
            f64::INFINITY
        },
        [et, ep, ew],
    )
}

/// Downhill simplex. Minimises `f` starting at `x0` with initial step `step`.
pub fn nelder_mead(
    f: &mut dyn FnMut(&[f64]) -> f64,
    x0: &[f64],
    step: f64,
    iters: usize,
    mut on_iter: impl FnMut(usize, f64),
) -> (Vec<f64>, f64) {
    let n = x0.len();
    let mut simplex: Vec<(Vec<f64>, f64)> = vec![];
    simplex.push((x0.to_vec(), f(x0)));
    for i in 0..n {
        let mut x = x0.to_vec();
        x[i] += step;
        let fx = f(&x);
        simplex.push((x, fx));
    }
    for it in 0..iters {
        simplex.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        on_iter(it, simplex[0].1);
        let spread = simplex[n].1 - simplex[0].1;
        if spread.abs() < 1e-6 {
            break;
        }
        let centroid: Vec<f64> = (0..n)
            .map(|j| simplex[..n].iter().map(|s| s.0[j]).sum::<f64>() / n as f64)
            .collect();
        let worst = simplex[n].clone();
        let lerp = |t: f64| -> Vec<f64> {
            (0..n)
                .map(|j| centroid[j] + t * (worst.0[j] - centroid[j]))
                .collect()
        };
        let xr = lerp(-1.0);
        let fr = f(&xr);
        if fr < simplex[0].1 {
            let xe = lerp(-2.0);
            let fe = f(&xe);
            simplex[n] = if fe < fr { (xe, fe) } else { (xr, fr) };
        } else if fr < simplex[n - 1].1 {
            simplex[n] = (xr, fr);
        } else {
            let (xc, fc) = if fr < worst.1 {
                let x = lerp(-0.5);
                let fx = f(&x);
                (x, fx)
            } else {
                let x = lerp(0.5);
                let fx = f(&x);
                (x, fx)
            };
            if fc < worst.1.min(fr) {
                simplex[n] = (xc, fc);
            } else {
                // shrink towards best
                let best = simplex[0].0.clone();
                for s in simplex.iter_mut().skip(1) {
                    for (x, b) in s.0.iter_mut().zip(&best) {
                        *x = b + 0.5 * (*x - b);
                    }
                    s.1 = f(&s.0);
                }
            }
        }
    }
    simplex.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    simplex.swap_remove(0)
}

pub struct FitResult {
    pub cfg: ModelConfig,
    pub before: (f64, [Option<f64>; 3]),
    pub after: (f64, [Option<f64>; 3]),
    pub params: Vec<(String, f64, f64)>,
}

/// Fit `params` (dotted model paths). Positive parameters are searched in log
/// space, so the simplex steps are relative (+/-25%).
pub fn fit(
    base: &ModelConfig,
    rows: &[Row],
    params: &[String],
    w: Weights,
    iters: usize,
    seed: u64,
    verbose: bool,
) -> Result<FitResult> {
    let init: Vec<f64> = params
        .iter()
        .map(|p| base.get_path(p))
        .collect::<Result<_>>()?;
    for (p, v) in params.iter().zip(&init) {
        if *v <= 0.0 {
            bail!("parameter `{p}` = {v}: only positive parameters can be fitted");
        }
    }
    let before = score(&simulate(base, rows, seed), rows, w);
    let make = |x: &[f64]| -> Result<ModelConfig> {
        let mut c = base.clone();
        for ((p, v0), xi) in params.iter().zip(&init).zip(x) {
            c.set_path(p, v0 * xi.exp())?;
        }
        Ok(c)
    };
    let mut evals = 0usize;
    let mut f = |x: &[f64]| -> f64 {
        evals += 1;
        match make(x) {
            Ok(c) => score(&simulate(&c, rows, seed), rows, w).0,
            Err(_) => f64::INFINITY,
        }
    };
    let x0 = vec![0.0; params.len()];
    let (xbest, _) = nelder_mead(&mut f, &x0, 0.25, iters, |it, best| {
        if verbose {
            eprintln!("  iter {it:3}  cost {best:.4}");
        }
    });
    let cfg = make(&xbest)?;
    let after = score(&simulate(&cfg, rows, seed), rows, w);
    let params = params
        .iter()
        .zip(&init)
        .zip(&xbest)
        .map(|((p, v0), x)| (p.clone(), *v0, v0 * x.exp()))
        .collect();
    Ok(FitResult {
        cfg,
        before,
        after,
        params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nelder_mead_finds_quadratic_min() {
        let mut f = |x: &[f64]| (x[0] - 1.0).powi(2) + 10.0 * (x[1] + 2.0).powi(2);
        let (x, fx) = nelder_mead(&mut f, &[0.0, 0.0], 0.5, 300, |_, _| {});
        assert!(fx < 1e-6, "{fx}");
        assert!((x[0] - 1.0).abs() < 1e-2 && (x[1] + 2.0).abs() < 1e-2);
    }

    /// Generate a synthetic "real" log from a known model, perturb the model, and
    /// check the fit recovers the true parameters.
    #[test]
    fn recovers_thermal_parameters() {
        let mut truth = ModelConfig::default();
        truth.thermal.boiler_mass_kg = 1.6;
        truth.thermal.loss_boiler_w_k = 0.8;
        // Bang-bang heater schedule: 90 s on, then 60 s off, then 30 s on.
        let mut rows = vec![];
        let n = 18_000;
        for i in 0..n {
            let t = i as f64 * 0.01;
            let heater = t < 90.0 || (150.0..180.0).contains(&t);
            rows.push(Row {
                t,
                act: Actuators {
                    heater,
                    pump: false,
                    valve: false,
                },
                inputs: 0,
                tc: None,
                p: None,
                scale: None,
            });
        }
        let sim = simulate(&truth, &rows, 7);
        for (r, tc) in rows.iter_mut().zip(sim.tc) {
            r.tc = tc;
        }
        let mut guess = truth.clone();
        guess.thermal.boiler_mass_kg = 1.1;
        guess.thermal.loss_boiler_w_k = 0.5;
        let w = Weights {
            temp: 1.0,
            pressure: 0.0,
            weight: 0.0,
        };
        let r = fit(
            &guess,
            &rows,
            &[
                "thermal.boiler_mass_kg".into(),
                "thermal.loss_boiler_w_k".into(),
            ],
            w,
            60,
            7,
            false,
        )
        .unwrap();
        assert!(
            r.after.0 < r.before.0 * 0.3,
            "before {:?} after {:?}",
            r.before,
            r.after
        );
        let mass = r.cfg.thermal.boiler_mass_kg;
        assert!((mass - 1.6).abs() < 0.15, "fitted mass {mass}");
    }
}
