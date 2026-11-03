//! The engine. Pure, deterministic, `no_std` (+ `alloc`): it owns no clocks, no
//! peripherals and no I/O. Feed it a [`coffee_io::SensorFrame`] once per mains
//! cycle and apply the [`coffee_io::ActuatorFrame`] it returns.
//!
//! The same code runs inside `coffee-engine` (host binary, talking to the emulator or
//! to a USB hardware bridge) and inside the ESP32 firmware.

#![no_std]
extern crate alloc;

pub mod config;
pub mod control;
pub mod engine;
pub mod profile;
pub mod pump;
pub mod sensors;

pub use config::EngineConfig;
pub use engine::{BrewEnd, Command, Engine, ShotSummary};
pub use profile::Profile;
