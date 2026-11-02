//! Open-loop sanity numbers for the default model.
use coffee_physics::*;
fn main() {
    let mut m = Machine::new(ModelConfig::default(), 1);
    let on = Actuators {
        heater: true,
        ..Default::default()
    };
    let mut t = 0.0;
    while m.t_body < 93.0 {
        m.sensor_frame();
        m.step_half_cycle(on);
        t += 0.01;
    }
    println!("heat-up body 22->93C: {t:.0}s, water {:.1}C", m.t_water);
    for _ in 0..1000 {
        m.sensor_frame();
        m.step_half_cycle(Actuators::default());
    }
    println!(
        "after 10s coast: body {:.1} water {:.1}",
        m.t_body, m.t_water
    );

    let mut cfg = ModelConfig::default();
    cfg.sim.initial_temp_c = 93.0;
    let mut m = Machine::new(cfg, 1);
    let act = Actuators {
        pump: true,
        valve: true,
        heater: false,
    };
    for i in 0..3000 {
        m.sensor_frame();
        m.step_half_cycle(act);
        if i % 250 == 0 {
            println!(
                "t={:5.1} P={:5.2} pump={:5.2} opv={:5.2} grp={:5.2} bev={:5.2} R={:5.2} cup={:5.1} wet={:.2} water={:.1} brew={:.1}",
                m.t,
                m.p,
                m.q_pump,
                m.q_opv,
                m.q_group,
                m.q_bev,
                m.puck_resistance,
                m.cup_g,
                m.signal("puck.wet").unwrap(),
                m.t_water,
                m.brew_water_c
            );
        }
    }
    // idle loss at 93
    let mut cfg = ModelConfig::default();
    cfg.sim.initial_temp_c = 93.0;
    let m = Machine::new(cfg, 1);
    println!(
        "idle loss at 93C ~ {:.0} W",
        m.cfg.thermal.loss_boiler_w_k * (93.0 - 22.0)
            + m.cfg.thermal.loss_portafilter_w_k * (93.0 - 22.0)
    );
}
