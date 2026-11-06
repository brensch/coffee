use alloc::vec;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// All tunables of the engine. Every field has a sensible default for a Gaggia
/// Classic (Pro) with an Ulka E5 pump, so a partial TOML/JSON file is fine.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineConfig {
    /// Mains frequency (Hz): always 60 (US 120 V / 60 Hz only). The engine ticks once per mains cycle, so this sets
    /// the control period `dt = 1 / mains_hz` and the volume of one pump stroke.
    pub mains_hz: f32,
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
    pub ntc: NtcConfig,
    pub pressure_sensor: PressureSensorConfig,
    pub pump: PumpConfig,
    pub safety: SafetyConfig,
    /// Built-in drip-tray scale (two load cells, dual HX711). Read by the board firmware.
    pub scale: ScaleConfig,
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

/// Boiler NTC: thermistor from the ADC node to GND, precision reference resistor
/// from 3.3 V to the node. The engine sees the node voltage in millivolts.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct NtcConfig {
    /// Reference (top) resistor, ohms. 4.7k 1 %: close to the probe's 3.9k at 93 C, the most mV per degree there.
    pub r_ref_ohm: f32,
    /// Divider supply as seen by the ADC, millivolts.
    pub v_ref_mv: f32,
    /// Beta model: resistance at 25 C and B constant. Default: Molex 218537-3919, 50k +-1 %, B25/95 = 4109
    /// fitted to Molex's R-T table (sales drawing sheet 2): 0 error at 25 and 95 C, +2.0 C at 140 C.
    pub r25_ohm: f32,
    pub beta: f32,
    /// Optional Steinhart-Hart coefficients `[A, B, C]` (1/T = A + B ln R + C ln^3 R,
    /// T in kelvin). When set they replace the beta model. Default: fitted to the Molex 218537-3919 table at
    /// 25, 100 and 140 C; within ~1 C of the table from 25 to 200 C (0.6 C at 80 C).
    pub steinhart_hart: Option<[f32; 3]>,
    /// Two-point calibration applied after the model: `T = gain * T_model + offset`.
    /// See docs/CALIBRATION.md (room temperature + boiling water).
    pub cal_gain: f32,
    pub cal_offset_c: f32,
    /// Node above this => open thermistor / broken wire (the node is pulled to the 3.3 V rail).
    /// An open probe reads the rail; the ESP32-C6 ADC is only specified to +-40 mV near the top of
    /// its 12 dB range (datasheet Table 5-6), so the threshold sits 100 mV below the rail. That also
    /// flags a boiler colder than ~2 C (6 C is 3178 mV for the 50k Molex probe with 4.7k): a fault, heater
    /// off, which is the safe way to be wrong. A machine indoors never starts that cold.
    pub open_mv: f32,
    /// A reading colder than this is treated as a fault too (heater off): a second net under
    /// `open_mv` for an open probe the ADC reads low. A boiler indoors never starts this cold.
    pub min_c: f32,
    /// Node below this => shorted thermistor / wire to GND. 100 mV is ~230 C,
    /// far above anything the boiler can reach.
    pub short_mv: f32,
    /// Measurement filter time constant (s).
    pub filter_tau_s: f32,
    /// Probe time constant against the boiler body (s): control compensates this lag. Molex rates
    /// the 218537-3919 at 15 s in water; refine it from a heat-up log (`coffee-emu fit`).
    pub probe_lag_s: f32,
    /// A fault reading must persist this long before a sensor fault is raised.
    pub fault_debounce_s: f32,
}

impl Default for NtcConfig {
    fn default() -> Self {
        Self {
            r_ref_ohm: 4700.0,
            v_ref_mv: 3300.0,
            r25_ohm: 50_000.0,
            beta: 4109.0,   // Molex 218537-3919 (docs/BUY.md), B25/95 from its R-T table
            steinhart_hart: Some([8.965973e-4, 2.145003e-4, 1.078228e-7]),
            cal_gain: 1.0,
            cal_offset_c: 0.0,
            open_mv: 3200.0,
            min_c: 3.0,
            short_mv: 100.0,
            filter_tau_s: 0.3,
            probe_lag_s: 15.0,
            fault_debounce_s: 0.5,
        }
    }
}

/// 0.5-4.5 V ratiometric transducer on 5 V, read through the 47k/100k divider (147k: the TE M3200 wants > 100k).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PressureSensorConfig {
    /// Transducer full scale (bar). 1.6 MPa part => 16 bar.
    pub range_bar: f32,
    /// Output at 0 bar and at full scale, at the transducer (millivolts).
    pub v_zero_mv: f32,
    pub v_full_mv: f32,
    /// Divider ratio between the transducer and the ADC pin.
    pub divider: f32,
    /// Install calibration: `bar = span_gain * nominal_bar - zero_offset_bar`.
    /// `zero_offset_bar` is the nominal reading with the system open to air.
    pub zero_offset_bar: f32,
    pub span_gain: f32,
    /// Power-on self-zero: the first `auto_zero_s` of readings are averaged and,
    /// if they sit within `auto_zero_band_bar` of the install zero *and* the boiler
    /// is cold (below `auto_zero_max_temp_c`), replace the install zero offset.
    /// 0 disables it. See `Engine::update_pressure` for why both gates are needed.
    pub auto_zero_s: f32,
    pub auto_zero_band_bar: f32,
    pub auto_zero_max_temp_c: f32,
    /// Below this pin voltage the sensor is disconnected (a live transducer never
    /// outputs less than ~0.5 V x 2/3 = 333 mV).
    pub fault_low_mv: f32,
    pub filter_tau_s: f32,
}

impl Default for PressureSensorConfig {
    fn default() -> Self {
        Self {
            range_bar: 16.0, // GaggiMate 0-1.6 MPa sensor (BUY.md 10b)
            v_zero_mv: 500.0,
            v_full_mv: 4500.0,
            divider: 100.0 / 147.0,
            zero_offset_bar: 0.0,
            span_gain: 1.0,
            auto_zero_s: 0.5,
            auto_zero_band_bar: 0.3,
            auto_zero_max_temp_c: 50.0,
            fault_low_mv: 200.0,
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
    /// Over-pressure valve, as the emulator models it: bleed back to the tank
    /// `k * w * softplus((p - crack) / w)` ml/s. US 120 V units ship a 9 bar OPV
    /// that opens gradually from ~7.7 bar, so at brew pressure part of what the
    /// pump pushes never reaches the puck. Set `opv_k_ml_s_per_bar = 0` to ignore it.
    /// Defaults match `models/gaggia_classic_pro_e24_120v.toml`; fit with a blind basket.
    pub opv_crack_bar: f32,
    pub opv_softness_bar: f32,
    pub opv_k_ml_s_per_bar: f32,
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
            opv_crack_bar: 8.6,
            opv_softness_bar: 0.9,
            opv_k_ml_s_per_bar: 4.0,
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
    /// HEATER_INEFFECTIVE energy-balance window (s): see the engine's safety checks.
    pub heater_ineffective_s: f32,
    /// Upper bounds on the boiler's heat capacity (J/K) and heat loss (W) for that check. The
    /// model has ~1.1 kJ/K and ~85-140 W at brew temperature; warm-up times reported for the
    /// Gaggia Classic (boiler plus the group bolted to it) imply up to ~3.5 kJ/K. These bounds sit
    /// above both, so a real boiler always rises faster than the check demands.
    pub heater_check_c_max_j_k: f32,
    pub heater_check_loss_max_w: f32,
    /// Temperature this far above setpoint, with the heater off and still rising
    /// => TEMP_RUNAWAY (shorted heater triac).
    pub runaway_margin_c: f32,
    /// Pump at high duty, pressure below `no_water_bar` for this long => NO_WATER.
    pub no_water_s: f32,
    pub no_water_bar: f32,
    /// Hard ceiling on a brew, regardless of profile.
    pub max_brew_s: f32,
}

/// Drip-tray scale: two load cells, each on its own HX711 channel (gain 128). Weight =
/// sum of (raw - zero) x mg_per_count. A 750 g, 1 mV/V cell reads ~2,860 counts/g at any HX711
/// supply (the input range scales with it), so 0.35 mg/count is the uncalibrated default; the sign
/// depends on how the cell is mounted. Calibrate each cell with a known weight (CALIBRATION.md).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ScaleConfig {
    pub mg_per_count: [f32; 2],
    pub zero: [i32; 2],
}

impl Default for ScaleConfig {
    fn default() -> Self {
        Self { mg_per_count: [0.35, 0.35], zero: [0, 0] }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            max_temp_c: 165.0,
            max_pressure_bar: 13.0,
            over_pressure_s: 0.3,
            max_temp_step_c: 12.0,
            max_rejected_s: 1.5,
            heater_ineffective_s: 120.0,
            heater_check_c_max_j_k: 5000.0,
            heater_check_loss_max_w: 200.0,
            runaway_margin_c: 15.0,
            no_water_s: 6.0,
            no_water_bar: 0.15,
            max_brew_s: 120.0,
        }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            mains_hz: 60.0,
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
                // D acts on the plain reading (Pid::update_split), not on the lag-compensated one:
                // a D term on top of the lead would amplify probe noise into net heat
                kd: 0.6,
                d_tau_s: 2.0,
                out_min: 0.0,
                out_max: 1.0,
                i_zone: 3.0,
                i_max: 0.3,
            },
            pressure_pid: PidGains {
                kp: 2.5,
                // 8 (was 2.5): after a ramp the integral must rebuild the puck's steady flow; at 2.5
                // the first 3 s of a 9 bar hold sat ~0.2 bar low (0.4 worst). 8 halves the ramp lag
                // and the post-ramp dip with the same +-0.03 bar ripple (emulator sweep, 2026-11-05).
                ki: 8.0,
                kd: 0.0,
                d_tau_s: 0.05,
                out_min: 0.0,
                out_max: 15.0,
                i_zone: f32::MAX,
                i_max: 15.0,
            },
            ntc: NtcConfig::default(),
            pressure_sensor: PressureSensorConfig::default(),
            pump: PumpConfig::default(),
            safety: SafetyConfig::default(),
            scale: ScaleConfig::default(),
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

/// Hard limits on every setting that guards the heater or the pressure, so no config (from the
/// API, a file or flash) can switch a safety off. Out-of-range or non-finite values are rejected.
impl EngineConfig {
    pub fn check(&self) -> Result<(), &'static str> {
        fn r(v: f32, lo: f32, hi: f32) -> bool {
            v.is_finite() && v >= lo && v <= hi
        }
        fn gains(g: &PidGains) -> bool {
            [g.kp, g.ki, g.kd, g.d_tau_s, g.i_max].iter().all(|v| r(*v, 0.0, 1e4)) && g.i_zone >= 0.0 // i_zone may be f32::MAX (always on)
        }
        let s = &self.safety;
        let n = &self.ntc;
        let t = &self.temp_pid;
        let p = &self.pressure_sensor;
        // A Steinhart-Hart model replaces the beta model: it must agree with it (the probe's own
        // r25/beta, which are range-checked) within 5 C over the working range.
        let sh_ok = match n.steinhart_hart {
            None => true,
            Some(k) => {
                let beta = NtcConfig { steinhart_hart: None, ..n.clone() };
                k.iter().all(|v| v.is_finite())
                    && [25.0f32, 93.0, 140.0].iter().all(|&c| {
                        let ohms = crate::sensors::ntc_ohms(&beta, crate::sensors::ntc_mv_for_c(&beta, c));
                        r(crate::sensors::ntc_model_c(n, ohms) - crate::sensors::ntc_model_c(&beta, ohms), -5.0, 5.0)
                    })
            }
        };
        // The whole reading chain (model, divider and calibration together) must stay within 5 C
        // of the fitted probe on this board: Molex 218537-3919 under a 4.7k reference. Each field
        // range-checked alone still allowed combinations that read 93 C at a real 174 C, which
        // would push OVER_TEMP past the thermal fuse.
        let probe_ok = {
            let real = NtcConfig { v_ref_mv: n.v_ref_mv, ..NtcConfig::default() };
            [25.0f32, 93.0, 140.0, 165.0].iter().all(|&c| {
                let mv = crate::sensors::ntc_mv_for_c(&real, c);
                let read = n.cal_gain * crate::sensors::ntc_model_c(n, crate::sensors::ntc_ohms(n, mv)) + n.cal_offset_c;
                r(read - c, -5.0, 5.0)
            })
        };
        let checks: &[(bool, &'static str)] = &[
            (probe_ok, "ntc settings read more than 5 C off the board's probe (Molex 218537-3919, 4.7k)"),
            (self.mains_hz == 60.0, "mains_hz must be 60 (US 120 V / 60 Hz only)"),
            (r(self.brew_temp_c, 60.0, 105.0), "brew_temp_c out of 60..105"),
            // the stock steam thermostat opens at about 145 C: a higher setpoint is never reached
            (r(self.steam_temp_c, 110.0, 142.0), "steam_temp_c out of 110..142"),
            (r(self.hot_water_temp_c, 60.0, 110.0), "hot_water_temp_c out of 60..110"),
            (r(self.warmup_boost_c, 0.0, 15.0), "warmup_boost_c out of 0..15"),
            (r(self.max_boost_c, 0.0, 20.0), "max_boost_c out of 0..20"),
            // above the steam thermostat (~145 C), below the thermal fuse (184 C)
            (r(s.max_temp_c, 100.0, 175.0), "safety.max_temp_c out of 100..175"),
            (r(s.max_pressure_bar, 1.0, 15.0), "safety.max_pressure_bar out of 1..15"),
            (r(s.over_pressure_s, 0.0, 5.0), "safety.over_pressure_s out of 0..5"),
            (r(s.heater_ineffective_s, 30.0, 300.0), "safety.heater_ineffective_s out of 30..300"),
            (r(s.heater_check_c_max_j_k, 800.0, 10000.0), "safety.heater_check_c_max_j_k out of 800..10000"),
            (r(s.heater_check_loss_max_w, 50.0, 300.0), "safety.heater_check_loss_max_w out of 50..300"),
            (r(s.runaway_margin_c, 1.0, 20.0), "safety.runaway_margin_c out of 1..20"),
            (r(s.max_temp_step_c, 0.5, 30.0) && r(s.max_rejected_s, 0.1, 10.0), "safety glitch filter out of range"),
            (r(s.max_brew_s, 5.0, 300.0), "safety.max_brew_s out of 5..300"),
            (r(s.no_water_s, 1.0, 30.0) && r(s.no_water_bar, 0.0, 2.0), "safety no-water check out of range"),
            (r(n.open_mv, 3000.0, 3250.0) && r(n.short_mv, 20.0, 300.0), "ntc fault thresholds out of range"),
            (r(n.min_c, -5.0, 15.0), "ntc.min_c out of -5..15"),
            (r(n.fault_debounce_s, 0.05, 2.0), "ntc.fault_debounce_s out of 0.05..2"),
            (r(n.filter_tau_s, 0.0, 3.0), "ntc.filter_tau_s out of 0..3"),
            (r(n.probe_lag_s, 0.0, 60.0), "ntc.probe_lag_s out of 0..60"),
            (r(n.cal_gain, 0.8, 1.2) && r(n.cal_offset_c, -15.0, 15.0), "ntc calibration out of range"),
            (r(n.beta, 3000.0, 5000.0) && r(n.r25_ohm, 10e3, 1e6), "ntc model out of range"),
            (sh_ok, "ntc.steinhart_hart disagrees with the beta model by > 5 C"),
            (r(n.r_ref_ohm, 1e3, 1e5) && r(n.v_ref_mv, 3000.0, 3600.0), "ntc divider out of range"),
            // out_min must stay 0: anything else forces heat whatever the reading says.
            (t.out_min == 0.0 && r(t.out_max, 0.3, 1.0), "temp_pid: out_min must be 0, out_max 0.3..1"),
            (gains(t) && gains(&self.pressure_pid), "pid gains must be finite, 0..1e4"),
            (r(self.pressure_pid.out_min, 0.0, 15.0) && r(self.pressure_pid.out_max, 0.0, 15.0)
                && self.pressure_pid.out_min <= self.pressure_pid.out_max, "pressure_pid output limits out of 0..15 or min > max"),
            // every model field that reaches the heater or pump must be finite and physical: a NaN
            // or runaway estimate would disable the energy-balance and runaway checks
            (r(self.inlet_temp_c, 0.0, 40.0), "inlet_temp_c out of 0..40"),
            (r(self.hot_water_pump_duty, 0.0, 1.0), "hot_water_pump_duty out of 0..1"),
            (r(self.brew_boost_h_w_k, 0.0, 500.0) && r(self.brew_boost_h_flow_gain, 0.0, 50.0), "brew boost conductance out of range"),
            (r(self.flow_kp_scale, 0.0, 10.0), "flow_kp_scale out of 0..10"),
            (r(self.water_heat_capacity_j_k, 100.0, 5000.0) && r(self.water_loop_gain, 0.0, 20.0), "water model out of range"),
            (r(self.boost_taper_s, 0.0, 60.0) && r(self.weight_lag_s, 0.0, 5.0), "boost taper / weight lag out of range"),
            (r(self.group_heat_capacity_j_k, 10.0, 5000.0) && r(self.group_h_boiler_w_k, 0.0, 50.0)
                && r(self.group_loss_w_k, 0.0, 10.0) && r(self.group_flow_ua_w_k, 0.0, 50.0)
                && r(self.group_ready_band_c, 0.0, 20.0), "group model out of range"),
            (r(self.ready_band_c, 0.1, 10.0) && r(self.ready_over_c, 0.1, 10.0) && r(self.ready_hold_s, 0.0, 120.0)
                && r(self.warmup_boost_until, 0.0, 1.0), "ready band out of range"),
            (r(self.pump.flow_scale, 0.2, 5.0) && !self.pump.curve.is_empty()
                && r(self.pump.opv_crack_bar, 5.0, 14.0) && r(self.pump.opv_softness_bar, 0.05, 3.0) && r(self.pump.opv_k_ml_s_per_bar, 0.0, 20.0)
                && self.pump.curve.iter().all(|(b, f)| r(*b, 0.0, 20.0) && r(*f, 0.0, 2000.0)), "pump curve out of range"),
            (r(p.range_bar, 5.0, 30.0) && r(p.v_zero_mv, 300.0, 700.0) && r(p.v_full_mv, 4000.0, 5000.0), "pressure_sensor range out of limits"),
            (r(p.divider, 0.5, 0.8) && r(p.span_gain, 0.8, 1.2) && r(p.zero_offset_bar, -1.0, 1.0), "pressure_sensor calibration out of limits"),
            (r(p.fault_low_mv, 50.0, 450.0) && r(p.filter_tau_s, 0.0, 0.5), "pressure_sensor fault/filter out of limits"),
            (r(p.auto_zero_s, 0.0, 10.0) && r(p.auto_zero_band_bar, 0.0, 1.0) && r(p.auto_zero_max_temp_c, 20.0, 60.0), "pressure_sensor auto-zero out of limits"),
            // the energy-balance check counts heat from this number: it must be the real element's
            (r(self.heater_power_w, 1000.0, 1600.0) && self.heater_power_w > 3.0 * s.heater_check_loss_max_w,
                "heater_power_w out of 1000..1600 (or not well above the loss bound)"),
            (self.scale.mg_per_count.iter().all(|k| k.is_finite() && k.abs() >= 0.01 && k.abs() <= 10.0),
                "scale.mg_per_count out of +-0.01..10"),
            (r(self.pressure_ramp_ff_ml_per_bar, 0.0, 20.0) && r(self.yield_settle_s, 0.0, 30.0)
                && r(self.exit_holdoff_s, 0.0, 10.0), "brew timing / pressure feed-forward out of range"),
        ];
        match checks.iter().find(|(ok, _)| !ok) {
            Some((_, why)) => Err(why),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod check_tests {
    use super::*;

    #[test]
    fn default_config_passes_check() {
        EngineConfig::default().check().unwrap();
    }

    #[test]
    fn check_rejects_unsafe_fields() {
        let mut c = EngineConfig::default();
        c.ntc.fault_debounce_s = 1e6;
        assert!(c.check().is_err());
        let mut c = EngineConfig::default();
        c.ntc.steinhart_hart = Some([1e-3, 2e-4, 1e-7]); // a curve far off the probe's
        assert!(c.check().is_err());
        let mut c = EngineConfig::default();
        c.temp_pid.kp = f32::NAN;
        assert!(c.check().is_err());
        // fields that reach the heater through the water/portafilter models (review round 5)
        let mut c = EngineConfig::default();
        c.brew_boost_h_w_k = 1e5;
        assert!(c.check().is_err());
        let mut c = EngineConfig::default();
        c.inlet_temp_c = -5000.0;
        assert!(c.check().is_err());
        let mut c = EngineConfig::default();
        c.flow_kp_scale = -1.0;
        assert!(c.check().is_err());
        let mut c = EngineConfig::default();
        c.pressure_pid.out_min = 10.0;
        c.pressure_pid.out_max = 5.0;
        assert!(c.check().is_err());
        let mut c = EngineConfig::default();
        c.temp_pid.out_min = 1.0;
        assert!(c.check().is_err());
        // review round 6: each NTC field in range, together reading 93 C at a real ~174 C
        let mut c = EngineConfig::default();
        c.ntc.r25_ohm = 30e3;
        c.ntc.beta = 5000.0;
        c.ntc.r_ref_ohm = 6000.0;
        c.ntc.cal_gain = 0.8;
        c.ntc.steinhart_hart = None;
        assert!(c.check().is_err());
        let mut c = EngineConfig::default();
        c.ntc.r_ref_ohm = 1e5;
        assert!(c.check().is_err());
        // a real two-point calibration (a degree or two) is still accepted
        let mut c = EngineConfig::default();
        c.ntc.cal_gain = 1.02;
        c.ntc.cal_offset_c = -1.5;
        c.check().unwrap();
    }
}
