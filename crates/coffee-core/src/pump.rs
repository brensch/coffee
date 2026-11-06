//! Model of the vibratory pump used for flow estimation and for linearising the
//! pressure loop (flow command -> stroke duty).

use crate::config::PumpConfig;

/// Interpolate the pump curve: flow in ml/s at `pressure_bar`, at 100% duty.
pub fn max_flow_ml_s(cfg: &PumpConfig, pressure_bar: f32) -> f32 {
    let c = &cfg.curve;
    if c.is_empty() {
        return 0.0;
    }
    let p = pressure_bar.max(0.0);
    let cc_min = if p <= c[0].0 {
        c[0].1
    } else if p >= c[c.len() - 1].0 {
        c[c.len() - 1].1
    } else {
        let mut v = 0.0;
        for w in c.windows(2) {
            let (p0, q0) = w[0];
            let (p1, q1) = w[1];
            if p >= p0 && p <= p1 {
                let t = if p1 > p0 { (p - p0) / (p1 - p0) } else { 0.0 };
                v = q0 + t * (q1 - q0);
                break;
            }
        }
        v
    };
    (cc_min / 60.0 * cfg.flow_scale).max(0.0)
}

/// Flow the over-pressure valve bleeds back to the tank at `pressure_bar` (ml/s).
pub fn opv_bleed_ml_s(cfg: &PumpConfig, pressure_bar: f32) -> f32 {
    if cfg.opv_k_ml_s_per_bar <= 0.0 {
        return 0.0;
    }
    let w = cfg.opv_softness_bar.max(1e-3);
    let x = (pressure_bar - cfg.opv_crack_bar) / w;
    // softplus knee, as the emulator's OPV
    let sp = if x > 30.0 { x } else { libm::logf(1.0 + libm::expf(x)) };
    cfg.opv_k_ml_s_per_bar * w * sp
}

/// Stroke duty (0..1) needed to deliver `flow_ml_s` to the group against
/// `pressure_bar`: the pump also has to cover what the OPV bleeds.
pub fn duty_for_flow(cfg: &PumpConfig, flow_ml_s: f32, pressure_bar: f32) -> f32 {
    if flow_ml_s <= 0.0 {
        return 0.0;
    }
    let max = max_flow_ml_s(cfg, pressure_bar);
    if max < 0.05 {
        1.0
    } else {
        ((flow_ml_s + opv_bleed_ml_s(cfg, pressure_bar)) / max).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn e5_curve() {
        let c = PumpConfig::default();
        assert!((max_flow_ml_s(&c, 0.0) - 650.0 / 60.0).abs() < 1e-4);
        assert!((max_flow_ml_s(&c, 9.0) - 255.0 / 60.0).abs() < 1e-3);
        assert_eq!(max_flow_ml_s(&c, 20.0), 0.0);
        assert_eq!(duty_for_flow(&c, 100.0, 0.0), 1.0);
    }
    #[test]
    fn us_opv_bleeds_near_brew_pressure() {
        let c = PumpConfig::default();
        assert!(opv_bleed_ml_s(&c, 3.0) < 0.01);
        let b9 = opv_bleed_ml_s(&c, 9.0);
        assert!(b9 > 1.0 && b9 < max_flow_ml_s(&c, 9.0), "{b9}");
        // covering the bleed costs duty
        assert!(duty_for_flow(&c, 1.5, 9.0) > 1.5 / max_flow_ml_s(&c, 9.0) + 0.2);
        let off = PumpConfig { opv_k_ml_s_per_bar: 0.0, ..c };
        assert_eq!(opv_bleed_ml_s(&off, 12.0), 0.0);
    }
}
