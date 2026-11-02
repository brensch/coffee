//! The emulated world: physics plus the engine's last reported status.

use anyhow::Result;
use coffee_io::EngineStatus;
use coffee_physics::{Machine, Value as PValue};
use coffee_scenario::{Value, World};

pub struct Sim {
    pub m: Machine,
    pub status: EngineStatus,
    pub engine_connected: bool,
}

pub const ENGINE_SIGNALS: &[(&str, &str)] = &[
    ("engine.state", "enum"),
    ("engine.faults", "bitmask"),
    ("engine.phase", "index"),
    ("engine.temp", "C"),
    ("engine.setpoint", "C"),
    ("engine.pressure", "bar"),
    ("engine.target_pressure", "bar"),
    ("engine.flow", "ml/s"),
    ("engine.target_flow", "ml/s"),
    ("engine.weight", "g"),
    ("engine.shot_time", "s"),
    ("engine.heater_duty", "0..1"),
    ("engine.pump_duty", "0..1"),
    ("engine.water", "C"),
    ("engine.heat_soak", "0..1"),
    ("engine.group", "C"),
    ("engine.connected", "bool"),
];

impl Sim {
    pub fn new(m: Machine) -> Self {
        Self {
            m,
            status: EngineStatus::default(),
            engine_connected: false,
        }
    }

    pub fn all_signals() -> Vec<(&'static str, &'static str)> {
        Machine::SIGNALS
            .iter()
            .chain(ENGINE_SIGNALS.iter())
            .copied()
            .collect()
    }

    fn engine_signal(&self, name: &str) -> Option<f64> {
        let s = &self.status;
        Some(match name {
            "engine.state" => s.state as u8 as f64,
            "engine.faults" => s.faults as f64,
            "engine.phase" => s.phase as f64,
            "engine.temp" => s.temp_c as f64,
            "engine.setpoint" => s.setpoint_c as f64,
            "engine.pressure" => s.pressure_bar as f64,
            "engine.target_pressure" => s.target_pressure_bar as f64,
            "engine.flow" => s.flow_ml_s as f64,
            "engine.target_flow" => s.target_flow_ml_s as f64,
            "engine.weight" => s.weight_g as f64,
            "engine.shot_time" => s.shot_time_s as f64,
            "engine.heater_duty" => s.heater_duty as f64,
            "engine.pump_duty" => s.pump_duty as f64,
            "engine.water" => s.water_c as f64,
            "engine.heat_soak" => s.heat_soak as f64,
            "engine.group" => s.group_c as f64,
            "engine.connected" => self.engine_connected as u8 as f64,
            _ => return None,
        })
    }
}

impl World for Sim {
    fn get(&self, name: &str) -> Option<f64> {
        self.m
            .signal(name)
            .or_else(|| self.engine_signal(name))
            .or_else(|| self.m.get_param(name))
    }
    fn set(&mut self, path: &str, v: &Value) -> Result<()> {
        let pv = match v {
            Value::Num(n) => PValue::Num(*n),
            Value::Word(w) => PValue::Word(w.clone()),
        };
        self.m.set(path, &pv)
    }
}
