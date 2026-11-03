//! Model parameters. Loaded from a TOML file in `models/`; every numeric leaf is
//! addressable by its dotted path (e.g. `thermal.boiler_mass_kg`) for scenario
//! overrides and for `coffee-emu fit`.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModelConfig {
    pub meta: Meta,
    pub mains: Mains,
    pub heater: Heater,
    pub thermal: Thermal,
    pub hydraulics: Hydraulics,
    pub pump: Pump,
    pub puck: Puck,
    pub cup: Cup,
    pub scale: Scale,
    pub thermistor: Thermistor,
    pub pressure_sensor: PressureSensor,
    pub reservoir: Reservoir,
    pub sim: Sim,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub name: String,
    pub notes: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Mains {
    pub voltage_v: f64,
    pub frequency_hz: f64,
}
impl Default for Mains {
    fn default() -> Self {
        Self {
            voltage_v: 230.0,
            frequency_hz: 50.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Heater {
    /// Element power at `rated_voltage_v`. Resistance is derived: R = V^2 / P.
    pub power_w: f64,
    pub rated_voltage_v: f64,
    /// Bimetal safety thermostat in series with the element (body temperature).
    pub cutoff_open_c: f64,
    pub cutoff_close_c: f64,
    /// One-shot thermal fuse on the boiler body.
    pub thermal_fuse_c: f64,
}
impl Default for Heater {
    fn default() -> Self {
        Self {
            power_w: 1370.0,
            rated_voltage_v: 230.0,
            cutoff_open_c: 170.0,
            cutoff_close_c: 150.0,
            thermal_fuse_c: 184.0,
        }
    }
}

/// Lumped thermal network: boiler body (incl. element) <-> water, body <->
/// portafilter/group, everything -> ambient. Brew water exchanges heat with the
/// portafilter on its way to the puck (effectiveness-NTU).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Thermal {
    pub ambient_c: f64,
    /// Air temperature inside the chassis (what a detached probe settles to).
    pub chassis_air_c: f64,
    pub boiler_mass_kg: f64,
    /// J/(kg K). Brass ~380, aluminium ~900.
    pub boiler_cp: f64,
    /// Water volume of a full boiler.
    pub boiler_capacity_ml: f64,
    /// Body <-> water conductance with the element fully submerged.
    pub h_boiler_water_w_k: f64,
    /// Extra body <-> water conductance per ml/s of throughflow (forced convection).
    pub h_flow_gain_w_k_per_ml_s: f64,
    pub loss_boiler_w_k: f64,
    pub portafilter_mass_kg: f64,
    pub portafilter_cp: f64,
    pub h_boiler_portafilter_w_k: f64,
    pub loss_portafilter_w_k: f64,
    /// UA between brew water and the group/screen/portafilter on the way out.
    pub screen_ua_w_k: f64,
}
impl Default for Thermal {
    fn default() -> Self {
        Self {
            ambient_c: 22.0,
            chassis_air_c: 35.0,
            boiler_mass_kg: 1.3,
            boiler_cp: 380.0,
            boiler_capacity_ml: 136.0,
            h_boiler_water_w_k: 45.0,
            h_flow_gain_w_k_per_ml_s: 4.0,
            loss_boiler_w_k: 0.55,
            portafilter_mass_kg: 0.5,
            portafilter_cp: 380.0,
            h_boiler_portafilter_w_k: 1.2,
            loss_portafilter_w_k: 0.25,
            screen_ua_w_k: 3.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Hydraulics {
    /// Liquid-side compliance (hoses, boiler flex, trapped air), ml per bar.
    pub compliance_ml_per_bar: f64,
    /// Over-pressure valve: soft-knee relief back to the reservoir.
    pub opv_crack_bar: f64,
    pub opv_k_ml_s_per_bar: f64,
    pub opv_softness_bar: f64,
    /// Solenoid + shower screen resistance, bar per ml/s.
    pub valve_screen_r: f64,
    /// Steam wand: liquid resistance (hot water) and vapour coefficient (g/s per bar).
    pub wand_liquid_r: f64,
    pub wand_vapor_g_s_per_bar: f64,
    /// Slow seepage out of the sealed boiler circuit (pump check valve, solenoid
    /// seat), ml/s per bar. Sets how far thermal expansion pressurises the idle
    /// boiler and how fast it bleeds off. Fit from an idle pressure log.
    pub seep_ml_s_per_bar: f64,
}
impl Default for Hydraulics {
    fn default() -> Self {
        Self {
            compliance_ml_per_bar: 0.8,
            opv_crack_bar: 10.0,
            opv_k_ml_s_per_bar: 4.0,
            opv_softness_bar: 0.4,
            valve_screen_r: 0.05,
            wand_liquid_r: 0.6,
            wand_vapor_g_s_per_bar: 0.6,
            seep_ml_s_per_bar: 0.004,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Pump {
    /// (bar, cc/min) average flow at rated voltage and frequency, continuous running.
    /// Default: Ulka E5 datasheet.
    pub curve: Vec<(f64, f64)>,
    pub rated_voltage_v: f64,
    pub rated_hz: f64,
    /// 1.0 = new. Multiplies delivered volume per stroke.
    pub wear: f64,
}
impl Default for Pump {
    fn default() -> Self {
        Self {
            curve: vec![
                (0.0, 650.0),
                (2.0, 520.0),
                (4.0, 420.0),
                (6.0, 350.0),
                (7.0, 330.0),
                (8.0, 300.0),
                (10.0, 210.0),
                (12.0, 140.0),
                (14.0, 60.0),
                (15.0, 0.0),
            ],
            rated_voltage_v: 230.0,
            rated_hz: 50.0,
            wear: 1.0,
        }
    }
}

/// Puck: headspace fill -> wetting front (resistance grows with wetted depth, no
/// drips) -> Darcy flow whose resistance compacts with pressure, swells early and
/// erodes with throughput.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Puck {
    pub dose_g: f64,
    /// Multiplier on resistance: >1 finer, <1 coarser.
    pub grind: f64,
    /// Base resistance per gram of dose, bar per (ml/s).
    pub r_per_g: f64,
    /// Fractional resistance increase per bar across the puck (compaction).
    pub compression_per_bar: f64,
    pub swell_amp: f64,
    pub swell_tau_s: f64,
    /// Resistance divisor growth per ml through the puck.
    pub erosion_per_ml: f64,
    pub headspace_ml: f64,
    /// Water retained by the puck per gram of dose.
    pub absorb_g_per_g: f64,
    /// Dissolved solids: initial concentration (g/ml) and decay volume (ml).
    pub solids_c0: f64,
    pub solids_decay_ml: f64,
}
impl Default for Puck {
    fn default() -> Self {
        Self {
            dose_g: 18.0,
            grind: 1.0,
            r_per_g: 0.095,
            compression_per_bar: 0.15,
            swell_amp: 0.3,
            swell_tau_s: 5.0,
            erosion_per_ml: 0.008,
            headspace_ml: 12.0,
            absorb_g_per_g: 1.1,
            solids_c0: 0.12,
            solids_decay_ml: 25.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Cup {
    /// First-order lag between leaving the basket and landing in the cup.
    pub drip_tau_s: f64,
}
impl Default for Cup {
    fn default() -> Self {
        Self { drip_tau_s: 0.4 }
    }
}

/// Bluetooth scale: sample-and-hold, transport latency, quantisation, noise.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Scale {
    pub connected: bool,
    pub period_s: f64,
    pub latency_s: f64,
    pub resolution_g: f64,
    pub noise_g: f64,
}
impl Default for Scale {
    fn default() -> Self {
        Self {
            connected: true,
            period_s: 0.1,
            latency_s: 0.25,
            resolution_g: 0.1,
            noise_g: 0.03,
        }
    }
}

/// Boiler NTC as wired on the controller: thermistor from the ADC node to GND,
/// `r_ref_ohm` from the 3.3 V rail to the node, read by the C6's ADC1 and
/// oversampled over each mains cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Thermistor {
    /// Probe lag relative to the boiler body (bead in a brass screw probe).
    pub tau_s: f64,
    /// The real part's curve (beta model). The engine's `ntc` config holds what
    /// it *believes*; a mismatch is what the two-point calibration removes.
    pub r25_ohm: f64,
    pub beta: f64,
    pub r_ref_ohm: f64,
    pub v_ref_mv: f64,
    /// Systematic error (probe placement / thermal contact), deg C.
    pub offset_c: f64,
    /// Noise on the per-cycle oversampled average, mV RMS.
    pub noise_mv: f64,
}
impl Default for Thermistor {
    fn default() -> Self {
        Self {
            tau_s: 3.0,
            r25_ohm: 100_000.0,
            beta: 3950.0,
            r_ref_ohm: 8200.0,
            v_ref_mv: 3300.0,
            offset_c: 0.0,
            noise_mv: 1.5,
        }
    }
}

/// 0.5-4.5 V ratiometric transducer on 5 V, through the 10k/20k divider.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PressureSensor {
    pub range_bar: f64,
    pub v_zero_mv: f64,
    pub v_full_mv: f64,
    pub divider: f64,
    /// Noise on the per-cycle oversampled average, in bar.
    pub noise_bar: f64,
    pub tau_s: f64,
    /// Zero drift of the real part (bar). The power-on self-zero removes it.
    pub offset_bar: f64,
}
impl Default for PressureSensor {
    fn default() -> Self {
        Self {
            range_bar: 16.0,
            v_zero_mv: 500.0,
            v_full_mv: 4500.0,
            divider: 2.0 / 3.0,
            noise_bar: 0.02,
            tau_s: 0.003,
            offset_bar: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Reservoir {
    pub capacity_ml: f64,
    pub initial_ml: f64,
    pub temp_c: f64,
}
impl Default for Reservoir {
    fn default() -> Self {
        Self {
            capacity_ml: 2100.0,
            initial_ml: 1800.0,
            temp_c: 22.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Sim {
    /// Physics substeps per mains half-cycle.
    pub substeps: u32,
    pub initial_temp_c: f64,
    /// Extra whole mains cycles between the engine's decision and the loads
    /// switching. The decision for cycle k always applies to cycle k+1 (lockstep
    /// models that already), so real hardware is 0.
    pub actuator_delay_cycles: u32,
}
impl Default for Sim {
    fn default() -> Self {
        Self {
            substeps: 20,
            initial_temp_c: 22.0,
            actuator_delay_cycles: 0,
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            meta: Meta {
                name: "default".into(),
                notes: String::new(),
            },
            mains: Default::default(),
            heater: Default::default(),
            thermal: Default::default(),
            hydraulics: Default::default(),
            pump: Default::default(),
            puck: Default::default(),
            cup: Default::default(),
            scale: Default::default(),
            thermistor: Default::default(),
            pressure_sensor: Default::default(),
            reservoir: Default::default(),
            sim: Default::default(),
        }
    }
}

impl ModelConfig {
    pub fn from_toml_str(s: &str) -> Result<Self> {
        toml::from_str(s).context("parsing model TOML")
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let s =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::from_toml_str(&s).with_context(|| format!("in {}", path.display()))
    }

    pub fn to_toml_string(&self) -> String {
        toml::to_string_pretty(self).expect("model config always serialises")
    }

    /// Read a numeric leaf by dotted path.
    pub fn get_path(&self, path: &str) -> Result<f64> {
        let v = toml::Value::try_from(self)?;
        let mut cur = &v;
        for part in path.split('.') {
            cur = cur
                .get(part)
                .with_context(|| format!("no model parameter `{path}`"))?;
        }
        match cur {
            toml::Value::Float(f) => Ok(*f),
            toml::Value::Integer(i) => Ok(*i as f64),
            toml::Value::Boolean(b) => Ok(*b as u8 as f64),
            _ => bail!("model parameter `{path}` is not numeric"),
        }
    }

    /// Set a numeric (or boolean) leaf by dotted path.
    pub fn set_path(&mut self, path: &str, value: f64) -> Result<()> {
        let mut v = toml::Value::try_from(&*self)?;
        let mut cur = &mut v;
        for part in path.split('.') {
            cur = cur
                .get_mut(part)
                .with_context(|| format!("no model parameter `{path}`"))?;
        }
        *cur = match cur {
            toml::Value::Float(_) => toml::Value::Float(value),
            toml::Value::Integer(_) => toml::Value::Integer(value.round() as i64),
            toml::Value::Boolean(_) => toml::Value::Boolean(value != 0.0),
            _ => bail!("model parameter `{path}` is not numeric"),
        };
        *self = v.try_into().context("re-applying model config")?;
        Ok(())
    }

    /// All numeric leaf paths, for discovery in the UI.
    pub fn numeric_paths(&self) -> Vec<String> {
        fn walk(prefix: &str, v: &toml::Value, out: &mut Vec<String>) {
            match v {
                toml::Value::Table(t) => {
                    for (k, v) in t {
                        let p = if prefix.is_empty() {
                            k.clone()
                        } else {
                            format!("{prefix}.{k}")
                        };
                        walk(&p, v, out);
                    }
                }
                toml::Value::Float(_) | toml::Value::Integer(_) | toml::Value::Boolean(_) => {
                    out.push(prefix.to_string())
                }
                _ => {}
            }
        }
        let mut out = vec![];
        walk("", &toml::Value::try_from(self).unwrap(), &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_get_set() {
        let mut c = ModelConfig::default();
        c.set_path("thermal.boiler_mass_kg", 2.0).unwrap();
        assert_eq!(c.thermal.boiler_mass_kg, 2.0);
        assert_eq!(c.get_path("thermal.boiler_mass_kg").unwrap(), 2.0);
        c.set_path("scale.connected", 0.0).unwrap();
        assert!(!c.scale.connected);
        assert!(c.set_path("thermal.nope", 1.0).is_err());
        assert!(c.numeric_paths().contains(&"puck.grind".to_string()));
        // Round-trips through TOML.
        let s = c.to_toml_string();
        let back = ModelConfig::from_toml_str(&s).unwrap();
        assert_eq!(back.thermal.boiler_mass_kg, 2.0);
    }
}
