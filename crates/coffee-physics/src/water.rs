//! Water properties.

/// Specific heat of liquid water, J/(g K). Varies < 1% over 20..100 C.
pub const CP: f64 = 4.186;
/// Latent heat of vaporisation near 100-140 C, J/g.
pub const H_FG: f64 = 2200.0;
/// Atmospheric pressure, bar.
pub const P_ATM: f64 = 1.01325;

/// Saturation pressure (bar absolute) from the Antoine equation
/// (two coefficient sets: 1..100 C and 99..374 C).
pub fn p_sat_abs(t_c: f64) -> f64 {
    let (a, b, c) = if t_c < 100.0 {
        (8.07131, 1730.63, 233.426)
    } else {
        (8.14019, 1810.94, 244.485)
    };
    let mmhg = 10f64.powf(a - b / (c + t_c));
    mmhg * 0.001_333_22
}

/// Saturation pressure as gauge pressure (bar), 0 below boiling.
pub fn p_sat_gauge(t_c: f64) -> f64 {
    (p_sat_abs(t_c) - P_ATM).max(0.0)
}

/// Volumetric thermal expansion coefficient, 1/K (linearised 20..100 C).
pub fn beta(t_c: f64) -> f64 {
    (2.1e-4 + (t_c - 20.0) * 6.9e-6).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steam_table_spot_checks() {
        // Steam tables: 100 C -> 1.0142 bar, 120 C -> 1.9867 bar, 140 C -> 3.6154 bar.
        assert!((p_sat_abs(100.0) - 1.0142).abs() < 0.01);
        assert!((p_sat_abs(120.0) - 1.9867).abs() < 0.02);
        assert!((p_sat_abs(140.0) - 3.6154).abs() < 0.04);
        assert_eq!(p_sat_gauge(90.0), 0.0);
    }
}
