//! The coupled machine model.
//!
//! The machine is stepped once per mains cycle, as two half-cycles (positive, then
//! negative) of substeps (default 20 per half, ~0.4 ms at 60 Hz). Loads switched by
//! zero-crossing opto-triacs conduct for whole cycles; the pump's diode lets it
//! stroke only during the positive half.
//!
//! Coupling, per substep:
//!
//! ```text
//!  mains ──V²/R──► heater ──► boiler body ◄──► water ◄── cold inflow (enthalpy)
//!    │                           │   ▲            │  └──► steam out (latent heat)
//!    │                           ▼   │            ▼
//!    │                       probe  portafilter ◄─ brew water (eff-NTU)
//!    │
//!    └─stroke on +half──► pump(P) ──► [P node: compliance | steam headspace] ──► OPV ──► reservoir
//!                                          │ valve open
//!                                          ▼
//!                       group headspace ─► wetting front ─► Darcy puck ─► drip ─► cup ─► scale
//! ```
//!
//! The boiler pressure is floored at the saturation pressure of the water, so
//! steam mode, flashing after steaming, and thermal expansion of trapped water
//! all fall out of the same equations.

use crate::config::ModelConfig;
use crate::water::{self, CP, H_FG};
use anyhow::{Result, bail};
use coffee_io::{SensorFrame, input, typek};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::VecDeque;
use std::f64::consts::PI;

/// A value assigned to a parameter: a number, or a word for enum-like faults.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Word(String),
}

impl Value {
    pub fn as_num(&self) -> Result<f64> {
        match self {
            Value::Num(n) => Ok(*n),
            Value::Word(w) => match w.as_str() {
                "on" | "true" | "yes" | "open" => Ok(1.0),
                "off" | "false" | "no" | "closed" => Ok(0.0),
                _ => bail!("expected a number, got `{w}`"),
            },
        }
    }
    pub fn as_word(&self) -> String {
        match self {
            Value::Num(n) => format!("{n}"),
            Value::Word(w) => w.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeMode {
    Ok,
    /// Probe or wire open: the board's 10 MOhm bias drives the ADC input to ~90 mV.
    Open,
    /// Probe's two wires swapped in the plug: the EMF changes sign.
    Reversed,
    /// Probe leads shorted together at the plug: no EMF, so it reads the board's temperature.
    Short,
    /// Probe fell out of its hole: it now measures chassis air.
    Detached,
    /// Reading frozen at the probe temperature when the fault was set (a probe caught in an
    /// air pocket or a cracked junction): the cold junction still tracks the board.
    Stuck,
    /// MCP3421 not answering: no conversions arrive.
    AdcDead,
    /// TMP117 not answering: no cold-junction readings arrive.
    CjDead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsMode {
    Ok,
    Disconnected,
    Stuck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriacMode {
    Ok,
    StuckOn,
    StuckOff,
}

/// What the actuators are doing this mains cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actuators {
    pub heater: bool,
    pub pump: bool,
    pub valve: bool,
}

#[derive(Clone, Debug)]
pub struct Faults {
    pub probe: ProbeMode,
    pub pressure: PsMode,
    pub heater_triac: TriacMode,
    /// The on-board pump triac: shorted (strokes every cycle whatever the engine says) or open.
    pub pump_triac: TriacMode,
    pub element_open: bool,
    pub pump_airlock: bool,
    /// Probability per conversion of an EMI spike on the thermocouple reading, and its size
    /// (uV, either sign; ~41 uV per C at brew temperature).
    pub probe_spike_prob: f64,
    pub probe_spike_uv: f64,
}

impl Default for Faults {
    fn default() -> Self {
        Self {
            probe: ProbeMode::Ok,
            pressure: PsMode::Ok,
            heater_triac: TriacMode::Ok,
            pump_triac: TriacMode::Ok,
            element_open: false,
            pump_airlock: false,
            probe_spike_prob: 0.0,
            probe_spike_uv: 800.0,
        }
    }
}

/// Pump outlet to boiler inlet: the cold water a bleeding OPV can draw back (ml, est).
const LINE_ML: f64 = 2.0;

pub struct Machine {
    pub cfg: ModelConfig,
    pub faults: Faults,
    rng: ChaCha8Rng,

    pub t: f64,
    pub cycle: u32,
    pub act: Actuators,
    pub switch_brew: bool,
    pub switch_steam: bool,
    pub switch_water: bool,
    pub portafilter_attached: bool,
    pub puck_present: bool,
    pub puck_auto_fresh: bool,
    pub wand_open: bool,

    // thermal state
    pub t_body: f64,
    pub t_water: f64,
    pub t_pf: f64,
    pub t_probe: f64,
    pub water_ml: f64,
    pub thermostat_open: bool,
    pub fuse_blown: bool,

    // hydraulic state
    pub p: f64,
    pub reservoir_ml: f64,
    /// Cold water in the pump-to-boiler line that a stroke pushed in and the OPV
    /// can still push back out between strokes (ml, at most `LINE_ML`).
    pub line_cold_ml: f64,
    pub head_ml: f64,
    pub absorbed_ml: f64,
    pub through_ml: f64,
    pub wet_time_s: f64,
    pub channel: f64,
    valve_closed_since: f64,
    prev_valve: bool,

    // cup
    drip_store_g: f64,
    pub cup_g: f64,
    pub tray_ml: f64,

    // averaged over the last mains cycle, for observation
    pub q_pump: f64,
    pub q_opv: f64,
    pub q_group: f64,
    pub q_bev: f64,
    pub q_wand: f64,
    pub steam_g_s: f64,
    pub heater_w: f64,
    pub brew_water_c: f64,
    pub puck_resistance: f64,
    pub energy_in_j: f64,
    act_queue: VecDeque<Actuators>,

    // sensor state
    /// Thermocouple EMF of the last conversion (uV).
    tc_uv: i32,
    /// The probe temperature a `Stuck` probe is frozen at.
    stuck_c: Option<f64>,
    /// Board (cold-junction) temperature, deg C.
    pub t_board: f64,
    ps_filtered: f64,
    /// The ADC oversamples across the whole cycle: running sums for the averages.
    ps_sum: (f64, u32),
    /// Lowest and highest transducer reading over the cycle (bar), for the peak-to-peak.
    ps_range: (f64, f64),
    pressure_pp_mv: u16,
    probe_sum: (f64, u32),
    pressure_mv: u16,
    scale_hist: VecDeque<(f64, f64)>,
    scale_next_t: f64,
    scale_mg: i32,
}

impl Machine {
    pub fn new(cfg: ModelConfig, seed: u64) -> Self {
        let t0 = cfg.sim.initial_temp_c;
        let mut m = Self {
            faults: Faults::default(),
            rng: ChaCha8Rng::seed_from_u64(seed),
            t: 0.0,
            cycle: 0,
            act: Actuators::default(),
            switch_brew: false,
            switch_steam: false,
            switch_water: false,
            portafilter_attached: true,
            puck_present: true,
            puck_auto_fresh: true,
            wand_open: false,
            t_body: t0,
            t_water: t0,
            t_pf: t0,
            t_probe: t0,
            water_ml: cfg.thermal.boiler_capacity_ml,
            thermostat_open: false,
            fuse_blown: false,
            p: 0.0,
            reservoir_ml: cfg.reservoir.initial_ml,
            line_cold_ml: 0.0,
            head_ml: 0.0,
            absorbed_ml: 0.0,
            through_ml: 0.0,
            wet_time_s: 0.0,
            channel: 1.0,
            valve_closed_since: 0.0,
            prev_valve: false,
            drip_store_g: 0.0,
            cup_g: 0.0,
            tray_ml: 0.0,
            q_pump: 0.0,
            q_opv: 0.0,
            q_group: 0.0,
            q_bev: 0.0,
            q_wand: 0.0,
            steam_g_s: 0.0,
            heater_w: 0.0,
            brew_water_c: f64::NAN,
            puck_resistance: f64::NAN,
            energy_in_j: 0.0,
            act_queue: VecDeque::new(),
            tc_uv: 0,
            stuck_c: None,
            t_board: t0,
            ps_filtered: 0.0,
            ps_sum: (0.0, 0),
            ps_range: (f64::INFINITY, f64::NEG_INFINITY),
            pressure_pp_mv: 0,
            probe_sum: (0.0, 0),
            pressure_mv: 0,
            scale_hist: VecDeque::new(),
            scale_next_t: 0.0,
            scale_mg: 0,
            cfg,
        };
        m.pressure_mv = m.make_pressure_mv();
        m
    }

    fn gauss(&mut self, sigma: f64) -> f64 {
        if sigma <= 0.0 {
            return 0.0;
        }
        // Box-Muller
        let u1: f64 = self.rng.r#gen::<f64>().max(1e-12);
        let u2: f64 = self.rng.r#gen::<f64>();
        sigma * (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }

    /// One mains cycle, the engine's tick.
    pub fn period_s(&self) -> f64 {
        1.0 / self.cfg.mains.frequency_hz.max(1.0)
    }

    /// Reset the puck and empty the cup (a fresh shot).
    pub fn fresh_puck(&mut self) {
        self.head_ml = 0.0;
        self.absorbed_ml = 0.0;
        self.through_ml = 0.0;
        self.wet_time_s = 0.0;
        self.channel = 1.0;
        self.drip_store_g = 0.0;
        self.cup_g = 0.0;
        self.scale_hist.clear();
    }

    // ------------------------------------------------------------------ pump

    /// Average flow (ml/s) the pump would deliver running continuously against `p_bar`.
    pub fn pump_curve_ml_s(&self, p_bar: f64) -> f64 {
        let pc = &self.cfg.pump;
        let a = (self.cfg.mains.voltage_v / pc.rated_voltage_v).max(0.0);
        if a <= 0.0 || pc.curve.is_empty() {
            return 0.0;
        }
        // Stroke force ~ V, spring-limited delivery pressure ~ V^2.
        let p_eq = p_bar.max(0.0) / (a * a);
        let c = &pc.curve;
        let cc_min = if p_eq <= c[0].0 {
            c[0].1
        } else if p_eq >= c[c.len() - 1].0 {
            c[c.len() - 1].1
        } else {
            let i = c
                .windows(2)
                .position(|w| p_eq >= w[0].0 && p_eq <= w[1].0)
                .unwrap_or(0);
            let (p0, q0) = c[i];
            let (p1, q1) = c[i + 1];
            q0 + (p_eq - p0) / (p1 - p0) * (q1 - q0)
        };
        let f_ratio = self.cfg.mains.frequency_hz / pc.rated_hz;
        (cc_min / 60.0 * a * f_ratio * pc.wear).max(0.0)
    }

    fn opv_flow(&self, p: f64) -> f64 {
        let h = &self.cfg.hydraulics;
        let w = h.opv_softness_bar.max(1e-3);
        let x = (p - h.opv_crack_bar) / w;
        // softplus knee
        let sp = if x > 30.0 { x } else { (1.0 + x.exp()).ln() };
        h.opv_k_ml_s_per_bar * w * sp
    }

    fn puck_r(&self, p_puck: f64) -> f64 {
        let k = &self.cfg.puck;
        let swell = 1.0 + k.swell_amp * (1.0 - (-self.wet_time_s / k.swell_tau_s.max(1e-3)).exp());
        let erosion = 1.0 / (1.0 + k.erosion_per_ml * self.through_ml);
        k.r_per_g
            * k.dose_g
            * k.grind
            * (1.0 + k.compression_per_bar * p_puck.max(0.0))
            * swell
            * erosion
            * self.channel
    }

    /// Flow into the group at boiler pressure `p`; also returns the part that
    /// leaves the basket as beverage and the effective puck resistance.
    fn group_flow(&self, p: f64) -> (f64, f64, f64) {
        let rs = self.cfg.hydraulics.valve_screen_r;
        if p <= 0.0 {
            return (0.0, 0.0, f64::NAN);
        }
        if !self.portafilter_attached || !self.puck_present {
            let q = p / rs;
            return (q, q, 0.0);
        }
        let k = &self.cfg.puck;
        let absorb_cap = k.absorb_g_per_g * k.dose_g;
        let wet_frac = if self.head_ml < k.headspace_ml {
            0.0
        } else if absorb_cap > 0.0 {
            (self.absorbed_ml / absorb_cap).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let saturated = self.head_ml >= k.headspace_ml && self.absorbed_ml >= absorb_cap;
        // Fixed-point on the pressure drop across the puck (compaction depends on it).
        let mut r = self.puck_r(p) * wet_frac.max(0.02);
        for _ in 0..3 {
            let q = p / (rs + r);
            let p_puck = (p - q * rs).max(0.0);
            r = self.puck_r(p_puck) * wet_frac.max(0.02);
        }
        if self.head_ml < k.headspace_ml {
            // Air escapes through the dry puck: only the screen resists.
            let q = p / rs;
            return (q, 0.0, r);
        }
        let q = p / (rs + r);
        (q, if saturated { q } else { 0.0 }, r)
    }

    // --------------------------------------------------------------- stepping

    /// Advance one mains cycle with the given actuator commands (the engine's
    /// decision from the previous tick).
    pub fn step_cycle(&mut self, commanded: Actuators) {
        self.act_queue.push_back(commanded);
        let delay = self.cfg.sim.actuator_delay_cycles as usize;
        let act = if self.act_queue.len() > delay {
            self.act_queue.pop_front().unwrap_or_default()
        } else {
            Actuators::default()
        };
        self.act = act;

        if !act.valve && self.prev_valve {
            self.valve_closed_since = self.t;
            // 3-way valve vents the group headspace to the drip tray.
            self.tray_ml += self.head_ml;
            self.head_ml = 0.0;
        }
        self.prev_valve = act.valve;
        // The emulated barista knocks out the puck and puts an empty cup on the
        // scale 10 s after a shot, well before the next one (so the engine tares
        // against the right cup).
        if !act.valve
            && self.puck_auto_fresh
            && self.t - self.valve_closed_since > 10.0
            && (self.absorbed_ml > 0.0 || self.cup_g > 0.0)
        {
            self.fresh_puck();
        }

        let mut sums = [0.0; 6];
        let mut brew = (0.0, 0.0);
        let mut r_last = f64::NAN;
        let mut p_heat = 0.0;
        for positive in [true, false] {
            p_heat = self.step_half(act, positive, &mut sums, &mut brew, &mut r_last);
        }
        let nf = 2.0 * self.cfg.sim.substeps.max(1) as f64;
        let [s_pump, s_opv, s_group, s_bev, s_wand, s_steam] = sums;
        self.q_pump = s_pump / nf;
        self.q_opv = s_opv / nf;
        self.q_group = s_group / nf;
        self.q_bev = s_bev / nf;
        self.q_wand = s_wand / nf;
        self.steam_g_s = s_steam / nf;
        self.heater_w = p_heat;
        self.puck_resistance = r_last;
        self.brew_water_c = if brew.1 > 0.0 {
            brew.0 / brew.1
        } else {
            f64::NAN
        };
        self.cycle = self.cycle.wrapping_add(1);
    }

    /// One half-cycle of substeps. Returns the heater power applied.
    fn step_half(
        &mut self,
        act: Actuators,
        positive: bool,
        sums: &mut [f64; 6],
        brew: &mut (f64, f64),
        r_last: &mut f64,
    ) -> f64 {
        let half = 0.5 * self.period_s();
        let n = self.cfg.sim.substeps.max(1);
        let dt = half / n as f64;

        // The opto-triac conducts for whole cycles (fires at the zero crossing).
        let triac_on = match self.faults.heater_triac {
            TriacMode::Ok => act.heater,
            TriacMode::StuckOn => true,
            TriacMode::StuckOff => false,
        };
        let h = &self.cfg.heater;
        let r_el = h.rated_voltage_v * h.rated_voltage_v / h.power_w.max(1.0);
        let heater_live =
            triac_on && !self.faults.element_open && !self.thermostat_open && !self.fuse_blown;
        let p_heat = if heater_live {
            self.cfg.mains.voltage_v * self.cfg.mains.voltage_v / r_el
        } else {
            0.0
        };

        // The pump's series diode: one stroke, on the positive half only.
        let pump_on = match self.faults.pump_triac {
            TriacMode::Ok => act.pump,
            TriacMode::StuckOn => true,
            TriacMode::StuckOff => false,
        };
        let pump_live =
            pump_on && positive && !self.faults.pump_airlock && self.reservoir_ml > 1.0;

        for i in 0..n {
            let th = self.cfg.thermal.clone();
            let hy = self.cfg.hydraulics.clone();
            let cap = th.boiler_capacity_ml;
            let has_head = self.water_ml < cap - 0.5;
            let p_sat = water::p_sat_gauge(self.t_water);
            if has_head {
                self.p = p_sat;
            }
            let p = self.p;

            // Flows.
            let theta = PI * (i as f64 + 0.5) / n as f64;
            let q_pump = if pump_live {
                self.pump_curve_ml_s(p) * PI * theta.sin()
            } else {
                0.0
            };
            let q_opv = self.opv_flow(p);
            let (q_group, q_bev, r) = if act.valve {
                self.group_flow(p)
            } else {
                (0.0, 0.0, f64::NAN)
            };
            *r_last = r;
            let q_wand = if self.wand_open && !has_head {
                p / hy.wand_liquid_r
            } else {
                0.0
            };
            let steam = if self.wand_open && has_head {
                hy.wand_vapor_g_s_per_bar * p_sat
            } else {
                0.0
            };
            // Net flow into the boiler side; negative when the OPV bleeds the
            // line between strokes.
            let net = q_pump - q_opv;
            // Cold water reaching the boiler. Between strokes the OPV bleeds the
            // line back to the tank, and what it bleeds is the cold slug the last
            // stroke pushed in, not boiler water: so backflow cancels recent cold
            // inflow (up to the line's volume) instead of being ignored. Without
            // this a soft OPV that bleeds at brew pressure (US units) would count
            // the same cold water in on every stroke and never out.
            let inflow = if net >= 0.0 {
                self.line_cold_ml = (self.line_cold_ml + net * dt).min(LINE_ML);
                net
            } else {
                let back = (-net * dt).min(self.line_cold_ml);
                self.line_cold_ml -= back;
                -back / dt
            };

            // Thermal expansion of the water (only matters when liquid-full).
            let t_w_before = self.t_water;

            // ---- thermal ----
            let c_b = th.boiler_mass_kg * th.boiler_cp;
            let c_w = (self.water_ml.max(1.0)) * CP;
            let c_pf = th.portafilter_mass_kg * th.portafilter_cp;
            let wet = (self.water_ml / cap).clamp(0.05, 1.0);
            let h_bw =
                (th.h_boiler_water_w_k + th.h_flow_gain_w_k_per_ml_s * (q_group + q_wand)) * wet;
            let q_bw = h_bw * (self.t_body - self.t_water);
            let h_bpf = if self.portafilter_attached {
                th.h_boiler_portafilter_w_k
            } else {
                0.0
            };
            let q_bpf = h_bpf * (self.t_body - self.t_pf);
            let t_in = self.cfg.reservoir.temp_c;

            // Brew water exchanges heat with group + portafilter on the way out.
            let mut q_to_pf = 0.0;
            if q_group > 1e-6 {
                let mdot_cp = q_group * CP;
                let eps = 1.0 - (-th.screen_ua_w_k / mdot_cp).exp();
                let t_out = self.t_water + eps * (self.t_pf - self.t_water);
                q_to_pf = mdot_cp * (self.t_water - t_out);
                brew.0 += t_out * q_group;
                brew.1 += q_group;
            }

            let d_body =
                (p_heat - q_bw - th.loss_boiler_w_k * (self.t_body - th.ambient_c) - q_bpf) / c_b;
            let d_water = (q_bw + inflow * CP * (t_in - self.t_water) - steam * H_FG) / c_w;
            let pf_loss = if self.portafilter_attached {
                th.loss_portafilter_w_k
            } else {
                th.loss_portafilter_w_k * 3.0
            };
            let d_pf = (q_bpf - pf_loss * (self.t_pf - th.ambient_c) + q_to_pf) / c_pf;
            self.t_body += d_body * dt;
            self.t_water += d_water * dt;
            self.t_pf += d_pf * dt;
            self.energy_in_j += p_heat * dt;

            // ---- hydraulics ----
            if has_head {
                // Pressure set by vapour; liquid in/out changes the level.
                self.water_ml += (net - q_group - q_wand) * dt - steam * dt;
                self.water_ml = self.water_ml.clamp(0.0, cap);
            } else {
                let q_exp =
                    water::beta(self.t_water) * self.water_ml * (self.t_water - t_w_before) / dt;
                let c = hy.compliance_ml_per_bar.max(1e-3);
                let seep = hy.seep_ml_s_per_bar * self.p.max(0.0);
                self.p += (net - q_group - q_wand + q_exp - seep) / c * dt;
                // Seepage leaks back through the pump's check valve into the tank.
                self.reservoir_ml += seep * dt;
                let p_floor = water::p_sat_gauge(self.t_water);
                if self.p < p_floor {
                    self.p = p_floor;
                }
                if self.p < 0.0 {
                    self.p = 0.0;
                }
            }
            self.reservoir_ml -= net * dt;
            if self.reservoir_ml < 0.0 {
                self.reservoir_ml = 0.0;
            }

            // ---- group / puck ----
            if act.valve {
                if !self.portafilter_attached {
                    self.tray_ml += q_group * dt;
                } else if !self.puck_present {
                    self.drip_store_g += q_group * dt;
                } else {
                    let k = self.cfg.puck.clone();
                    let mut vin = q_group * dt;
                    let to_head = (k.headspace_ml - self.head_ml).clamp(0.0, vin);
                    self.head_ml += to_head;
                    vin -= to_head;
                    let absorb_cap = k.absorb_g_per_g * k.dose_g;
                    let to_abs = (absorb_cap - self.absorbed_ml).clamp(0.0, vin);
                    self.absorbed_ml += to_abs;
                    vin -= to_abs;
                    if vin > 0.0 {
                        let conc =
                            k.solids_c0 * (-self.through_ml / k.solids_decay_ml.max(1e-3)).exp();
                        self.through_ml += vin;
                        self.drip_store_g += vin * (1.0 + conc);
                    }
                }
            }
            if self.absorbed_ml > 0.0 {
                self.wet_time_s += dt;
            }
            // Drip lag into the cup.
            let tau = self.cfg.cup.drip_tau_s.max(1e-3);
            let out = self.drip_store_g * (dt / tau).min(1.0);
            self.drip_store_g -= out;
            if self.portafilter_attached {
                self.cup_g += out;
            }
            self.tray_ml += q_wand * dt;

            // Probe lag.
            let (target, tau_p) = match self.faults.probe {
                ProbeMode::Detached => (th.chassis_air_c, 20.0),
                _ => (self.t_body, self.cfg.thermocouple.tau_s),
            };
            self.t_probe += (target - self.t_probe) * (dt / tau_p.max(1e-3)).min(1.0);
            // The board warms with the case around the boiler.
            let tc = &self.cfg.thermocouple;
            let board = th.ambient_c + tc.board_rise_frac * (self.t_body - th.ambient_c);
            self.t_board += (board - self.t_board) * (dt / tc.board_tau_s.max(1e-3)).min(1.0);

            // Pressure transducer lag.
            let tau_s = self.cfg.pressure_sensor.tau_s.max(1e-5);
            self.ps_filtered += (self.p - self.ps_filtered) * (dt / tau_s).min(1.0);
            self.ps_sum.0 += self.ps_filtered;
            self.ps_sum.1 += 1;
            self.ps_range = (self.ps_range.0.min(self.ps_filtered), self.ps_range.1.max(self.ps_filtered));
            self.probe_sum.0 += self.t_probe;
            self.probe_sum.1 += 1;

            for (acc, v) in sums
                .iter_mut()
                .zip([q_pump, q_opv, q_group, q_bev, q_wand, steam])
            {
                *acc += v;
            }
        }

        // Protective devices on the boiler body.
        let h = &self.cfg.heater;
        if self.t_body >= h.cutoff_open_c {
            self.thermostat_open = true;
        } else if self.t_body <= h.cutoff_close_c {
            self.thermostat_open = false;
        }
        if self.t_body >= h.thermal_fuse_c {
            self.fuse_blown = true;
        }

        self.t += half;
        p_heat
    }

    // ---------------------------------------------------------------- sensors

    /// The EMF the ADC sees for a probe temperature against the board's, from the NIST curve.
    pub fn tc_emf_uv(&self, probe_c: f64) -> f64 {
        let c = &self.cfg.thermocouple;
        (typek::emf_mv(probe_c + c.offset_c) - typek::emf_mv(self.t_board)) * 1000.0 * c.input_gain
    }

    /// One MCP3421 conversion, if one completes at the end of this cycle (every `conv_cycles`):
    /// the sinc filter averages the probe over the conversion, 2 uV resolution.
    fn make_tc_uv(&mut self) -> Option<i32> {
        let n = self.cfg.thermocouple.conv_cycles.max(1);
        if !self.cycle.is_multiple_of(n) {
            return None;
        }
        let probe = match core::mem::take(&mut self.probe_sum) {
            (s, k) if k > 0 => s / k as f64,
            _ => self.t_probe,
        };
        let noise = self.gauss(self.cfg.thermocouple.noise_uv);
        if self.faults.probe != ProbeMode::Stuck {
            self.stuck_c = None;
        }
        let uv = match self.faults.probe {
            ProbeMode::AdcDead => return None,
            ProbeMode::Stuck => {
                let c = *self.stuck_c.get_or_insert(probe);
                self.tc_emf_uv(c) + noise
            }
            ProbeMode::Open => 90_000.0 + noise,
            ProbeMode::Short => noise,
            ProbeMode::Reversed => -self.tc_emf_uv(probe) + noise,
            ProbeMode::Ok | ProbeMode::Detached | ProbeMode::CjDead => {
                let mut v = self.tc_emf_uv(probe) + noise;
                if self.faults.probe_spike_prob > 0.0 && self.rng.r#gen::<f64>() < self.faults.probe_spike_prob {
                    // a burst of interference corrupting the conversion
                    let a = self.faults.probe_spike_uv;
                    v += if self.rng.r#gen::<bool>() { a } else { -a };
                }
                v
            }
        };
        // 18-bit code at PGA 8: 15.625 uV / 8 per LSB, +-256 mV full scale
        let code = (uv / 1.953125).round().clamp(-131_072.0, 131_071.0);
        self.tc_uv = (code * 1.953125).round() as i32;
        Some(self.tc_uv)
    }

    /// TMP117 reading of the board, with each conversion (centi-C; 7.8125 mC resolution).
    fn make_cj_cc(&mut self, fresh: bool) -> Option<i16> {
        if !fresh || self.faults.probe == ProbeMode::CjDead {
            return None;
        }
        let lsb = (self.t_board / 0.0078125).round() * 0.0078125;
        Some((lsb * 100.0).round() as i16)
    }

    /// Per-cycle oversampled ADC reading of the pressure divider, 1 mV resolution.
    fn make_pressure_mv(&mut self) -> u16 {
        let c = self.cfg.pressure_sensor.clone();
        let mv = match self.faults.pressure {
            PsMode::Stuck => return self.pressure_mv,
            // Unplugged: the 20k leg pulls the pin to ground.
            PsMode::Disconnected => self.gauss(2.0).abs(),
            PsMode::Ok => {
                let avg = match core::mem::take(&mut self.ps_sum) {
                    (s, n) if n > 0 => s / n as f64,
                    _ => self.ps_filtered,
                };
                let bar = avg + c.offset_bar + self.gauss(c.noise_bar);
                let v = (c.v_zero_mv + bar / c.range_bar * (c.v_full_mv - c.v_zero_mv))
                    .clamp(0.0, 5000.0);
                v * c.divider
            }
        };
        mv.round().clamp(0.0, 3300.0) as u16
    }

    /// Peak-to-peak of the cycle's 32 pressure samples at the pin (mV): the transducer's swing
    /// over the cycle plus the ADC's own sample noise.
    fn make_pressure_pp_mv(&mut self) -> u16 {
        let c = self.cfg.pressure_sensor.clone();
        let (lo, hi) = core::mem::replace(&mut self.ps_range, (f64::INFINITY, f64::NEG_INFINITY));
        let swing_bar = match self.faults.pressure {
            PsMode::Ok if hi >= lo => hi - lo,
            PsMode::Stuck => return self.pressure_pp_mv,
            _ => 0.0,
        };
        let mv_per_bar = (c.v_full_mv - c.v_zero_mv) / c.range_bar * c.divider;
        let (mut nlo, mut nhi) = (f64::INFINITY, f64::NEG_INFINITY);
        for _ in 0..32 {
            let n = self.gauss(c.sample_noise_mv);
            nlo = nlo.min(n);
            nhi = nhi.max(n);
        }
        (swing_bar * mv_per_bar + (nhi - nlo)).round().clamp(0.0, 3300.0) as u16
    }

    fn update_scale(&mut self) {
        let sc = self.cfg.scale.clone();
        self.scale_hist.push_back((self.t, self.cup_g));
        while self.scale_hist.len() > 2 && self.scale_hist[1].0 <= self.t - sc.latency_s - 1.0 {
            self.scale_hist.pop_front();
        }
        if self.t >= self.scale_next_t {
            self.scale_next_t = self.t + sc.period_s;
            let want = self.t - sc.latency_s;
            let g = self
                .scale_hist
                .iter()
                .rev()
                .find(|(t, _)| *t <= want)
                .map(|x| x.1)
                .unwrap_or(0.0);
            let noisy = g + self.gauss(sc.noise_g);
            let q = if sc.resolution_g > 0.0 {
                (noisy / sc.resolution_g).round() * sc.resolution_g
            } else {
                noisy
            };
            self.scale_mg = (q * 1000.0).round() as i32;
        }
    }

    /// The sensors as seen over the cycle just simulated.
    pub fn sensor_frame(&mut self) -> SensorFrame {
        let tc_uv = self.make_tc_uv();
        // the firmware reads the TMP117 with each conversion slot, whether or not the ADC answered
        let slot = self.cycle.is_multiple_of(self.cfg.thermocouple.conv_cycles.max(1));
        let cj_cc = self.make_cj_cc(slot);
        self.pressure_mv = self.make_pressure_mv();
        self.pressure_pp_mv = self.make_pressure_pp_mv();
        self.ps_sum = (0.0, 0);
        self.update_scale();
        let mut inputs = 0;
        if self.switch_brew {
            inputs |= input::BREW;
        }
        if self.switch_steam {
            inputs |= input::STEAM;
        }
        if self.switch_water {
            inputs |= input::WATER;
        }
        if self.cfg.scale.connected {
            inputs |= input::SCALE_OK;
        }
        SensorFrame {
            cycle: self.cycle,
            t_us: (self.t * 1e6).round() as u64,
            tc_uv,
            cj_cc,
            pressure_mv: self.pressure_mv,
            pressure_pp_mv: self.pressure_pp_mv,
            scale_mg: self.scale_mg,
            inputs,
        }
    }

    // ------------------------------------------------------- observation / control

    pub const SIGNALS: &'static [(&'static str, &'static str)] = &[
        ("time", "s"),
        ("boiler.temp", "C"),
        ("boiler.water_temp", "C"),
        ("boiler.water_ml", "ml"),
        ("portafilter.temp", "C"),
        ("brew.water_temp", "C"),
        ("probe.temp", "C"),
        ("tc.uv", "uV"),
        ("board.temp", "C"),
        ("pressure_sensor.mv", "mV"),
        ("pressure_sensor.pp_mv", "mV (peak-to-peak over the cycle)"),
        ("hydraulics.pressure", "bar"),
        ("pump.flow", "ml/s"),
        ("opv.flow", "ml/s"),
        ("group.flow", "ml/s"),
        ("puck.flow", "ml/s"),
        ("puck.resistance", "bar/(ml/s)"),
        ("puck.wet", "0..1"),
        ("puck.through_ml", "ml"),
        ("cup.mass", "g"),
        ("tray.ml", "ml"),
        ("reservoir.ml", "ml"),
        ("steam.flow", "g/s"),
        ("heater.power", "W"),
        ("heater.energy", "kJ"),
        ("heater.on", "bool"),
        ("pump.on", "bool"),
        ("valve.open", "bool"),
        ("thermostat.open", "bool"),
        ("fuse.blown", "bool"),
        ("mains.voltage", "V"),
        ("switch.brew", "bool"),
        ("switch.steam", "bool"),
        ("switch.water", "bool"),
        ("scale.reading", "g"),
        ("puck.grind", "x normal"),
        ("wand.open", "bool"),
        ("portafilter.attached", "bool"),
        ("puck.present", "bool"),
    ];

    pub fn signal(&self, name: &str) -> Option<f64> {
        let b = |x: bool| x as u8 as f64;
        let k = &self.cfg.puck;
        Some(match name {
            "time" => self.t,
            "boiler.temp" => self.t_body,
            "boiler.water_temp" => self.t_water,
            "boiler.water_ml" => self.water_ml,
            "portafilter.temp" => self.t_pf,
            "brew.water_temp" => self.brew_water_c,
            "probe.temp" => self.t_probe,
            "tc.uv" => self.tc_uv as f64,
            "board.temp" => self.t_board,
            "pressure_sensor.mv" => self.pressure_mv as f64,
            "pressure_sensor.pp_mv" => self.pressure_pp_mv as f64,
            "hydraulics.pressure" => self.p,
            "pump.flow" => self.q_pump,
            "opv.flow" => self.q_opv,
            "group.flow" => self.q_group,
            "puck.flow" => self.q_bev,
            "puck.resistance" => self.puck_resistance,
            "puck.wet" => {
                let cap = k.headspace_ml + k.absorb_g_per_g * k.dose_g;
                if cap > 0.0 {
                    ((self.head_ml + self.absorbed_ml) / cap).min(1.0)
                } else {
                    1.0
                }
            }
            "puck.through_ml" => self.through_ml,
            "cup.mass" => self.cup_g,
            "tray.ml" => self.tray_ml,
            "reservoir.ml" => self.reservoir_ml,
            "steam.flow" => self.steam_g_s,
            "heater.power" => self.heater_w,
            "heater.energy" => self.energy_in_j / 1000.0,
            "heater.on" => b(self.heater_w > 0.0),
            "pump.on" => b(self.act.pump),
            "valve.open" => b(self.act.valve),
            "thermostat.open" => b(self.thermostat_open),
            "fuse.blown" => b(self.fuse_blown),
            "mains.voltage" => self.cfg.mains.voltage_v,
            "switch.brew" => b(self.switch_brew),
            "switch.steam" => b(self.switch_steam),
            "switch.water" => b(self.switch_water),
            "scale.reading" => self.scale_mg as f64 / 1000.0,
            "wand.open" => b(self.wand_open),
            "portafilter.attached" => b(self.portafilter_attached),
            "puck.present" => b(self.puck_present),
            _ => return None,
        })
    }

    /// Runtime knobs and faults. Anything not listed here is treated as a model
    /// parameter path (see [`ModelConfig::set_path`]).
    pub const KNOBS: &'static [(&'static str, &'static str)] = &[
        ("switch.brew", "on|off"),
        ("switch.steam", "on|off"),
        ("switch.water", "on|off"),
        ("wand.open", "on|off"),
        ("portafilter.attached", "on|off"),
        ("puck.present", "on|off"),
        ("puck.auto_fresh", "on|off"),
        ("puck.fresh", "any (action)"),
        ("puck.channel", "R multiplier, 0.5"),
        ("reservoir.ml", "ml"),
        ("cup.mass", "g"),
        ("probe.fault", "ok|open|reversed|short|detached|stuck|adc_dead|cj_dead"),
        ("probe.spikes", "prob/conversion, 0.03"),
        ("probe.spike_uv", "spike size, uV (default 800, ~20 C)"),
        ("pressure_sensor.fault", "ok|disconnected|stuck"),
        ("heater_triac.fault", "ok|stuck_on|stuck_off"),
        ("pump_triac.fault", "ok|stuck_on|stuck_off"),
        ("heater.element", "ok|open"),
        ("pump.fault", "ok|airlock"),
        ("fuse.reset", "any (action)"),
    ];

    pub fn set(&mut self, path: &str, v: &Value) -> Result<()> {
        let on = || v.as_num().map(|x| x != 0.0);
        match path {
            "switch.brew" => self.switch_brew = on()?,
            "switch.steam" => self.switch_steam = on()?,
            "switch.water" => self.switch_water = on()?,
            "wand.open" => self.wand_open = on()?,
            "portafilter.attached" => self.portafilter_attached = on()?,
            "puck.present" => self.puck_present = on()?,
            "puck.auto_fresh" => self.puck_auto_fresh = on()?,
            "puck.fresh" => self.fresh_puck(),
            "puck.channel" => self.channel = v.as_num()?.max(0.01),
            "reservoir.ml" => {
                self.reservoir_ml = v.as_num()?.clamp(0.0, self.cfg.reservoir.capacity_ml)
            }
            "cup.mass" => {
                self.cup_g = v.as_num()?;
                self.drip_store_g = 0.0;
            }
            "probe.spikes" => self.faults.probe_spike_prob = v.as_num()?.clamp(0.0, 1.0),
            "probe.spike_uv" => self.faults.probe_spike_uv = v.as_num()?.clamp(0.0, 100_000.0),
            "probe.fault" => {
                self.faults.probe = match v.as_word().as_str() {
                    "ok" | "none" => ProbeMode::Ok,
                    "open" => ProbeMode::Open,
                    "reversed" => ProbeMode::Reversed,
                    "short" => ProbeMode::Short,
                    "detached" => ProbeMode::Detached,
                    "stuck" => ProbeMode::Stuck,
                    "adc_dead" => ProbeMode::AdcDead,
                    "cj_dead" => ProbeMode::CjDead,
                    w => bail!("unknown probe.fault `{w}`"),
                };
            }
            "pressure_sensor.fault" => {
                self.faults.pressure = match v.as_word().as_str() {
                    "ok" | "none" => PsMode::Ok,
                    "disconnected" => PsMode::Disconnected,
                    "stuck" => PsMode::Stuck,
                    w => bail!("unknown pressure_sensor.fault `{w}`"),
                }
            }
            "heater_triac.fault" => {
                self.faults.heater_triac = match v.as_word().as_str() {
                    "ok" | "none" => TriacMode::Ok,
                    "stuck_on" => TriacMode::StuckOn,
                    "stuck_off" => TriacMode::StuckOff,
                    w => bail!("unknown heater_triac.fault `{w}`"),
                }
            }
            "pump_triac.fault" => {
                self.faults.pump_triac = match v.as_word().as_str() {
                    "ok" | "none" => TriacMode::Ok,
                    "stuck_on" => TriacMode::StuckOn,
                    "stuck_off" => TriacMode::StuckOff,
                    w => bail!("unknown pump_triac.fault `{w}`"),
                }
            }
            "heater.element" => {
                self.faults.element_open = match v.as_word().as_str() {
                    "ok" | "none" => false,
                    "open" => true,
                    w => bail!("unknown heater.element `{w}`"),
                }
            }
            "pump.fault" => {
                self.faults.pump_airlock = match v.as_word().as_str() {
                    "ok" | "none" => false,
                    "airlock" => true,
                    w => bail!("unknown pump.fault `{w}`"),
                }
            }
            "fuse.reset" => self.fuse_blown = false,
            _ => {
                let x = v.as_num()?;
                self.cfg.set_path(path, x)?;
            }
        }
        Ok(())
    }

    /// Current value of a knob or model parameter, if numeric.
    pub fn get_param(&self, path: &str) -> Option<f64> {
        self.signal(path).or_else(|| self.cfg.get_path(path).ok())
    }
}
