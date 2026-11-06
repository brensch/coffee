//! Type K thermocouple, NIST ITS-90 reference functions (NIST Monograph 175; srdata.nist.gov/its90).
//!
//! [`emf_mv`] is the reference function E(t) for a junction at `t` against 0 °C, and [`temp_c`] its
//! published inverse. Shared by the engine (cold-junction compensation) and the emulator (making the
//! EMF the board would see), so both sides use exactly the same curve. f64 throughout: the C6 has no
//! FPU at all, and this runs at the ADC's 3.75 conversions per second.

/// E(t) for -270..0 °C (mV), c0..c10.
const NEG: [f64; 11] = [
    0.0,
    0.394501280250e-01,
    0.236223735980e-04,
    -0.328589067840e-06,
    -0.499048287770e-08,
    -0.675090591730e-10,
    -0.574103274280e-12,
    -0.310888728940e-14,
    -0.104516093650e-16,
    -0.198892668780e-19,
    -0.163226974860e-22,
];
/// E(t) for 0..1372 °C (mV), c0..c9, plus `A0 * exp(A1 * (t - A2)^2)`.
const POS: [f64; 10] = [
    -0.176004136860e-01,
    0.389212049750e-01,
    0.185587700320e-04,
    -0.994575928740e-07,
    0.318409457190e-09,
    -0.560728448890e-12,
    0.560750590590e-15,
    -0.320207200030e-18,
    0.971511471520e-22,
    -0.121047212750e-25,
];
const A: [f64; 3] = [0.118597600000e+00, -0.118343200000e-03, 0.126968600000e+03];

/// Inverse, -5.891..0 mV (-200..0 °C), d0..d8. NIST: within 0.04 °C of the reference function.
const INV_NEG: [f64; 9] = [
    0.0, 2.5173462e+01, -1.1662878e+00, -1.0833638e+00, -8.9773540e-01, -3.7342377e-01, -8.6632643e-02,
    -1.0450598e-02, -5.1920577e-04,
];
/// Inverse, 0..20.644 mV (0..500 °C), d0..d9. NIST: -0.05..0.04 °C.
const INV_MID: [f64; 10] = [
    0.0, 2.508355e+01, 7.860106e-02, -2.503131e-01, 8.315270e-02, -1.228034e-02, 9.804036e-04, -4.413030e-05,
    1.057734e-06, -1.052755e-08,
];
/// Inverse, 20.644..54.886 mV (500..1372 °C), d0..d6. NIST: -0.05..0.06 °C.
const INV_HIGH: [f64; 7] = [-1.318058e+02, 4.830222e+01, -1.646031e+00, 5.464731e-02, -9.650715e-04, 8.802193e-06, -3.110810e-08];

fn horner(c: &[f64], x: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, &k| acc * x + k)
}

/// Thermocouple EMF (mV) for a junction at `t` °C, cold junction at 0 °C. Clamped to the
/// reference function's range (-270..1372 °C).
pub fn emf_mv(t: f64) -> f64 {
    let t = t.clamp(-270.0, 1372.0);
    if t < 0.0 {
        horner(&NEG, t)
    } else {
        let d = t - A[2];
        horner(&POS, t) + A[0] * libm::exp(A[1] * d * d)
    }
}

/// Junction temperature (°C) for an EMF (mV) referred to 0 °C. Clamped to the inverse's range
/// (-5.891..54.886 mV, -200..1372 °C).
pub fn temp_c(mv: f64) -> f64 {
    let e = mv.clamp(-5.891, 54.886);
    if e < 0.0 {
        horner(&INV_NEG, e)
    } else if e < 20.644 {
        horner(&INV_MID, e)
    } else {
        horner(&INV_HIGH, e)
    }
}

/// Hot-junction temperature (°C) from the measured EMF (mV, hot against cold junction) and the
/// cold-junction temperature: the cold junction's own EMF is added back before inverting.
pub fn hot_junction_c(measured_mv: f64, cold_c: f64) -> f64 {
    temp_c(measured_mv + emf_mv(cold_c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nist_table_points() {
        // NIST ITS-90 type K table (mV): reference function values.
        for (t, mv) in [(-20.0, -0.778), (0.0, 0.0), (25.0, 1.000), (100.0, 4.096), (150.0, 6.138), (200.0, 8.138), (300.0, 12.209), (500.0, 20.644), (1000.0, 41.276)] {
            assert!((emf_mv(t) - mv).abs() < 0.0015, "E({t}) = {} vs {mv}", emf_mv(t));
        }
    }

    #[test]
    fn inverse_round_trips() {
        let mut t = -150.0;
        while t <= 1300.0 {
            let back = temp_c(emf_mv(t));
            assert!((back - t).abs() < 0.07, "{t} -> {back}");
            t += 2.5;
        }
    }

    #[test]
    fn cold_junction_compensation() {
        // Boiler at 93 C, board at 41 C: the meter sees E(93) - E(41).
        let m = emf_mv(93.0) - emf_mv(41.0);
        assert!((hot_junction_c(m, 41.0) - 93.0).abs() < 0.05);
        // ~41 uV/C near brew temperature: 2 uV per ADC LSB is ~0.05 C.
        let s = (emf_mv(94.0) - emf_mv(92.0)) / 2.0;
        assert!((s - 0.0411).abs() < 0.0015, "{s}");
    }
}
