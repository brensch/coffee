//! The engine state machine. See the crate docs for the timing contract.

use crate::config::EngineConfig;
use crate::control::{Lpf, Pid, SigmaDelta};
use crate::profile::{Observed, Profile, Target};
use crate::pump;
use crate::sensors::{self, NtcReading, PressureReading};
use coffee_io::{ActuatorFrame, EngineState, EngineStatus, SensorFrame, fault, input};

/// Water specific heat, J/(g K).
const CP_WATER: f32 = 4.186;

/// Faults that stay active until [`Command::ClearFaults`] (or a reboot), even
/// after the condition that caused them goes away.
const LATCHING: u16 = fault::OVER_TEMP
    | fault::TEMP_RUNAWAY
    | fault::HEATER_INEFFECTIVE
    | fault::TEMP_IMPLAUSIBLE
    | fault::NO_WATER;

/// Requests from the UI / API (front-panel switches arrive in the sensor frame).
#[derive(Clone, Debug)]
pub enum Command {
    StartBrew,
    /// Run water through the (empty) portafilter for this long: warms it up,
    /// no weight stop, not a shot.
    Flush(f32),
    StopBrew,
    SetProfile(Profile),
    SetBrewTemp(f32),
    SetSteamTemp(f32),
    SetConfig(EngineConfig),
    ClearFaults,
}

/// Why a brew ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrewEnd {
    ProfileComplete,
    TargetWeight,
    MaxTime,
    UserStop,
    Fault,
}

/// Summary of the last finished shot, used for history and grind advice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotSummary {
    pub duration_s: f32,
    pub weight_g: Option<f32>,
    pub pumped_ml: f32,
    pub end: BrewEnd,
    /// Mean hydraulic resistance (bar per ml/s) while pressure > 3 bar.
    /// Higher = finer grind / tighter puck. None if pressure never got there.
    pub mean_resistance: Option<f32>,
    pub peak_pressure_bar: f32,
}

#[derive(Clone, Debug)]
struct Brew {
    start_t: f64,
    phase: usize,
    phase_start_t: f64,
    phase_start_value: f32,
    phase_start_pumped: f32,
    /// Pumped volume when the volume-stop counting phase began.
    volume_base: Option<f32>,
    last_setpoint: f32,
    pumped_ml: f32,
    tare_g: Option<f32>,
    resistance_sum: f32,
    resistance_n: u32,
    peak_pressure: f32,
    started_by_switch: bool,
    ended: Option<(BrewEnd, f64)>,
}

pub struct Engine {
    pub cfg: EngineConfig,
    pub profile: Profile,
    state: EngineState,
    faults: u16,
    t: f64,
    /// Time of the first frame (power-on), for the pressure self-zero window.
    boot_t: Option<f64>,

    // temperature
    temp: Lpf,
    temp_rate: Lpf,
    temp_prev: Option<f32>,
    ntc_bad_since: Option<f64>,
    rejected_since: Option<f64>,

    // pressure
    pressure_ctl: Lpf,
    pressure_disp: Lpf,
    /// Slower pressure for exit conditions and safety (vibratory pumps ripple).
    pressure_slow: Lpf,
    pressure_bad_since: Option<f64>,
    over_pressure_since: Option<f64>,
    /// Accepted power-on self-zero (nominal bar at atmospheric). None: the
    /// install calibration `pressure_sensor.zero_offset_bar` applies.
    self_zero: Option<f32>,
    /// Self-zero accumulator (sum of nominal bar, samples); None once decided.
    auto_zero: Option<(f32, u32)>,

    // pump / flow
    stroke_frac: Lpf,
    /// The stroke decision now being executed by the hardware (last tick's output).
    pump_on: bool,
    flow_est: f32,

    // weight
    weight: Lpf,
    weight_rate: Lpf,
    weight_prev: Option<f32>,

    // control
    temp_pid: Pid,
    pressure_pid: Pid,
    heater_sd: SigmaDelta,
    pump_sd: SigmaDelta,
    heater_duty: f32,
    heater_duty_avg: Lpf,
    pump_duty: f32,
    flow_cmd: f32,
    target_pressure: f32,
    target_flow: f32,

    // state tracking
    prev_inputs: u8,
    in_band_since: Option<f64>,
    brew: Option<Brew>,
    pending_start: bool,
    pending_stop: bool,
    last_shot: Option<ShotSummary>,
    shot_counter: u32,
    /// The user's profile while a flush runs on a temporary one.
    restore_profile: Option<Profile>,
    /// After a shot: keep updating its yield until this time (drips landing),
    /// with the shot's tare. Runs in the background; the machine is free.
    settle: Option<(f64, Option<f32>)>,
    /// The last shot's tare, kept after the settle window so the display keeps
    /// reading the shot until the cup comes off the scale.
    hold_tare: Option<f32>,

    group_est: Option<f32>,
    water_est: Option<f32>,
    prev_target: Option<f32>,

    // safety windows
    /// (start time, reading, heater energy) of the current energy-balance window.
    heater_window: Option<(f64, f32, f64)>,
    /// Heater energy actually delivered (J, after fault overrides), and the last time water moved.
    heater_energy_j: f64,
    last_flow_t: f64,
    runaway_since: Option<f64>,
    no_water_since: Option<f64>,
}

impl Engine {
    pub fn new(cfg: EngineConfig, profile: Profile) -> Self {
        Self {
            cfg,
            profile,
            state: EngineState::Boot,
            faults: 0,
            t: 0.0,
            boot_t: None,
            temp: Lpf::default(),
            temp_rate: Lpf::default(),
            temp_prev: None,
            ntc_bad_since: None,
            rejected_since: None,
            pressure_ctl: Lpf::default(),
            pressure_disp: Lpf::default(),
            pressure_slow: Lpf::default(),
            pressure_bad_since: None,
            over_pressure_since: None,
            self_zero: None,
            auto_zero: Some((0.0, 0)),
            stroke_frac: Lpf::default(),
            pump_on: false,
            flow_est: 0.0,
            weight: Lpf::default(),
            weight_rate: Lpf::default(),
            weight_prev: None,
            temp_pid: Pid::default(),
            pressure_pid: Pid::default(),
            heater_sd: SigmaDelta::default(),
            pump_sd: SigmaDelta::default(),
            heater_duty: 0.0,
            heater_duty_avg: Lpf::default(),
            pump_duty: 0.0,
            flow_cmd: 0.0,
            target_pressure: f32::NAN,
            target_flow: f32::NAN,
            prev_inputs: 0,
            in_band_since: None,
            brew: None,
            pending_start: false,
            pending_stop: false,
            last_shot: None,
            shot_counter: 0,
            restore_profile: None,
            settle: None,
            hold_tare: None,
            group_est: None,
            water_est: None,
            prev_target: None,
            heater_window: None,
            heater_energy_j: 0.0,
            last_flow_t: 0.0,
            runaway_since: None,
            no_water_since: None,
        }
    }

    pub fn state(&self) -> EngineState {
        self.state
    }
    pub fn faults(&self) -> u16 {
        self.faults
    }
    /// Pressure zero offset in use (nominal bar at atmospheric).
    pub fn pressure_zero(&self) -> f32 {
        self.self_zero
            .unwrap_or(self.cfg.pressure_sensor.zero_offset_bar)
    }
    /// Control period: one mains cycle.
    pub fn dt(&self) -> f32 {
        1.0 / self.cfg.mains_hz.clamp(40.0, 70.0)
    }
    pub fn last_shot(&self) -> Option<(u32, ShotSummary)> {
        self.last_shot.map(|s| (self.shot_counter, s))
    }

    pub fn command(&mut self, cmd: Command) -> Result<(), &'static str> {
        match cmd {
            Command::StartBrew => self.pending_start = true,
            Command::Flush(secs) => {
                if !matches!(self.state, EngineState::Heating | EngineState::Ready) {
                    return Err("can only flush while idle");
                }
                let flush = Profile::flush(secs.clamp(1.0, 120.0));
                self.restore_profile = Some(core::mem::replace(&mut self.profile, flush));
                self.pending_start = true;
            }
            Command::StopBrew => self.pending_stop = true,
            Command::SetProfile(p) => {
                p.validate()?;
                if self.brew.is_some() {
                    return Err("cannot change profile while brewing");
                }
                self.profile = p;
            }
            Command::SetBrewTemp(t) => {
                if !(80.0..=105.0).contains(&t) {
                    return Err("brew temperature out of range 80..105");
                }
                self.cfg.brew_temp_c = t;
                self.profile.temperature_c = None;
            }
            Command::SetSteamTemp(t) => {
                if !(110.0..=142.0).contains(&t) {
                    return Err("steam temperature out of range 110..142 (the steam thermostat opens ~145)");
                }
                self.cfg.steam_temp_c = t;
            }
            Command::SetConfig(c) => {
                if self.brew.is_some() {
                    return Err("cannot change config while brewing");
                }
                c.check()?;
                self.cfg = c;
            }
            Command::ClearFaults => {
                // The heater and runaway windows keep running: clearing must not reset the
                // evidence, or a client clearing faults on a timer would stop them ever latching.
                self.faults &= !LATCHING;
                self.no_water_since = None;
            }
        }
        Ok(())
    }

    fn set_fault(&mut self, bit: u16, active: bool) {
        if active {
            self.faults |= bit;
        } else if bit & LATCHING == 0 {
            self.faults &= !bit;
        }
    }

    fn brew_setpoint(&self) -> f32 {
        // Clamped as well as validated: profiles also arrive from flash and imports.
        let t = self.profile.temperature_c.unwrap_or(self.cfg.brew_temp_c);
        if t.is_finite() { t.clamp(60.0, 105.0) } else { self.cfg.brew_temp_c }
    }

    fn setpoint(&self) -> f32 {
        match self.state {
            EngineState::Steam => self.cfg.steam_temp_c,
            EngineState::HotWater => self.cfg.hot_water_temp_c,
            _ => self.brew_setpoint(),
        }
    }

    /// Setpoint the heater actually controls to (warm-up boost included). The
    /// target shown to the user and used for Ready stays `setpoint()`.
    fn control_setpoint(&self) -> f32 {
        let sp = self.setpoint();
        let warming = matches!(self.state, EngineState::Heating | EngineState::Boot);
        if warming
            && self.cfg.warmup_boost_c > 0.0
            && self.heat_soak() < self.cfg.warmup_boost_until
        {
            sp + self.cfg.warmup_boost_c
        } else {
            sp
        }
    }

    /// The probe sits on the boiler body. While cold water flows in, the water
    /// lags the body by roughly P_needed / h. Run the body that much hotter so the
    /// *water* stays at setpoint.
    fn body_setpoint(&self) -> f32 {
        let sp = self.control_setpoint();
        let flowing = matches!(self.state, EngineState::Brewing | EngineState::HotWater)
            && self.brew.as_ref().is_none_or(|b| b.ended.is_none());
        if !flowing || self.cfg.brew_boost_h_w_k <= 0.0 {
            return sp;
        }
        // Feed-forward: the wall-to-water gap needed to carry the inflow's heat,
        // plus the outer loop correcting the observed water error.
        let watts = self.flow_est * CP_WATER * (sp - self.cfg.inlet_temp_c).max(0.0);
        let water_err = sp - self.water_est.unwrap_or(sp);
        let boost = watts / self.h_eff() + self.cfg.water_loop_gain * water_err;
        sp + boost.clamp(0.0, self.cfg.max_boost_c) * self.boost_taper()
    }

    fn h_eff(&self) -> f32 {
        self.cfg.brew_boost_h_w_k + self.cfg.brew_boost_h_flow_gain * self.flow_est
    }

    /// 1.0 normally; fades to 0 over the predicted last `boost_taper_s` of a
    /// weight-stopped shot.
    fn boost_taper(&self) -> f32 {
        let (Some(target), Some(w)) = (self.profile.stop.weight_g, self.cup_weight()) else {
            return 1.0;
        };
        let rate = self.weight_rate.value.unwrap_or(0.0);
        if rate < 0.3 || self.cfg.boost_taper_s <= 0.0 {
            return 1.0;
        }
        let remaining = (target - w).max(0.0) / rate;
        (remaining / self.cfg.boost_taper_s).clamp(0.0, 1.0)
    }

    /// Advance one mains cycle. `s` describes the cycle that just ended; the
    /// returned frame is applied by the hardware to the next cycle.
    pub fn tick(&mut self, s: &SensorFrame) -> ActuatorFrame {
        let dt = self.dt();
        self.t = s.t_us as f64 * 1e-6;
        self.boot_t.get_or_insert(self.t);

        self.update_temperature(s.ntc_mv, dt);
        let p = self.update_pressure(s.pressure_mv, dt);
        self.update_weight(s, dt);
        self.update_flow(p, dt);

        let inputs = s.inputs;
        let rising = inputs & !self.prev_inputs;
        let falling = !inputs & self.prev_inputs;
        self.prev_inputs = inputs;

        self.safety_checks(p);
        self.state_machine(inputs, rising, falling, p);
        if let Some((until, _)) = self.settle {
            if self.t <= until {
                let w = self.cup_weight();
                if let Some(s) = self.last_shot.as_mut() {
                    s.weight_g = w.or(s.weight_g);
                }
            } else {
                self.hold_tare = self.settle.and_then(|s| s.1);
                self.settle = None;
            }
        }
        self.control(p, dt);

        // Faults override everything: heater, pump and valve off.
        // A non-finite duty (a model estimate gone bad) is no heat and no pumping, never NaN:
        // NaN would poison the duty average and energy totals and disable the checks below.
        if !self.heater_duty.is_finite() {
            self.heater_duty = 0.0;
        }
        if !self.pump_duty.is_finite() {
            self.pump_duty = 0.0;
        }
        self.heater_duty = self.heater_duty.clamp(0.0, 1.0);
        self.pump_duty = self.pump_duty.clamp(0.0, 1.0);
        if self.faults != 0 {
            self.heater_duty = 0.0;
            self.pump_duty = 0.0;
        }
        // What the heater really does (after the fault override): the duty average and the
        // energy balance must not count heat that was requested but never delivered.
        self.heater_duty_avg.update(self.heater_duty, dt, 20.0);
        self.heater_energy_j += (self.heater_duty * self.cfg.heater_power_w * dt) as f64;
        if self.flow_est >= 0.3 || self.pump_duty > 0.0 {
            self.last_flow_t = self.t;
        }
        let valve = self.faults == 0
            && matches!(self.state, EngineState::Brewing)
            && self.brew.as_ref().is_some_and(|b| b.ended.is_none());

        // Burst firing: whole cycles of heat; one stroke per pump cycle.
        let heater = self.heater_sd.next(self.heater_duty);
        self.pump_on = self.pump_sd.next(self.pump_duty);

        ActuatorFrame {
            cycle: s.cycle,
            heater,
            pump: self.pump_on,
            valve,
            status: self.status(p),
        }
    }

    fn update_temperature(&mut self, mv: u16, dt: f32) {
        let now = self.t;
        let t = match sensors::ntc(&self.cfg.ntc, mv) {
            NtcReading::Celsius(t) => t,
            bad => {
                let since = *self.ntc_bad_since.get_or_insert(now);
                if now - since >= self.cfg.ntc.fault_debounce_s as f64 {
                    let bit = if bad == NtcReading::Open {
                        fault::NTC_OPEN
                    } else {
                        fault::NTC_SHORT
                    };
                    self.set_fault(bit, true);
                    self.temp.reset();
                    self.temp_prev = None;
                }
                return;
            }
        };
        self.ntc_bad_since = None;
        self.set_fault(fault::NTC_OPEN, false);
        self.set_fault(fault::NTC_SHORT, false);
        if let Some(f) = self.temp.value {
            if (t - f).abs() > self.cfg.safety.max_temp_step_c {
                let since = *self.rejected_since.get_or_insert(now);
                if now - since > self.cfg.safety.max_rejected_s as f64 {
                    // Persistently far away: it is real (or the sensor is lying
                    // consistently). Flag it and re-seed.
                    self.set_fault(fault::TEMP_IMPLAUSIBLE, true);
                    self.temp.reset();
                    self.rejected_since = None;
                } else {
                    return;
                }
            } else {
                self.rejected_since = None;
            }
        }
        let v = self.temp.update(t, dt, self.cfg.ntc.filter_tau_s);
        if let Some(prev) = self.temp_prev {
            if dt > 0.0 {
                let r = (v - prev) / dt;
                if r.is_finite() {
                    self.temp_rate.update(r, dt, 3.0);
                }
            }
        }
        self.temp_prev = Some(v);
    }

    /// Boiler body temperature for control: the probe reading plus its lag (first-order lead,
    /// `probe_lag_s` x the filtered rate). The probe trails the body by about lag x rate while the
    /// temperature moves. Safety checks use the plain reading.
    fn body_temp(&self) -> f32 {
        // clamped: at most 5 C of lead, so a burst of noise can't swing the control much
        let lead = self.cfg.ntc.probe_lag_s * self.temp_rate.get();
        self.temp.get() + if lead.is_finite() { lead.clamp(-5.0, 5.0) } else { 0.0 }
    }

    fn temp_valid(&self) -> bool {
        self.temp.value.is_some() && self.faults & (fault::NTC_OPEN | fault::NTC_SHORT) == 0
    }

    /// Power-on pressure self-zero. Transducers drift a few tens of mbar with
    /// temperature and age, so re-zeroing at power-on is worthwhile, but the
    /// reading at power-on is only "zero" when the hydraulics are at atmospheric:
    ///
    /// * A hot boiler holds 2-4 bar behind the closed 3-way valve (thermal
    ///   expansion, the OPV seat) and bleeds down only slowly, so a quick power
    ///   cycle can leave anything from 0.1 to 4 bar on the gauge. Requiring a cold
    ///   boiler (NTC below `auto_zero_max_temp_c`) rules that out: cold water
    ///   has not expanded and the valve has had time to vent.
    /// * Even cold, a reading far from the install zero means a stuck valve or a
    ///   faulty sensor, not drift, so it must also sit within
    ///   `auto_zero_band_bar` of the install calibration.
    ///
    /// Only when both hold is the average of the first `auto_zero_s` adopted;
    /// otherwise the install calibration stays. Readings during the window use
    /// the install zero.
    fn auto_zero_step(&mut self, mv: u16) {
        let Some((sum, n)) = self.auto_zero.as_mut() else {
            return;
        };
        let c = &self.cfg.pressure_sensor;
        let elapsed = self.t - self.boot_t.unwrap_or(self.t);
        if c.auto_zero_s <= 0.0 {
            self.auto_zero = None;
            return;
        }
        if elapsed < c.auto_zero_s as f64 {
            if mv as f32 >= c.fault_low_mv {
                *sum += sensors::pressure_nominal_bar(c, mv as f32);
                *n += 1;
            }
            return;
        }
        let (sum, n) = (*sum, *n);
        self.auto_zero = None;
        if n == 0 {
            return;
        }
        let mean = sum / n as f32;
        let cold = self.temp_valid() && self.temp.get() < c.auto_zero_max_temp_c;
        if cold && (mean - c.zero_offset_bar).abs() <= c.auto_zero_band_bar {
            self.self_zero = Some(mean);
        }
    }

    fn update_pressure(&mut self, mv: u16, dt: f32) -> f32 {
        self.auto_zero_step(mv);
        match sensors::pressure(&self.cfg.pressure_sensor, mv, self.pressure_zero()) {
            PressureReading::Bar(b) => {
                self.pressure_bad_since = None;
                self.set_fault(fault::PRESSURE_SENSOR, false);
                let tau = self.cfg.pressure_sensor.filter_tau_s;
                self.pressure_disp.update(b, dt, 0.15);
                self.pressure_slow.update(b, dt, 0.12);
                self.pressure_ctl.update(b, dt, tau)
            }
            PressureReading::Disconnected => {
                let since = *self.pressure_bad_since.get_or_insert(self.t);
                if self.t - since > 0.2 {
                    self.set_fault(fault::PRESSURE_SENSOR, true);
                }
                self.pressure_ctl.value.unwrap_or(0.0)
            }
        }
    }

    fn update_weight(&mut self, s: &SensorFrame, dt: f32) {
        if s.inputs & input::SCALE_OK == 0 {
            self.weight.reset();
            self.weight_rate.reset();
            self.weight_prev = None;
            return;
        }
        let g = s.scale_mg as f32 / 1000.0;
        let w = self.weight.update(g, dt, 0.1);
        // Cup lifted off (reading well below the held tare): back to raw weight.
        if self.hold_tare.is_some_and(|t| g < t - 2.0) {
            self.hold_tare = None;
        }
        if let Some(prev) = self.weight_prev {
            self.weight_rate.update((w - prev) / dt, dt, 0.6);
        }
        self.weight_prev = Some(w);
    }

    fn update_flow(&mut self, p: f32, dt: f32) {
        // One pump cycle is exactly one stroke, delivering curve(p) / mains_hz ml.
        let max_flow = pump::max_flow_ml_s(&self.cfg.pump, p);
        let stroke = if self.pump_on { 1.0 } else { 0.0 };
        let frac = self.stroke_frac.update(stroke, dt, 0.25);
        self.flow_est = frac * max_flow;
        if let Some(b) = self.brew.as_mut()
            && b.ended.is_none()
        {
            b.pumped_ml += stroke * max_flow * dt;
            b.peak_pressure = b.peak_pressure.max(p);
            if p > 3.0 && self.flow_est > 0.2 {
                b.resistance_sum += p / self.flow_est;
                b.resistance_n += 1;
            }
        }
    }

    fn cup_weight(&self) -> Option<f32> {
        let w = self.weight.value?;
        let tare = self
            .brew
            .as_ref()
            .and_then(|b| b.tare_g)
            .or_else(|| self.settle.and_then(|s| s.1))
            .or(self.hold_tare)
            .unwrap_or(0.0);
        Some(w - tare)
    }

    fn safety_checks(&mut self, p: f32) {
        let now = self.t;
        let sc = self.cfg.safety.clone();

        if self.temp_valid() {
            let temp = self.temp.get();
            if temp > sc.max_temp_c {
                self.set_fault(fault::OVER_TEMP, true);
            }

            // Heater ineffective, as an energy balance: over each window, the heat actually
            // delivered minus the most the boiler can lose (`heater_check_loss_max_w`), spread over
            // the most it can hold (`heater_check_c_max_j_k`), is the least the reading must rise.
            // A reading that rises less than half of that (minus 1 C) is wrong: a probe that is
            // open but under the open threshold, backed out of its seat, or stuck at any value
            // (the PID then keeps adding heat the reading never shows), or a heater that isn't
            // heating. At idle the duty is far below the loss allowance, so nothing is expected
            // and nothing is checked. Disarmed while water moves and for 90 s after (fresh cold
            // water absorbs heat unseen by the probe on the body), and in steam mode (steaming
            // holds the boiler flat at full power; the steam thermostat bounds that mode).
            let armed = self.state != EngineState::Steam && now - self.last_flow_t > 90.0;
            if armed {
                let (t0, temp0, e0) = *self.heater_window.get_or_insert((now, temp, self.heater_energy_j));
                let w = now - t0;
                if w >= sc.heater_ineffective_s as f64 {
                    let net = (self.heater_energy_j - e0) - sc.heater_check_loss_max_w as f64 * w;
                    let expected = (net / sc.heater_check_c_max_j_k as f64) as f32;
                    if expected >= 1.5 && temp - temp0 < 0.5 * expected - 0.5 {
                        self.set_fault(fault::HEATER_INEFFECTIVE, true);
                    }
                    self.heater_window = Some((now, temp, self.heater_energy_j));
                }
            } else {
                self.heater_window = None;
            }

            // Runaway: heater off, far above setpoint and still climbing.
            let sp = self.setpoint();
            let runaway = self.heater_duty_avg.get() < 0.05
                && temp > sp + sc.runaway_margin_c
                && self.temp_rate.get() > 0.1;
            if runaway {
                let since = *self.runaway_since.get_or_insert(now);
                if now - since > 10.0 {
                    self.set_fault(fault::TEMP_RUNAWAY, true);
                }
            } else {
                self.runaway_since = None;
            }
        }

        // Over-pressure, with hysteresis on clear.
        if p > sc.max_pressure_bar {
            let since = *self.over_pressure_since.get_or_insert(now);
            if now - since > sc.over_pressure_s as f64 {
                self.set_fault(fault::OVER_PRESSURE, true);
            }
        } else {
            self.over_pressure_since = None;
            if p < sc.max_pressure_bar - 2.0 {
                self.set_fault(fault::OVER_PRESSURE, false);
            }
        }

        // Pump running hard, nothing coming back: empty tank / lost prime.
        // Any meaningful pump demand should produce *some* back-pressure: even a
        // flush with no portafilter builds ~0.3 bar across the shower screen.
        let pumping = matches!(self.state, EngineState::Brewing | EngineState::HotWater)
            && self.pump_duty > 0.3;
        if pumping && p < sc.no_water_bar {
            let since = *self.no_water_since.get_or_insert(now);
            if now - since > sc.no_water_s as f64 {
                self.set_fault(fault::NO_WATER, true);
            }
        } else {
            self.no_water_since = None;
        }
    }

    fn start_brew(&mut self, by_switch: bool, p: f32) {
        self.settle = None;
        self.hold_tare = None;
        self.temp_pid.integral += 0.0;
        self.pressure_pid.reset();
        self.pump_sd.reset();
        let start = match self.profile.phases[0].target {
            Target::Pressure(_) => p,
            Target::Flow(_) => 0.0,
        };
        self.brew = Some(Brew {
            start_t: self.t,
            phase: 0,
            phase_start_t: self.t,
            phase_start_value: start,
            phase_start_pumped: 0.0,
            volume_base: (self.profile.stop.volume_from_phase.unwrap_or(0) == 0).then_some(0.0),
            last_setpoint: start,
            pumped_ml: 0.0,
            tare_g: self.weight.value,
            resistance_sum: 0.0,
            resistance_n: 0,
            peak_pressure: 0.0,
            started_by_switch: by_switch,
            ended: None,
        });
        self.state = EngineState::Brewing;
    }

    fn end_brew(&mut self, why: BrewEnd) {
        let now = self.t;
        let weight = self.cup_weight();
        if let Some(b) = self.brew.as_mut() {
            if b.ended.is_some() {
                return;
            }
            b.ended = Some((why, now));
            self.settle = Some((now + self.cfg.yield_settle_s as f64, b.tare_g));
            self.shot_counter += 1;
            self.last_shot = Some(ShotSummary {
                duration_s: (now - b.start_t) as f32,
                weight_g: weight,
                pumped_ml: b.pumped_ml,
                end: why,
                mean_resistance: (b.resistance_n > 50)
                    .then(|| b.resistance_sum / b.resistance_n as f32),
                peak_pressure_bar: b.peak_pressure,
            });
        }
        self.state = EngineState::BrewDone;
    }

    fn state_machine(&mut self, inputs: u8, rising: u8, falling: u8, p: f32) {
        let start_req = core::mem::take(&mut self.pending_start) || rising & input::BREW != 0;
        let stop_req = core::mem::take(&mut self.pending_stop);

        if self.faults != 0 {
            if self.brew.as_ref().is_some_and(|b| b.ended.is_none()) {
                self.end_brew(BrewEnd::Fault);
            }
            self.brew = None;
            self.state = EngineState::Fault;
            return;
        }

        match self.state {
            EngineState::Boot | EngineState::Fault => {
                if self.temp_valid() {
                    self.state = EngineState::Heating;
                }
            }
            EngineState::Heating | EngineState::Ready => {
                if inputs & input::STEAM != 0 {
                    self.state = EngineState::Steam;
                } else if inputs & input::WATER != 0 {
                    self.state = EngineState::HotWater;
                } else if start_req {
                    self.start_brew(rising & input::BREW != 0, p);
                } else {
                    // Judge readiness on the water (what brews), not the wall probe.
                    let water = self.water_est.unwrap_or(self.temp.get());
                    let err = water - self.setpoint();
                    let c = &self.cfg;
                    let in_band = err > -c.ready_band_c && err < c.ready_over_c;
                    let soaked = !c.ready_requires_heat_soak || self.heat_soak() >= 1.0;
                    if in_band && soaked {
                        let since = *self.in_band_since.get_or_insert(self.t);
                        if self.t - since >= c.ready_hold_s as f64 {
                            self.state = EngineState::Ready;
                        }
                    } else {
                        self.in_band_since = None;
                        if err < -2.0 * c.ready_band_c || err > c.ready_over_c + 2.0 {
                            self.state = EngineState::Heating;
                        }
                    }
                }
            }
            EngineState::Brewing => {
                let switch_off = falling & input::BREW != 0;
                if stop_req || switch_off {
                    self.end_brew(BrewEnd::UserStop);
                } else {
                    self.advance_profile(p);
                }
            }
            EngineState::BrewDone => {
                let by_switch = self.brew.as_ref().is_some_and(|b| b.started_by_switch);
                let ended_at = self
                    .brew
                    .as_ref()
                    .and_then(|b| b.ended)
                    .map(|e| e.1)
                    .unwrap_or(self.t);
                let _ = ended_at;
                // A UI-started shot frees the machine at once (the yield keeps
                // settling in the background); the brew switch holds BrewDone
                // until it's turned off, as its position says.
                let release = !by_switch || inputs & input::BREW == 0;
                if release {
                    self.finish_brew_done();
                }
            }
            EngineState::Steam => {
                if inputs & input::STEAM == 0 {
                    self.state = EngineState::Heating;
                    self.in_band_since = None;
                }
            }
            EngineState::HotWater => {
                if inputs & input::WATER == 0 {
                    self.state = EngineState::Heating;
                    self.in_band_since = None;
                }
            }
        }
    }

    fn finish_brew_done(&mut self) {
        if let Some(p) = self.restore_profile.take() {
            self.profile = p;
        }
        if let Some(b) = self.brew.take()
            && b.ended.is_some()
            && self.settle.is_none()
        {
            self.settle = Some((self.t + self.cfg.yield_settle_s as f64, b.tare_g));
        }
        // Straight back to Ready if the water is still in range: a shot doesn't
        // make the machine "unready" by itself.
        self.state = if self.ready_now() {
            EngineState::Ready
        } else {
            EngineState::Heating
        };
        self.in_band_since = None;
    }

    /// Ready criteria without the hold time.
    fn ready_now(&self) -> bool {
        if !self.temp_valid() {
            return false;
        }
        let c = &self.cfg;
        let err = self.water_est.unwrap_or(self.temp.get()) - self.brew_setpoint();
        let soaked = !c.ready_requires_heat_soak || self.heat_soak() >= 1.0;
        err > -c.ready_band_c && err < c.ready_over_c && soaked
    }

    fn observed(&self, b: &Brew, _p: f32) -> Observed {
        Observed {
            phase_time_s: (self.t - b.phase_start_t) as f32,
            pressure_bar: self.pressure_slow.value.unwrap_or(0.0),
            flow_ml_s: self.flow_est,
            weight_g: self.cup_weight(),
            pumped_ml: b.pumped_ml - b.phase_start_pumped,
        }
    }

    fn advance_profile(&mut self, p: f32) {
        let Some(b) = self.brew.as_ref() else { return };
        if b.ended.is_some() {
            return;
        }
        let shot_t = (self.t - b.start_t) as f32;

        // Global stops.
        let stop = self.profile.stop;
        if let (Some(target), Some(w)) = (stop.weight_g, self.cup_weight()) {
            let rate = self.weight_rate.value.unwrap_or(0.0).max(0.0);
            if w + rate * self.cfg.weight_lag_s >= target {
                self.end_brew(BrewEnd::TargetWeight);
                return;
            }
        }
        if let (Some(v), Some(base)) = (stop.volume_ml, b.volume_base) {
            if b.pumped_ml - base >= v {
                self.end_brew(BrewEnd::ProfileComplete);
                return;
            }
        }
        let max_t = stop
            .max_time_s
            .unwrap_or(f32::MAX)
            .min(self.cfg.safety.max_brew_s);
        if shot_t >= max_t {
            self.end_brew(BrewEnd::MaxTime);
            return;
        }

        // Phase exits. Sensor-based exits wait out the hold-off (trapped pressure
        // from the previous shot vents in the first few hundred ms).
        let obs = self.observed(b, p);
        let phase = &self.profile.phases[b.phase];
        let holdoff = obs.phase_time_s < self.cfg.exit_holdoff_s;
        let exit_now = phase
            .exit
            .iter()
            .any(|e| (!holdoff || matches!(e, crate::profile::Exit::TimeS(_))) && e.met(&obs));
        if exit_now {
            let next = b.phase + 1;
            if next >= self.profile.phases.len() {
                self.end_brew(BrewEnd::ProfileComplete);
                return;
            }
            let prev_target = phase.target;
            let next_target = self.profile.phases[next].target;
            let adaptive = self.profile.phases[next].transition.adaptive;
            let start_value = if prev_target.same_kind(next_target) && !adaptive {
                self.brew.as_ref().unwrap().last_setpoint
            } else {
                match next_target {
                    Target::Pressure(_) => p,
                    Target::Flow(_) => self.flow_est,
                }
            };
            if matches!(
                (prev_target, next_target),
                (Target::Flow(_), Target::Pressure(_))
            ) {
                // Bumpless hand-over: start the pressure loop at the current flow.
                self.pressure_pid.reset();
                self.pressure_pid.preload(self.flow_cmd);
            }
            let now = self.t;
            let b = self.brew.as_mut().unwrap();
            b.phase = next;
            b.phase_start_pumped = b.pumped_ml;
            if self.profile.stop.volume_from_phase == Some(next as u8) {
                b.volume_base = Some(b.pumped_ml);
            }
            b.phase_start_t = now;
            b.phase_start_value = start_value;
        }
    }

    fn control(&mut self, p: f32, dt: f32) {
        // ---- temperature ----
        if self.temp_valid() {
            let sp = self.body_setpoint();
            let flowing = matches!(self.state, EngineState::Brewing | EngineState::HotWater);
            let ff = if flowing {
                (self.flow_est * CP_WATER * (sp - self.cfg.inlet_temp_c).max(0.0))
                    / self.cfg.heater_power_w.max(1.0)
            } else {
                0.0
            };
            let mut gains = self.cfg.temp_pid.clone();
            if flowing {
                gains.kp *= self.cfg.flow_kp_scale;
            }
            self.heater_duty = self.temp_pid.update_split(&gains, sp, self.body_temp(), self.temp.get(), ff, dt);
        } else {
            self.heater_duty = 0.0;
            self.temp_pid.reset();
        }

        // Water observer: C_w dTw/dt = h (T_wall - Tw) - q c_p (Tw - T_in).
        if self.temp_valid() && self.cfg.brew_boost_h_w_k > 0.0 {
            let tb = self.body_temp();
            let tw = self.water_est.get_or_insert(tb);
            let q_in = if matches!(self.state, EngineState::Brewing | EngineState::HotWater) {
                self.flow_est
            } else {
                0.0
            };
            let h = self.cfg.brew_boost_h_w_k + self.cfg.brew_boost_h_flow_gain * q_in;
            let d = (h * (tb - *tw) - q_in * CP_WATER * (*tw - self.cfg.inlet_temp_c))
                / self.cfg.water_heat_capacity_j_k.max(1.0);
            *tw += d * dt;
            if !tw.is_finite() {
                self.water_est = None;   // an unstable estimate restarts from the wall reading
            }
        } else if !self.temp_valid() {
            self.water_est = None;
        }

        // Portafilter model: heated by the boiler through the lugs, by (or, if
        // it's hotter, cooling) the water flowing through it, losing heat to the
        // room. A flush therefore warms it, which a lag filter can't express.
        if self.temp_valid() {
            let c = &self.cfg;
            let wall = self.body_temp();
            let water = self.water_est.unwrap_or(wall);
            let g = *self.group_est.get_or_insert(wall);
            let flowing = matches!(self.state, EngineState::Brewing | EngineState::HotWater)
                && self.valve_open();
            let flow_w_k = if flowing && self.flow_est > 0.05 {
                let mcp = self.flow_est * CP_WATER;
                mcp * (1.0 - libm::expf(-c.group_flow_ua_w_k / mcp))
            } else {
                0.0
            };
            let q = c.group_h_boiler_w_k * (wall - g) + flow_w_k * (water - g)
                - c.group_loss_w_k * (g - c.inlet_temp_c);
            let ng = g + q / c.group_heat_capacity_j_k.max(1.0) * dt;
            self.group_est = if ng.is_finite() { Some(ng) } else { None };
        }

        // ---- pump ----
        self.target_pressure = f32::NAN;
        self.target_flow = f32::NAN;
        self.flow_cmd = 0.0;
        match self.state {
            EngineState::Brewing => {
                let Some(b) = self.brew.as_ref() else { return };
                if b.ended.is_some() {
                    self.pump_duty = 0.0;
                    return;
                }
                let phase = &self.profile.phases[b.phase];
                let t = (self.t - b.phase_start_t) as f32;
                let sp = phase.setpoint(b.phase_start_value, t);
                let gains = self.cfg.pressure_pid.clone();
                let cmd = match phase.target {
                    Target::Pressure(_) => {
                        self.target_pressure = sp;
                        let rate = match self.prev_target {
                            Some(prev) if dt > 0.0 => ((sp - prev) / dt).max(0.0),
                            _ => 0.0,
                        };
                        self.prev_target = Some(sp);
                        let ff = rate * self.cfg.pressure_ramp_ff_ml_per_bar;
                        let pid_out = self.pressure_pid.update(&gains, sp, p, ff, dt);
                        let lim = phase.limit.unwrap_or(f32::MAX);
                        let cmd = pid_out.min(lim);
                        // Back-calculation so the integrator doesn't wind past the limit.
                        self.pressure_pid.integral -= pid_out - cmd;
                        cmd
                    }
                    Target::Flow(_) => {
                        self.prev_target = None;
                        self.target_flow = sp;
                        match phase.limit {
                            Some(pmax) => {
                                let lim = self.pressure_pid.update(&gains, pmax, p, 0.0, dt);
                                let cmd = sp.min(lim);
                                self.pressure_pid.integral -= lim - cmd;
                                cmd
                            }
                            None => sp,
                        }
                    }
                };
                self.brew.as_mut().unwrap().last_setpoint = sp;
                self.flow_cmd = cmd.max(0.0);
                self.pump_duty = pump::duty_for_flow(&self.cfg.pump, self.flow_cmd, p);
            }
            EngineState::HotWater => {
                self.pump_duty = self.cfg.hot_water_pump_duty;
            }
            _ => {
                self.pump_duty = 0.0;
                self.pressure_pid.reset();
                self.prev_target = None;
            }
        }
    }

    /// 0..1 group heat-soak: 1 when the estimate is within the ready band.
    fn heat_soak(&self) -> f32 {
        let (Some(g), true) = (self.group_est, self.temp_valid()) else {
            return 0.0;
        };
        let c = &self.cfg;
        let ta = c.inlet_temp_c;
        // Steady state with the boiler at setpoint and no flow.
        let g_ss = (c.group_h_boiler_w_k * self.brew_setpoint() + c.group_loss_w_k * ta)
            / (c.group_h_boiler_w_k + c.group_loss_w_k);
        let full = g_ss - c.group_ready_band_c;
        ((g - ta) / (full - ta)).clamp(0.0, 1.0)
    }

    fn valve_open(&self) -> bool {
        self.brew.as_ref().is_some_and(|b| b.ended.is_none())
    }

    fn status(&self, _p: f32) -> EngineStatus {
        let (phase, shot_t) = match &self.brew {
            Some(b) => {
                let end = b.ended.map(|e| e.1).unwrap_or(self.t);
                (b.phase as u8, (end - b.start_t) as f32)
            }
            None => (0xFF, 0.0),
        };
        EngineStatus {
            state: self.state,
            faults: self.faults,
            phase,
            temp_c: self.temp.get(),
            setpoint_c: self.setpoint(),
            pressure_bar: self.pressure_disp.value.unwrap_or(f32::NAN),
            target_pressure_bar: self.target_pressure,
            flow_ml_s: self.flow_est,
            target_flow_ml_s: self.target_flow,
            weight_g: self.cup_weight().unwrap_or(f32::NAN),
            shot_time_s: shot_t,
            heater_duty: self.heater_duty,
            pump_duty: self.pump_duty,
            boost_c: if self.temp_valid() {
                self.body_setpoint() - self.setpoint()
            } else {
                0.0
            },
            heat_soak: self.heat_soak(),
            water_c: self.water_est.unwrap_or(f32::NAN),
            group_c: self.group_est.unwrap_or(f32::NAN),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sensors::{ntc_mv_for_c, pressure_to_mv};

    fn frame(cfg: &EngineConfig, k: u32, temp_c: f32, nominal_bar: f32, inputs: u8) -> SensorFrame {
        // `nominal_bar` is what the uncalibrated transfer function would read.
        let per_ns = (1e9 / cfg.mains_hz) as u64;
        SensorFrame {
            cycle: k,
            t_us: (k as u64 * per_ns + 500) / 1000,
            ntc_mv: ntc_mv_for_c(&cfg.ntc, temp_c).round() as u16,
            pressure_mv: pressure_to_mv(&cfg.pressure_sensor, nominal_bar, 0.0).round() as u16,
            scale_mg: 0,
            inputs,
        }
    }

    #[test]
    fn stuck_reading_under_setpoint_latches_heater_ineffective() {
        // The reading freezes 4 C under setpoint: the PID keeps adding heat the reading never
        // shows. The energy balance must catch it; a real boiler would have risen far more.
        let cfg = EngineConfig::default();
        let mut e = Engine::new(cfg.clone(), Profile::classic_9bar());
        let hz = cfg.mains_hz as u32;
        let mut k = 0;
        for _ in 0..(400 * hz) {
            // pressure climbing as the sealed boiler heats (thermal expansion), as in the emulator
            let bar = (3.0 + k as f32 / hz as f32 * 0.012).min(8.0);
            e.tick(&frame(&cfg, k, 89.0, bar, 0));
            k += 1;
            if e.faults & fault::HEATER_INEFFECTIVE != 0 {
                break;
            }
        }
        assert!(e.faults & fault::HEATER_INEFFECTIVE != 0, "not latched after {} s, duty avg {}", k / hz, e.heater_duty_avg.get());
    }

    #[test]
    fn steady_at_setpoint_does_not_trip_heater_check() {
        // A reading sitting at setpoint with idle duty must never trip the energy balance.
        let cfg = EngineConfig::default();
        let mut e = Engine::new(cfg.clone(), Profile::classic_9bar());
        let hz = cfg.mains_hz as u32;
        for k in 0..(900 * hz) {
            e.tick(&frame(&cfg, k, cfg.brew_temp_c, 0.0, 0));
        }
        assert_eq!(e.faults & fault::HEATER_INEFFECTIVE, 0);
    }

    #[test]
    fn one_stroke_per_pump_cycle_and_flow_follows_stroke_fraction() {
        let cfg = EngineConfig {
            hot_water_pump_duty: 0.6,
            ..Default::default()
        };
        let mut e = Engine::new(cfg.clone(), Profile::classic_9bar());
        let mut k = 0;
        for _ in 0..120 {
            e.tick(&frame(&cfg, k, 93.0, 0.0, 0));
            k += 1;
        }
        // Hot water: fixed duty, open wand (~0.5 bar of back-pressure).
        let mut strokes = 0;
        let mut last = ActuatorFrame::default();
        let n = 600;
        for _ in 0..n {
            last = e.tick(&frame(&cfg, k, 93.0, 0.5, input::WATER));
            strokes += last.pump as u32;
            k += 1;
        }
        assert_eq!(e.state(), EngineState::HotWater);
        // Sigma-delta: exactly duty x cycles, to within one stroke.
        assert!((strokes as i32 - 360).abs() <= 1, "{strokes} strokes");
        // Flow estimate = stroke fraction x pump curve at the line pressure.
        let expect = 0.6 * pump::max_flow_ml_s(&cfg.pump, 0.5);
        assert!(
            (last.status.flow_ml_s - expect).abs() < 0.05 * expect,
            "{} vs {expect}",
            last.status.flow_ml_s
        );
    }

    #[test]
    fn pumped_volume_counts_strokes() {
        // 50 Hz: each stroke at ~0 bar delivers curve(0)/50 ml.
        let cfg = EngineConfig {
            mains_hz: 50.0,
            ..Default::default()
        };
        let mut e = Engine::new(cfg.clone(), Profile::flush(2.0));
        let mut k = 0;
        for _ in 0..100 {
            e.tick(&frame(&cfg, k, 93.0, 0.3, 0));
            k += 1;
        }
        e.command(Command::StartBrew).unwrap();
        let mut strokes = 0u32;
        let mut prev_pump = false;
        loop {
            let a = e.tick(&frame(&cfg, k, 93.0, 0.3, 0));
            // Strokes are executed the cycle after they are decided.
            strokes += prev_pump as u32;
            prev_pump = a.pump;
            k += 1;
            if a.status.state == EngineState::BrewDone || k > 1000 {
                break;
            }
        }
        let shot = e.last_shot().unwrap().1;
        let per_stroke = pump::max_flow_ml_s(&cfg.pump, 0.3) / 50.0;
        let expect = strokes as f32 * per_stroke;
        assert!(strokes > 50, "{strokes}");
        assert!(
            (shot.pumped_ml - expect).abs() < per_stroke * 1.5,
            "{} vs {expect}",
            shot.pumped_ml
        );
    }

    fn zero_after_boot(temp_c: f32, nominal_bar: f32) -> f32 {
        let cfg = EngineConfig::default();
        let mut e = Engine::new(cfg.clone(), Profile::classic_9bar());
        for k in 0..60 {
            e.tick(&frame(&cfg, k, temp_c, nominal_bar, 0));
        }
        e.pressure_zero()
    }

    #[test]
    fn pressure_self_zero_only_when_physically_at_atmosphere() {
        // Cold, small drift: adopted.
        assert!((zero_after_boot(22.0, 0.2) - 0.2).abs() < 0.02);
        // Cold but 2 bar on the gauge: a stuck valve or bad sensor, not drift.
        assert_eq!(zero_after_boot(22.0, 2.0), 0.0);
        // Hot boiler after a quick power cycle: trapped pressure, refuse.
        assert_eq!(zero_after_boot(90.0, 0.2), 0.0);
        // Disabled.
        let cfg = EngineConfig {
            pressure_sensor: crate::config::PressureSensorConfig {
                auto_zero_s: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut e = Engine::new(cfg.clone(), Profile::classic_9bar());
        for k in 0..60 {
            e.tick(&frame(&cfg, k, 22.0, 0.2, 0));
        }
        assert_eq!(e.pressure_zero(), 0.0);
    }

    #[test]
    fn ntc_faults_raise_after_debounce() {
        let cfg = EngineConfig::default();
        let mut e = Engine::new(cfg.clone(), Profile::classic_9bar());
        let mut k = 0;
        for _ in 0..60 {
            e.tick(&frame(&cfg, k, 93.0, 0.0, 0));
            k += 1;
        }
        let mut f = frame(&cfg, k, 93.0, 0.0, 0);
        f.ntc_mv = 3300;
        // Under the 0.5 s debounce: no fault yet.
        for _ in 0..20 {
            f.cycle = k;
            f.t_us = frame(&cfg, k, 93.0, 0.0, 0).t_us;
            assert_eq!(e.tick(&f).status.faults & fault::NTC_OPEN, 0);
            k += 1;
        }
        for _ in 0..20 {
            f.cycle = k;
            f.t_us = frame(&cfg, k, 93.0, 0.0, 0).t_us;
            e.tick(&f);
            k += 1;
        }
        assert_ne!(e.faults() & fault::NTC_OPEN, 0);
        assert_eq!(e.state(), EngineState::Fault);
        // Shorted: the other bit.
        let mut e = Engine::new(cfg.clone(), Profile::classic_9bar());
        for k in 0..60 {
            let mut f = frame(&cfg, k, 93.0, 0.0, 0);
            f.ntc_mv = 3;
            e.tick(&f);
        }
        assert_eq!(
            e.faults() & (fault::NTC_SHORT | fault::NTC_OPEN),
            fault::NTC_SHORT
        );
    }
}
