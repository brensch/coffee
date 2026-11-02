//! The coupled machine model.
//!
//! Coupling, per substep (default 20 per mains half-cycle, ~0.5 ms):
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
use coffee_io::{SensorFrame, ads1115, input, max31855};
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
pub enum TcMode {
    Ok,
    Open,
    ShortGnd,
    ShortVcc,
    /// Probe fell out of its hole: it now measures chassis air.
    Detached,
    /// SPI read-out frozen.
    Stuck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PsMode {
    Ok,
    Disconnected,
    Stuck,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SsrMode {
    Ok,
    StuckOn,
    StuckOff,
}

/// What the actuators are doing this half-cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Actuators {
    pub heater: bool,
    pub pump: bool,
    pub valve: bool,
}

#[derive(Clone, Debug)]
pub struct Faults {
    pub tc: TcMode,
    pub pressure: PsMode,
    pub ssr: SsrMode,
    pub element_open: bool,
    pub pump_airlock: bool,
    /// Probability per thermocouple read of an EMI spike.
    pub tc_spike_prob: f64,
}

impl Default for Faults {
    fn default() -> Self {
        Self {
            tc: TcMode::Ok,
            pressure: PsMode::Ok,
            ssr: SsrMode::Ok,
            element_open: false,
            pump_airlock: false,
            tc_spike_prob: 0.0,
        }
    }
}

pub struct Machine {
    pub cfg: ModelConfig,
    pub faults: Faults,
    rng: ChaCha8Rng,

    pub t: f64,
    pub half_cycle: u32,
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

    // instantaneous (averaged over the last half-cycle) for observation
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
    prev_half_q: (f64, f64),
    act_queue: VecDeque<Actuators>,

    // sensor state
    tc_word: u32,
    tc_next_t: f64,
    ps_filtered: f64,
    ps_stuck_counts: Option<i16>,
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
            half_cycle: 0,
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
            prev_half_q: (0.0, 0.0),
            act_queue: VecDeque::new(),
            tc_word: 0,
            tc_next_t: 0.0,
            ps_filtered: 0.0,
            ps_stuck_counts: None,
            scale_hist: VecDeque::new(),
            scale_next_t: 0.0,
            scale_mg: 0,
            cfg,
        };
        m.tc_word = m.make_tc_word();
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

    pub fn half_period_s(&self) -> f64 {
        0.5 / self.cfg.mains.frequency_hz.max(1.0)
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

    /// Advance one mains half-cycle with the given actuator commands.
    pub fn step_half_cycle(&mut self, commanded: Actuators) {
        // Model the SSR pipeline: commands take effect N half-cycles later.
        self.act_queue.push_back(commanded);
        let delay = self.cfg.sim.actuator_delay_half_cycles as usize;
        let act = if self.act_queue.len() > delay {
            self.act_queue.pop_front().unwrap_or_default()
        } else {
            Actuators::default()
        };
        self.act = act;
        let half = self.half_period_s();
        let n = self.cfg.sim.substeps.max(1);
        let dt = half / n as f64;
        let positive = self.half_cycle.is_multiple_of(2);

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

        // Heater power for this half-cycle (zero-cross SSR: whole half-cycles).
        let ssr_on = match self.faults.ssr {
            SsrMode::Ok => act.heater,
            SsrMode::StuckOn => true,
            SsrMode::StuckOff => false,
        };
        let h = &self.cfg.heater;
        let r_el = h.rated_voltage_v * h.rated_voltage_v / h.power_w.max(1.0);
        let heater_live =
            ssr_on && !self.faults.element_open && !self.thermostat_open && !self.fuse_blown;
        let p_heat = if heater_live {
            self.cfg.mains.voltage_v * self.cfg.mains.voltage_v / r_el
        } else {
            0.0
        };

        let pump_live =
            act.pump && positive && !self.faults.pump_airlock && self.reservoir_ml > 1.0;

        let (mut s_pump, mut s_opv, mut s_group, mut s_bev, mut s_wand, mut s_steam) =
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let mut brew_t_acc = 0.0;
        let mut brew_q_acc = 0.0;
        let mut r_last = f64::NAN;

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
            r_last = r;
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
            let inflow = net.max(0.0);

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
                brew_t_acc += t_out * q_group;
                brew_q_acc += q_group;
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
            let (target, tau_p) = match self.faults.tc {
                TcMode::Detached => (th.chassis_air_c, 20.0),
                _ => (self.t_body, self.cfg.thermocouple.tau_s),
            };
            self.t_probe += (target - self.t_probe) * (dt / tau_p.max(1e-3)).min(1.0);

            // Pressure transducer lag.
            let tau_s = self.cfg.pressure_sensor.tau_s.max(1e-5);
            self.ps_filtered += (self.p - self.ps_filtered) * (dt / tau_s).min(1.0);

            s_pump += q_pump;
            s_opv += q_opv;
            s_group += q_group;
            s_bev += q_bev;
            s_wand += q_wand;
            s_steam += steam;
        }

        let nf = n as f64;
        // Pump and OPV pulse once per mains cycle: report full-cycle averages.
        let (pp, po) = self.prev_half_q;
        self.prev_half_q = (s_pump / nf, s_opv / nf);
        self.q_pump = 0.5 * (pp + s_pump / nf);
        self.q_opv = 0.5 * (po + s_opv / nf);
        self.q_group = s_group / nf;
        self.q_bev = s_bev / nf;
        self.q_wand = s_wand / nf;
        self.steam_g_s = s_steam / nf;
        self.heater_w = p_heat;
        self.puck_resistance = r_last;
        self.brew_water_c = if brew_q_acc > 0.0 {
            brew_t_acc / brew_q_acc
        } else {
            f64::NAN
        };

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
        self.half_cycle = self.half_cycle.wrapping_add(1);
    }

    // ---------------------------------------------------------------- sensors

    fn make_tc_word(&mut self) -> u32 {
        let internal = self.cfg.thermal.chassis_air_c;
        let mut reading = self.t_probe + self.cfg.thermocouple.offset_c;
        let sigma = self.cfg.thermocouple.noise_c;
        reading += self.gauss(sigma);
        if self.faults.tc_spike_prob > 0.0 && self.rng.r#gen::<f64>() < self.faults.tc_spike_prob {
            reading += if self.rng.r#gen::<bool>() {
                40.0
            } else {
                -40.0
            };
        }
        match self.faults.tc {
            TcMode::Open => max31855::encode(0.0, internal as f32, Some(max31855::TcFault::Open)),
            TcMode::ShortGnd => {
                max31855::encode(0.0, internal as f32, Some(max31855::TcFault::ShortGnd))
            }
            TcMode::ShortVcc => {
                max31855::encode(0.0, internal as f32, Some(max31855::TcFault::ShortVcc))
            }
            TcMode::Stuck => self.tc_word,
            TcMode::Ok | TcMode::Detached => {
                max31855::encode(reading as f32, internal as f32, None)
            }
        }
    }

    fn make_pressure_counts(&mut self) -> i16 {
        let c = self.cfg.pressure_sensor.clone();
        let counts = match self.faults.pressure {
            PsMode::Disconnected => {
                let n = self.gauss(0.002);
                ads1115::volts_to_counts(n.abs() as f32)
            }
            PsMode::Stuck => {
                if let Some(s) = self.ps_stuck_counts {
                    return s;
                }
                ads1115::volts_to_counts(0.0)
            }
            PsMode::Ok => {
                let bar = self.ps_filtered + c.offset_bar + self.gauss(c.noise_bar);
                let v = (c.v_zero + bar / c.range_bar * (c.v_full - c.v_zero)).clamp(0.0, 5.0);
                ads1115::volts_to_counts((v * c.divider) as f32)
            }
        };
        if self.faults.pressure != PsMode::Stuck {
            self.ps_stuck_counts = Some(counts);
        }
        counts
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

    /// Sample every sensor at the zero crossing that starts the next half-cycle.
    pub fn sensor_frame(&mut self) -> SensorFrame {
        if self.t >= self.tc_next_t {
            self.tc_next_t = self.t + self.cfg.thermocouple.period_s;
            self.tc_word = self.make_tc_word();
        }
        let pressure_adc = self.make_pressure_counts();
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
            half_cycle: self.half_cycle,
            t_us: (self.t * 1e6).round() as u64,
            half_period_us: (self.half_period_s() * 1e6).round() as u32,
            max31855: self.tc_word,
            pressure_adc,
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
        ("tc.fault", "ok|open|short_gnd|short_vcc|detached|stuck"),
        ("tc.spikes", "prob/read, 0.02"),
        ("pressure_sensor.fault", "ok|disconnected|stuck"),
        ("ssr.fault", "ok|stuck_on|stuck_off"),
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
            "tc.spikes" => self.faults.tc_spike_prob = v.as_num()?.clamp(0.0, 1.0),
            "tc.fault" => {
                self.faults.tc = match v.as_word().as_str() {
                    "ok" | "none" => TcMode::Ok,
                    "open" => TcMode::Open,
                    "short_gnd" => TcMode::ShortGnd,
                    "short_vcc" => TcMode::ShortVcc,
                    "detached" => TcMode::Detached,
                    "stuck" => TcMode::Stuck,
                    w => bail!("unknown tc.fault `{w}`"),
                };
                if self.faults.tc == TcMode::Ok {
                    // Re-seated probe: it is back in contact with the body.
                }
            }
            "pressure_sensor.fault" => {
                self.faults.pressure = match v.as_word().as_str() {
                    "ok" | "none" => PsMode::Ok,
                    "disconnected" => PsMode::Disconnected,
                    "stuck" => PsMode::Stuck,
                    w => bail!("unknown pressure_sensor.fault `{w}`"),
                }
            }
            "ssr.fault" => {
                self.faults.ssr = match v.as_word().as_str() {
                    "ok" | "none" => SsrMode::Ok,
                    "stuck_on" => SsrMode::StuckOn,
                    "stuck_off" => SsrMode::StuckOff,
                    w => bail!("unknown ssr.fault `{w}`"),
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
