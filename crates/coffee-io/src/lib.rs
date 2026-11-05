//! The hardware boundary.
//!
//! Everything the engine knows about the outside world arrives in a [`SensorFrame`],
//! and everything it does to the world leaves in an [`ActuatorFrame`]. Sensor values
//! are carried as calibrated ADC millivolts at the MCU pin (the ESP32-C6 ADC1 with
//! eFuse curve calibration), not engineering units, so the NTC and pressure models in
//! the engine are exercised identically whether the frame came from real silicon or
//! from the emulator.
//!
//! Timing contract: the engine is ticked once per **mains cycle** (1/60 s or 1/50 s),
//! timed by the MCU crystal. There is no zero-cross detector: the MOC3063 opto-triacs
//! only fire a triac at a zero crossing, so an output raised at any point of a cycle
//! takes effect at the next crossing and an output dropped stops at the next current
//! zero. Firmware applies the decision for cycle `k` at the start of cycle `k + 1`.
//!
//! The vibratory pump has an internal diode and strokes only on the positive half
//! wave. A crystal window one mains period long contains exactly one positive-going
//! zero crossing, so "pump on for one cycle" is exactly one stroke, whatever the
//! phase of the window relative to the grid. (Off-nominal grid frequency slips one
//! window per beat period; see `coffee_board::grid`.)

#![no_std]

pub mod wire;

/// Bit flags for [`SensorFrame::inputs`].
pub mod input {
    /// Front panel brew switch closed (debounced).
    pub const BREW: u8 = 1 << 0;
    /// Front panel steam switch closed (debounced).
    pub const STEAM: u8 = 1 << 1;
    /// Hot water switch closed (machines that have one).
    pub const WATER: u8 = 1 << 2;
    /// A scale is connected and `scale_mg` is valid.
    pub const SCALE_OK: u8 = 1 << 3;
    /// Front-panel power switch is OFF: the controller stays powered and the engine idles in
    /// Standby (heater, pump and valve off). Never set on machines without a wired power switch.
    pub const STANDBY: u8 = 1 << 4;
}

/// One sample of the world: the cycle that just ended, reported at the tick that
/// ends it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SensorFrame {
    /// Tick count since boot (one tick per mains cycle).
    pub cycle: u32,
    /// Crystal time of this tick, microseconds: `t0 + round(cycle * period)`.
    pub t_us: u64,
    /// NTC divider node, millivolts (oversampled average over the cycle).
    pub ntc_mv: u16,
    /// Pressure transducer after the 10k/20k divider, millivolts.
    pub pressure_mv: u16,
    /// Scale reading in milligrams. Only meaningful when [`input::SCALE_OK`] is set.
    pub scale_mg: i32,
    /// Digital inputs, see [`input`].
    pub inputs: u8,
}

/// What the engine wants the hardware to do for the next mains cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActuatorFrame {
    /// Echo of the sensor frame's cycle this answers.
    pub cycle: u32,
    /// Heater triac conducts for the whole cycle.
    pub heater: bool,
    /// Pump triac conducts for the cycle: exactly one stroke.
    pub pump: bool,
    /// 3-way solenoid energised (boiler connected to group).
    pub valve: bool,
    /// Front-panel lamps (power, coffee, steam): the levels the board drives on its LED pins
    /// (GPIO21/22/16) this cycle. Filled by the engine from [`panel_leds`] on the sensor frame's clock.
    pub lamps: [bool; 3],
    /// Engine state for telemetry / UI / scenario assertions. Not used by hardware.
    pub status: EngineStatus,
}

/// Coarse state of the engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum EngineState {
    #[default]
    Boot = 0,
    Heating = 1,
    Ready = 2,
    Brewing = 3,
    BrewDone = 4,
    Steam = 5,
    HotWater = 6,
    Fault = 7,
    Standby = 8,
}

impl EngineState {
    pub const ALL: [EngineState; 9] = [
        EngineState::Boot,
        EngineState::Heating,
        EngineState::Ready,
        EngineState::Brewing,
        EngineState::BrewDone,
        EngineState::Steam,
        EngineState::HotWater,
        EngineState::Fault,
        EngineState::Standby,
    ];

    pub fn from_u8(v: u8) -> Self {
        Self::ALL
            .get(v as usize)
            .copied()
            .unwrap_or(EngineState::Fault)
    }

    pub fn name(self) -> &'static str {
        match self {
            EngineState::Boot => "boot",
            EngineState::Heating => "heating",
            EngineState::Ready => "ready",
            EngineState::Brewing => "brewing",
            EngineState::BrewDone => "brew_done",
            EngineState::Steam => "steam",
            EngineState::HotWater => "hot_water",
            EngineState::Fault => "fault",
            EngineState::Standby => "standby",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|st| st.name() == s)
    }
}

/// The machine's three front-panel lamps (replaced by LEDs): `[power, coffee, steam]`.
/// - power: on while the machine is on, off in standby, fast blink on any fault;
/// - coffee: slow blink while heating for a shot, solid when ready (and while brewing);
/// - steam: in steam mode, slow blink until the boiler is within 3 C of the steam setpoint, then solid.
pub fn panel_leds(state: EngineState, faults: u16, temp_c: f32, setpoint_c: f32, t_ms: u64) -> [bool; 3] {
    let fast = t_ms % 200 < 100;
    let slow = t_ms % 1000 < 500;
    if faults != 0 || state == EngineState::Fault {
        return [fast, false, false];
    }
    match state {
        EngineState::Standby => [false, false, false],
        EngineState::Boot | EngineState::Heating => [true, slow, false],
        EngineState::Ready | EngineState::Brewing | EngineState::BrewDone | EngineState::HotWater => [true, true, false],
        EngineState::Steam => [true, false, if (setpoint_c - temp_c).abs() <= 3.0 { true } else { slow }],
        EngineState::Fault => [fast, false, false],
    }
}

/// Fault codes. Stored as a bitmask so several can be active at once.
pub mod fault {
    /// NTC node near the 3.3 V rail: thermistor or its wiring open.
    pub const NTC_OPEN: u16 = 1 << 0;
    /// NTC node near ground: thermistor or its wiring shorted.
    pub const NTC_SHORT: u16 = 1 << 1;
    pub const OVER_TEMP: u16 = 1 << 2;
    pub const TEMP_IMPLAUSIBLE: u16 = 1 << 3;
    pub const PRESSURE_SENSOR: u16 = 1 << 4;
    pub const OVER_PRESSURE: u16 = 1 << 5;
    pub const HEATER_INEFFECTIVE: u16 = 1 << 6;
    pub const NO_WATER: u16 = 1 << 7;
    pub const TEMP_RUNAWAY: u16 = 1 << 8;

    pub const NAMES: [(u16, &str); 9] = [
        (NTC_OPEN, "ntc_open"),
        (NTC_SHORT, "ntc_short"),
        (OVER_TEMP, "over_temp"),
        (TEMP_IMPLAUSIBLE, "temp_implausible"),
        (PRESSURE_SENSOR, "pressure_sensor"),
        (OVER_PRESSURE, "over_pressure"),
        (HEATER_INEFFECTIVE, "heater_ineffective"),
        (NO_WATER, "no_water"),
        (TEMP_RUNAWAY, "temp_runaway"),
    ];

    pub fn from_name(s: &str) -> Option<u16> {
        NAMES.iter().find(|(_, n)| *n == s).map(|(b, _)| *b)
    }
}

/// Engine-side view, reported every tick for telemetry. Engineering units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineStatus {
    pub state: EngineState,
    /// Bitmask of [`fault`] codes.
    pub faults: u16,
    /// Index of the active profile phase (0xFF when not brewing).
    pub phase: u8,
    /// Filtered boiler temperature, deg C.
    pub temp_c: f32,
    /// Active temperature setpoint, deg C.
    pub setpoint_c: f32,
    /// Filtered pressure, bar (gauge).
    pub pressure_bar: f32,
    /// Profile target pressure, bar (NaN when flow-controlled).
    pub target_pressure_bar: f32,
    /// Estimated pump flow, ml/s.
    pub flow_ml_s: f32,
    /// Profile target flow, ml/s (NaN when pressure-controlled).
    pub target_flow_ml_s: f32,
    /// Weight in cup, g (NaN without scale).
    pub weight_g: f32,
    /// Seconds since brew start.
    pub shot_time_s: f32,
    /// Heater duty demanded by the controller, 0..1.
    pub heater_duty: f32,
    /// Pump duty demanded by the controller, 0..1.
    pub pump_duty: f32,
    /// How far above `setpoint_c` the boiler body is being driven so the water
    /// stays at setpoint while cold water flows in (deg C).
    pub boost_c: f32,
    /// Estimated group/portafilter heat-soak, 0..1 (1 = fully warmed).
    pub heat_soak: f32,
    /// Observer estimate of the boiler *water* temperature (the probe reads the
    /// boiler wall), deg C.
    pub water_c: f32,
    /// Estimated group/portafilter temperature, deg C.
    pub group_c: f32,
}
