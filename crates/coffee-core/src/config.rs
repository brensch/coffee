use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// All tunables of the engine. Every field has a sensible default for a Gaggia
/// Classic (Pro) with an Ulka E5 pump, so a partial TOML/JSON file is fine.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineConfig {
    pub brew_temp_c: f32,
    pub steam_temp_c: f32,
    pub hot_water_temp_c: f32,
    /// Nominal heater power at nominal mains voltage. Used for feed-forward.
    pub heater_power_w: f32,
    /// Temperature of water entering the boiler (reservoir), for brew feed-forward.
    pub inlet_temp_c: f32,
    /// Water (estimate) must stay within [setpoint - ready_band_c,
    /// setpoint + ready_over_c] for `ready_hold_s` to report Ready.
    pub ready_band_c: f32,
    /// Allowed overshoot for Ready: right after a shot the water sits a degree
    /// or two hot, which is fine to brew on.
    pub ready_over_c: f32,
    /// Also wait for the estimated group/portafilter heat-soak.
    pub ready_requires_heat_soak: bool,
    /// Warm-up boost: run the boiler this much above setpoint while the group
    /// is cold, to push heat into the portafilter faster...
    pub warmup_boost_c: f32,
    /// ...until the heat-soak estimate reaches this (0..1), then return to the
    /// setpoint so the water is back on target when the group is ready.
    pub warmup_boost_until: f32,
    pub ready_hold_s: f32,

    pub temp_pid: PidGains,
    /// Pressure loop: output is a *flow command* in ml/s, linearised through the pump
    /// model into a stroke duty.
    pub pressure_pid: PidGains,
    pub thermocouple: ThermocoupleConfig,
    pub pressure_sensor: PressureSensorConfig,
    pub pump: PumpConfig,
    pub safety: SafetyConfig,
    /// Scale + drip latency compensation: stop when weight + rate * lag >= target.
    pub weight_lag_s: f32,
    /// Duty used for the hot-water function.
    pub hot_water_pump_duty: f32,
    /// Boiler body <-> water conductance (W/K) used to predict how much hotter the
    /// body (where the probe is) must run to keep the *water* at setpoint while
    /// cold water flows in. 0 disables the boost. Calibrate with `coffee-emu fit`.
    pub brew_boost_h_w_k: f32,
    pub max_boost_c: f32,
    /// Extra wall<->water conductance per ml/s of throughflow (forced convection).
    pub brew_boost_h_flow_gain: f32,
    /// Proportional gain multiplier for the temperature loop while water flows
    /// (the idle gains are tuned for gentle holding).
    pub flow_kp_scale: f32,
    /// Fade the boost out over the predicted last N seconds of a shot, so heat
    /// stored in the boiler body doesn't overshoot the water after the shot.
    pub boost_taper_s: f32,
    /// Heat capacity of the boiler water (J/K): 136 ml x 4.186.
    pub water_heat_capacity_j_k: f32,
    /// Extra body-target degrees per degree of estimated water error while
    /// flowing (outer loop of the cascade).
    pub water_loop_gain: f32,
    /// Pressure loop setpoint-rate feed-forward (ml per bar): flow needed to
    /// follow a ramp through system + puck compliance.
    pub pressure_ramp_ff_ml_per_bar: f32,
    /// Portafilter thermal model (same heat paths as the emulator; copy fitted
    /// values here after calibration): heat capacity, conductance to the boiler
    /// through the lugs, loss to the room, and exchange with water flowing through.
    pub group_heat_capacity_j_k: f32,
    pub group_h_boiler_w_k: f32,
    pub group_loss_w_k: f32,
    pub group_flow_ua_w_k: f32,
    /// "Warm" = within this of the portafilter's steady-state temperature.
    pub group_ready_band_c: f32,
    /// After a shot, keep updating the yield while drips land (s).
    pub yield_settle_s: f32,
    /// Pressure/flow based exit conditions are ignored for this long after a phase
    /// starts (the valve vents trapped boiler pressure when it opens).
    pub exit_holdoff_s: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PidGains {
    pub kp: f32,
    pub ki: f32,
    pub kd: f32,
    /// Derivative filter time constant (s).
    pub d_tau_s: f32,
    pub out_min: f32,
    pub out_max: f32,
    /// Only integrate while |error| is below this (prevents wind-up during
    /// large transients such as a cold start).
    pub i_zone: f32,
    /// Clamp on the integral term's contribution.
    pub i_max: f32,
}

impl Default for PidGains {
    fn default() -> Self {
        Self {
            kp: 0.0,
            ki: 0.0,
            kd: 0.0,
            d_tau_s: 0.5,
            out_min: 0.0,
            out_max: 1.0,
            i_zone: f32::MAX,
            i_max: f32::MAX,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ThermocoupleConfig {
    /// Added to the reading (calibration against a reference thermometer).
    pub offset_c: f32,
    /// Measurement filter time constant (s).
    pub filter_tau_s: f32,
    /// A read error must persist this long before a sensor fault is raised.
    pub fault_debounce_s: f32,
}

impl Default for ThermocoupleConfig {
    fn default() -> Self {
        Self {
            offset_c: 0.0,
            filter_tau_s: 0.3,
            fault_debounce_s: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PressureSensorConfig {
    /// Transducer full scale (bar). 1.6 MPa part => 16 bar.
    pub range_bar: f32,
    /// Output voltage at 0 bar and at full scale, at the transducer.
    pub v_zero: f32,
    pub v_full: f32,
    /// Divider ratio between the transducer and the ADC input.
    pub divider: f32,
    /// Below this ADC-side voltage the sensor is considered disconnected.
    pub v_fault_low: f32,
    pub filter_tau_s: f32,
}

impl Default for PressureSensorConfig {
    fn default() -> Self {
        Self {
            range_bar: 16.0,
            v_zero: 0.5,
            v_full: 4.5,
            divider: 2.0 / 3.0,
            v_fault_low: 0.15,
            filter_tau_s: 0.03,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PumpConfig {
    /// (bar, cc/min) points of the pump curve at rated voltage/frequency.
    /// Default: Ulka E5 datasheet ("Family Pump E", average flow rate table).
    pub curve: Vec<(f32, f32)>,
    /// Multiplier applied to the curve (calibrate from a real flow measurement).
    pub flow_scale: f32,
}

impl Default for PumpConfig {
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
            flow_scale: 1.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SafetyConfig {
    pub max_temp_c: f32,
    pub max_pressure_bar: f32,
    pub over_pressure_s: f32,
    /// A single reading jumping by more than this from the filtered value is
    /// treated as a glitch and dropped.
    pub max_temp_step_c: f32,
    /// Readings rejected continuously for longer than this => TEMP_IMPLAUSIBLE.
    pub max_rejected_s: f32,
    /// Heater above `heater_ineffective_duty` (averaged) for this long with less
    /// than `heater_ineffective_rise_c` of temperature rise => HEATER_INEFFECTIVE.
    pub heater_ineffective_s: f32,
    pub heater_ineffective_duty: f32,
    pub heater_ineffective_rise_c: f32,
    /// Temperature this far above setpoint, with the heater off and still rising
    /// => TEMP_RUNAWAY (stuck SSR).
    pub runaway_margin_c: f32,
    /// Pump at high duty, pressure below `no_water_bar` for this long => NO_WATER.
    pub no_water_s: f32,
    pub no_water_bar: f32,
    /// Acceptable mains half period window (us). 50 Hz = 10000, 60 Hz = 8333.
    pub min_half_period_us: u32,
    pub max_half_period_us: u32,
    /// Hard ceiling on a brew, regardless of profile.
    pub max_brew_s: f32,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_temp_c: 165.0,
            max_pressure_bar: 13.0,
            over_pressure_s: 0.3,
            max_temp_step_c: 12.0,
            max_rejected_s: 1.5,
            heater_ineffective_s: 60.0,
            heater_ineffective_duty: 0.8,
            heater_ineffective_rise_c: 2.0,
            runaway_margin_c: 15.0,
            no_water_s: 6.0,
            no_water_bar: 0.15,
            min_half_period_us: 7500,
            max_half_period_us: 11500,
            max_brew_s: 120.0,
        }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            brew_temp_c: 93.0,
            steam_temp_c: 140.0,
            hot_water_temp_c: 95.0,
            heater_power_w: 1370.0,
            inlet_temp_c: 22.0,
            ready_band_c: 1.5,
            ready_over_c: 2.5,
            ready_requires_heat_soak: false,
            warmup_boost_c: 0.0,
            warmup_boost_until: 0.7,
            ready_hold_s: 5.0,
            temp_pid: PidGains {
                kp: 0.06,
                ki: 0.0015,
                kd: 0.6,
                d_tau_s: 2.0,
                out_min: 0.0,
                out_max: 1.0,
                i_zone: 3.0,
                i_max: 0.3,
            },
            pressure_pid: PidGains {
                kp: 2.5,
                ki: 2.5,
                kd: 0.0,
                d_tau_s: 0.05,
                out_min: 0.0,
                out_max: 15.0,
                i_zone: f32::MAX,
                i_max: 15.0,
            },
            thermocouple: ThermocoupleConfig::default(),
            pressure_sensor: PressureSensorConfig::default(),
            pump: PumpConfig::default(),
            safety: SafetyConfig::default(),
            weight_lag_s: 0.8,
            hot_water_pump_duty: 0.6,
            brew_boost_h_w_k: 45.0,
            max_boost_c: 16.0,
            brew_boost_h_flow_gain: 4.0,
            flow_kp_scale: 4.0,
            boost_taper_s: 8.0,
            water_heat_capacity_j_k: 570.0,
            water_loop_gain: 3.0,
            pressure_ramp_ff_ml_per_bar: 1.5,
            group_heat_capacity_j_k: 190.0,
            group_h_boiler_w_k: 1.2,
            group_loss_w_k: 0.25,
            group_flow_ua_w_k: 3.0,
            group_ready_band_c: 3.0,
            yield_settle_s: 4.0,
            exit_holdoff_s: 0.5,
        }
    }
}
