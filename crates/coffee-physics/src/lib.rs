//! Physical models for an emulated espresso machine.
//!
//! Start at [`machine::Machine`]. Parameters live in [`config::ModelConfig`] and are
//! loaded from `models/*.toml`. Each mains half-cycle the emulator asks the machine
//! for a [`coffee_io::SensorFrame`], gets actuator commands back from the engine,
//! and calls [`machine::Machine::step_half_cycle`].

pub mod config;
pub mod machine;
pub mod water;

pub use config::ModelConfig;
pub use machine::{Actuators, Machine, Value};

#[cfg(test)]
mod tests {
    use super::*;

    fn run(m: &mut Machine, secs: f64, act: Actuators) {
        let n = (secs / m.half_period_s()).round() as usize;
        for _ in 0..n {
            m.sensor_frame();
            m.step_half_cycle(act);
        }
    }

    #[test]
    fn heat_up_is_plausible() {
        let mut m = Machine::new(ModelConfig::default(), 1);
        let on = Actuators {
            heater: true,
            ..Default::default()
        };
        let mut t93 = None;
        for i in 0..(600 * 100) {
            m.sensor_frame();
            m.step_half_cycle(on);
            if t93.is_none() && m.t_body > 93.0 {
                t93 = Some(i as f64 * 0.01);
            }
        }
        let t93 = t93.expect("never reached 93 C");
        // A ~1.4 kW heater into ~1 kJ/K of brass+water: on the order of a minute or two.
        assert!(
            (40.0..240.0).contains(&t93),
            "body reached 93 C after {t93}s"
        );
        // Full power forever must eventually trip the safety thermostat.
        assert!(m.thermostat_open || m.t_body > 150.0);
    }

    #[test]
    fn blind_basket_hits_opv() {
        let mut cfg = ModelConfig::default();
        cfg.sim.initial_temp_c = 90.0;
        let mut m = Machine::new(cfg, 1);
        m.puck_present = true;
        m.cfg.puck.grind = 1000.0; // effectively blind
        let act = Actuators {
            pump: true,
            valve: true,
            heater: false,
        };
        run(&mut m, 8.0, act);
        let p = m.p;
        assert!((9.5..12.5).contains(&p), "dead-head pressure {p}");
        assert!(m.q_opv > 1.0);
    }

    #[test]
    fn shot_flows_at_nine_bar_ish() {
        let mut cfg = ModelConfig::default();
        cfg.sim.initial_temp_c = 93.0;
        cfg.hydraulics.opv_crack_bar = 9.0;
        let mut m = Machine::new(cfg, 1);
        let act = Actuators {
            pump: true,
            valve: true,
            heater: false,
        };
        run(&mut m, 30.0, act);
        assert!(m.cup_g > 20.0 && m.cup_g < 90.0, "cup {}", m.cup_g);
        assert!(m.p > 6.0, "pressure {}", m.p);
    }

    #[test]
    fn steam_pressure_follows_saturation() {
        let mut cfg = ModelConfig::default();
        cfg.sim.initial_temp_c = 140.0;
        let mut m = Machine::new(cfg, 1);
        m.water_ml = 100.0; // headspace present
        run(&mut m, 0.5, Actuators::default());
        assert!((m.p - 2.6).abs() < 0.3, "steam pressure {}", m.p);
    }
}
