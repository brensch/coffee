//! The hardware boundary.
//!
//! Everything the engine knows about the outside world arrives in a [`SensorFrame`],
//! and everything it does to the world leaves in an [`ActuatorFrame`]. The frames carry
//! *raw* peripheral data (a MAX31855 SPI word, ADS1115 counts, ...) rather than
//! engineering units, so the conversion code in the engine is exercised identically
//! whether the frame came from real silicon or from the emulator.
//!
//! Timing contract: the engine is ticked once per mains half-cycle, at the zero
//! crossing. The actuator frame it returns applies to the half-cycle that is starting.
//! A zero-cross SSR (heater) and a triac (pump) can only change state at a zero
//! crossing anyway, so this is the natural control quantum.
//!
//! The vibratory pump has an internal diode and only strokes on the positive
//! half-cycle. Zero-cross detectors do not report polarity, so the engine commands the
//! pump in whole mains cycles: `pump` must be held for pairs of ticks starting on an
//! even `half_cycle` count. Firmware and emulator both honour that.

#![no_std]

pub mod ads1115;
pub mod max31855;
pub mod wire;

/// Bit flags for [`SensorFrame::inputs`].
pub mod input {
    /// Front panel brew switch closed.
    pub const BREW: u8 = 1 << 0;
    /// Front panel steam switch closed.
    pub const STEAM: u8 = 1 << 1;
    /// Hot water switch closed (machines that have one).
    pub const WATER: u8 = 1 << 2;
    /// A scale is connected and `scale_mg` is valid.
    pub const SCALE_OK: u8 = 1 << 3;
}

/// One sample of the world, taken at a mains zero crossing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SensorFrame {
    /// Monotonic count of half-cycles since boot. Parity selects pump-legal ticks.
    pub half_cycle: u32,
    /// Monotonic microseconds at this zero crossing.
    pub t_us: u64,
    /// Measured duration of the half-cycle that just ended (microseconds).
    pub half_period_us: u32,
    /// Latest raw 32-bit MAX31855 read-out (see [`max31855`]).
    pub max31855: u32,
    /// Latest raw ADS1115 conversion for the pressure channel (gain +/-4.096 V).
    pub pressure_adc: i16,
    /// Scale reading in milligrams. Only meaningful when [`input::SCALE_OK`] is set.
    pub scale_mg: i32,
    /// Digital inputs, see [`input`].
    pub inputs: u8,
}

/// What the engine wants the hardware to do for the coming half-cycle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActuatorFrame {
    /// Echo of the sensor frame's half-cycle this answers.
    pub half_cycle: u32,
    /// Heater SSR conducts this half-cycle.
    pub heater: bool,
    /// Pump triac conducts this half-cycle.
    pub pump: bool,
    /// 3-way solenoid energised (boiler connected to group).
    pub valve: bool,
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
}

impl EngineState {
    pub const ALL: [EngineState; 8] = [
        EngineState::Boot,
        EngineState::Heating,
        EngineState::Ready,
        EngineState::Brewing,
        EngineState::BrewDone,
        EngineState::Steam,
        EngineState::HotWater,
        EngineState::Fault,
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
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|st| st.name() == s)
    }
}

/// Fault codes. Stored as a bitmask so several can be active at once.
pub mod fault {
    pub const TC_OPEN: u16 = 1 << 0;
    pub const TC_SHORT: u16 = 1 << 1;
    pub const OVER_TEMP: u16 = 1 << 2;
    pub const TEMP_IMPLAUSIBLE: u16 = 1 << 3;
    pub const PRESSURE_SENSOR: u16 = 1 << 4;
    pub const OVER_PRESSURE: u16 = 1 << 5;
    pub const HEATER_INEFFECTIVE: u16 = 1 << 6;
    pub const NO_WATER: u16 = 1 << 7;
    pub const MAINS: u16 = 1 << 8;
    pub const TEMP_RUNAWAY: u16 = 1 << 9;

    pub const NAMES: [(u16, &str); 10] = [
        (TC_OPEN, "tc_open"),
        (TC_SHORT, "tc_short"),
        (OVER_TEMP, "over_temp"),
        (TEMP_IMPLAUSIBLE, "temp_implausible"),
        (PRESSURE_SENSOR, "pressure_sensor"),
        (OVER_PRESSURE, "over_pressure"),
        (HEATER_INEFFECTIVE, "heater_ineffective"),
        (NO_WATER, "no_water"),
        (MAINS, "mains"),
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
