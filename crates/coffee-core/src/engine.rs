//! The engine state machine. See the crate docs for the timing contract.

use crate::config::EngineConfig;
use crate::control::{Lpf, Pid, SigmaDelta};
use crate::profile::{Observed, Profile, Target};
use crate::pump;
use crate::sensors::{self, PressureReading};
use coffee_io::max31855::TcFault;
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
    first_tick: bool,

    // temperature
    temp: Lpf,
    temp_rate: Lpf,
    temp_prev: Option<f32>,
    tc_bad_since: Option<f64>,
    tc_last_fault: Option<TcFault>,
    rejected_since: Option<f64>,

    // pressure
    pressure_ctl: Lpf,
    pressure_disp: Lpf,
    /// Slower pressure for exit conditions and safety (vibratory pumps ripple).
    pressure_slow: Lpf,
    pressure_bad_since: Option<f64>,
    over_pressure_since: Option<f64>,

    // pump / flow
    stroke_frac: Lpf,
    pump_cycle_on: bool,
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

    group_est: Option<f32>,
    water_est: Option<f32>,
    prev_target: Option<f32>,

    // safety windows
    heater_window: Option<(f64, f32)>,
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
            first_tick: true,
            temp: Lpf::default(),
            temp_rate: Lpf::default(),
            temp_prev: None,
            tc_bad_since: None,
            tc_last_fault: None,
            rejected_since: None,
            pressure_ctl: Lpf::default(),
            pressure_disp: Lpf::default(),
            pressure_slow: Lpf::default(),
            pressure_bad_since: None,
            over_pressure_since: None,
            stroke_frac: Lpf::default(),
            pump_cycle_on: false,
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
            group_est: None,
            water_est: None,
            prev_target: None,
            heater_window: None,
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
                if !(110.0..=150.0).contains(&t) {
                    return Err("steam temperature out of range 110..150");
                }
                self.cfg.steam_temp_c = t;
            }
            Command::SetConfig(c) => {
                if self.brew.is_some() {
                    return Err("cannot change config while brewing");
                }
                self.cfg = c;
            }
            Command::ClearFaults => {
                self.faults &= !LATCHING;
                self.heater_window = None;
                self.runaway_since = None;
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
        self.profile.temperature_c.unwrap_or(self.cfg.brew_temp_c)
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

    /// Advance one mains half-cycle.
    pub fn tick(&mut self, s: &SensorFrame) -> ActuatorFrame {
        let dt = if (1..=50_000).contains(&s.half_period_us) {
            s.half_period_us as f32 * 1e-6
        } else {
            0.01
        };
        self.t = s.t_us as f64 * 1e-6;

        if !self.first_tick {
            let sc = &self.cfg.safety;
            let ok = (sc.min_half_period_us..=sc.max_half_period_us).contains(&s.half_period_us);
            self.set_fault(fault::MAINS, !ok);
        }
        self.first_tick = false;

        self.update_temperature(s.max31855, dt);
        let p = self.update_pressure(s.pressure_adc, dt);
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
                self.settle = None;
            }
        }
        self.control(p, dt);

        // Faults override everything: heater, pump and valve off.
        if self.faults != 0 {
            self.heater_duty = 0.0;
            self.pump_duty = 0.0;
        }
        let valve = self.faults == 0
            && matches!(self.state, EngineState::Brewing)
            && self.brew.as_ref().is_some_and(|b| b.ended.is_none());

        let heater = self.heater_sd.next(self.heater_duty);
        // The pump may only *start* at the beginning of a mains cycle (even tick),
        // but may always be cut.
        if s.half_cycle.is_multiple_of(2) {
            self.pump_cycle_on = self.pump_sd.next(self.pump_duty);
        } else if self.pump_duty <= 0.0 {
            self.pump_cycle_on = false;
        }
        let pump = self.pump_cycle_on;

        ActuatorFrame {
            half_cycle: s.half_cycle,
            heater,
            pump,
            valve,
            status: self.status(p),
        }
    }

    fn update_temperature(&mut self, word: u32, dt: f32) {
        let now = self.t;
        match sensors::thermocouple_c(&self.cfg.thermocouple, word) {
            Ok(t) => {
                self.tc_bad_since = None;
                self.tc_last_fault = None;
                self.set_fault(fault::TC_OPEN, false);
                self.set_fault(fault::TC_SHORT, false);
                if let Some(f) = self.temp.value {
                    if (t - f).abs() > self.cfg.safety.max_temp_step_c {
                        let since = *self.rejected_since.get_or_insert(now);
                        if now - since > self.cfg.safety.max_rejected_s as f64 {
                            // Persistently far away: it is real (or the sensor is
                            // lying consistently). Flag it and re-seed.
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
                let v = self.temp.update(t, dt, self.cfg.thermocouple.filter_tau_s);
                if let Some(prev) = self.temp_prev {
                    self.temp_rate.update((v - prev) / dt, dt, 3.0);
                }
                self.temp_prev = Some(v);
            }
            Err(e) => {
                self.tc_last_fault = Some(e);
                let since = *self.tc_bad_since.get_or_insert(now);
                if now - since >= self.cfg.thermocouple.fault_debounce_s as f64 {
                    match e {
                        TcFault::Open => self.set_fault(fault::TC_OPEN, true),
                        _ => self.set_fault(fault::TC_SHORT, true),
                    }
                    self.temp.reset();
                    self.temp_prev = None;
                }
            }
        }
    }

    fn temp_valid(&self) -> bool {
        self.temp.value.is_some() && self.faults & (fault::TC_OPEN | fault::TC_SHORT) == 0
    }

    fn update_pressure(&mut self, counts: i16, dt: f32) -> f32 {
        match sensors::pressure(&self.cfg.pressure_sensor, counts) {
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
        if let Some(prev) = self.weight_prev {
            self.weight_rate.update((w - prev) / dt, dt, 0.6);
        }
        self.weight_prev = Some(w);
    }

    fn update_flow(&mut self, p: f32, dt: f32) {
        let frac = self
            .stroke_frac
            .update(if self.pump_cycle_on { 1.0 } else { 0.0 }, dt, 0.25);
        self.flow_est = frac * pump::max_flow_ml_s(&self.cfg.pump, p);
        if let Some(b) = self.brew.as_mut()
            && b.ended.is_none()
        {
            b.pumped_ml += self.flow_est * dt;
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

            // Heater ineffective: commanding lots of heat, no temperature rise.
            // Disabled while water is flowing (cold water legitimately absorbs it).
            let sp = self.setpoint();
            let heating_hard = self.heater_duty_avg.get() > sc.heater_ineffective_duty
                && temp < sp - 5.0
                && self.flow_est < 0.3;
            if heating_hard {
                let (t0, temp0) = *self.heater_window.get_or_insert((now, temp));
                if now - t0 > sc.heater_ineffective_s as f64 {
                    if temp - temp0 < sc.heater_ineffective_rise_c {
                        self.set_fault(fault::HEATER_INEFFECTIVE, true);
                    }
                    self.heater_window = Some((now, temp));
                }
            } else {
                self.heater_window = None;
            }

            // Runaway: heater off, far above setpoint and still climbing.
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
            self.heater_duty = self.temp_pid.update(&gains, sp, self.temp.get(), ff, dt);
        } else {
            self.heater_duty = 0.0;
            self.temp_pid.reset();
        }
        self.heater_duty_avg.update(self.heater_duty, dt, 20.0);

        // Water observer: C_w dTw/dt = h (T_wall - Tw) - q c_p (Tw - T_in).
        if self.temp_valid() && self.cfg.brew_boost_h_w_k > 0.0 {
            let tb = self.temp.get();
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
        } else if !self.temp_valid() {
            self.water_est = None;
        }

        // Portafilter model: heated by the boiler through the lugs, by (or, if
        // it's hotter, cooling) the water flowing through it, losing heat to the
        // room. A flush therefore warms it, which a lag filter can't express.
        if self.temp_valid() {
            let c = &self.cfg;
            let wall = self.temp.get();
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
            self.group_est = Some(g + q / c.group_heat_capacity_j_k.max(1.0) * dt);
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
