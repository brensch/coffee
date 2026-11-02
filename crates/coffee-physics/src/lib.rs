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

    fn warm(temp: f64) -> Machine {
        let mut cfg = ModelConfig::default();
        cfg.sim.initial_temp_c = temp;
        Machine::new(cfg, 7)
    }

    const BREW: Actuators = Actuators { pump: true, valve: true, heater: false };

    #[test]
    fn idle_seepage_returns_to_the_reservoir() {
        // No heat flow at all (ambient = water temperature), so the only thing moving
        // water is the pump and then seepage back through the check valve.
        let mut cfg = ModelConfig::default();
        cfg.sim.initial_temp_c = 60.0;
        cfg.thermal.ambient_c = 60.0;
        cfg.reservoir.temp_c = 60.0;
        let mut m = Machine::new(cfg, 1);
        run(&mut m, 0.6, Actuators { pump: true, ..Default::default() });
        run(&mut m, 0.2, Actuators::default());
        let c = m.cfg.hydraulics.compliance_ml_per_bar;
        let before = m.reservoir_ml + c * m.p;
        assert!(m.p > 1.0, "should have pressurised: {}", m.p);
        run(&mut m, 300.0, Actuators::default());
        let after = m.reservoir_ml + c * m.p;
        assert!((after - before).abs() < 0.05, "water not conserved while idle: {before} -> {after}");
    }

    #[test]
    fn shot_water_is_accounted_for() {
        let mut m = warm(93.0);
        let r0 = m.reservoir_ml;
        run(&mut m, 25.0, BREW);
        run(&mut m, 3.0, Actuators::default()); // valve shuts, headspace vents to the tray
        let used = r0 - m.reservoir_ml;
        let c = m.cfg.hydraulics.compliance_ml_per_bar;
        let placed = m.absorbed_ml + m.through_ml + m.tray_ml + m.head_ml + c * m.p;
        // Boiler-water contraction while it cools a little is the only other term.
        assert!((used - placed).abs() < 2.0, "pumped {used:.1} ml, found {placed:.1} ml");
    }

    #[test]
    fn heater_energy_goes_into_the_metal_and_water() {
        let mut m = warm(22.0);
        let (b0, w0, f0) = (m.t_body, m.t_water, m.t_pf);
        run(&mut m, 60.0, Actuators { heater: true, ..Default::default() });
        let th = &m.cfg.thermal;
        let stored = (m.t_body - b0) * th.boiler_mass_kg * th.boiler_cp
            + (m.t_water - w0) * m.water_ml * 4.18
            + (m.t_pf - f0) * th.portafilter_mass_kg * th.portafilter_cp;
        let ratio = stored / m.energy_in_j;
        // A little leaks to ambient; none may appear from nowhere.
        assert!((0.9..=1.0).contains(&ratio), "stored/in = {ratio:.3}");
    }

    #[test]
    fn cools_monotonically_towards_ambient() {
        let mut m = warm(93.0);
        let mut last = m.t_body;
        for _ in 0..60 {
            run(&mut m, 60.0, Actuators::default());
            assert!(m.t_body <= last + 1e-9, "warmed up with the heater off");
            assert!(m.t_body > m.cfg.thermal.ambient_c - 0.01);
            last = m.t_body;
        }
        assert!(m.t_body < 60.0, "an hour off should cool a lot: {}", m.t_body);
    }

    #[test]
    fn finer_grind_means_less_coffee() {
        let cup = |g: f64| {
            let mut m = warm(93.0);
            m.cfg.puck.grind = g;
            run(&mut m, 25.0, BREW);
            m.cup_g
        };
        let (coarse, normal, fine) = (cup(0.6), cup(1.0), cup(1.6));
        assert!(coarse > normal && normal > fine, "{coarse} {normal} {fine}");
    }

    #[test]
    fn open_steam_wand_steals_the_shot() {
        let cup = |wand: bool| {
            let mut m = warm(93.0);
            m.wand_open = wand;
            run(&mut m, 25.0, BREW);
            (m.cup_g, m.tray_ml)
        };
        let (shut, open) = (cup(false), cup(true));
        assert!(open.0 < shut.0 * 0.6, "wand open: {open:?} vs shut {shut:?}");
        assert!(open.1 > shut.1 + 20.0);
    }

    #[test]
    fn low_mains_weakens_the_pump() {
        let dead_head = |v: f64| {
            let mut m = warm(90.0);
            m.cfg.mains.voltage_v = v;
            m.cfg.puck.grind = 1000.0;
            run(&mut m, 8.0, BREW);
            (m.p, m.pump_curve_ml_s(0.0))
        };
        let (p_full, q_full) = dead_head(230.0);
        let (p_sag, q_sag) = dead_head(207.0);
        assert!(q_sag < q_full * 0.95, "free flow {q_sag} vs {q_full}");
        // The OPV still caps it if the pump can reach it; it must not exceed nominal.
        assert!(p_sag <= p_full + 0.05, "{p_sag} vs {p_full}");
    }
}
