//! # Scenario DSL
//!
//! A scenario is a plain-text script, one statement per line:
//!
//! ```text
//! name "probe falls off mid-shot"
//! machine "../models/gaggia_classic_pro_e24_230v.toml"
//! seed 42
//! duration 10min
//! model puck.grind = 1.1                       # override a model parameter
//! trace boiler.temp, engine.temp, hydraulics.pressure
//!
//! when engine.state == ready for 10s as warm: brew on
//! at warm+20s: set tc.fault = detached; mark "probe off"
//! at warm+40s: brew off
//! at warm+60s: ramp mains.voltage_v to 200 over 5s
//!
//! expect always boiler.temp < 175C
//! expect eventually engine.faults has heater_ineffective from warm+20s by warm+200s
//! expect at warm+19s: hydraulics.pressure > 7bar
//! expect never engine.state == fault until warm+19s
//! ```
//!
//! * Times: `250ms`, `12s`, `5min`, `label`, `label+3s`, `label-1s`.
//! * Labels are set when a `when ... as label:` rule fires. `when cond [for dur]
//!   [from time] [as label]:` only arms after `from` (e.g. the second shot).
//! * Actions: `brew|steam|water on|off`, `wand open|close`, `fresh puck`,
//!   `set path = value`, `ramp path to value over dur`, `mark "text"`, `end`.
//! * Conditions: `signal OP value` with `< <= > >= == != has`, combined with
//!   `and`, `or`, `not`, parentheses. Words compare against enum signals
//!   (`engine.state == brewing`, `engine.faults has over_temp`).
//! * Units on values are checked but not converted (signals use C, bar, g, ml,
//!   ml/s, V, W, s); `%` and `ms` are scaled.

pub mod ast;
pub mod lex;
pub mod parse;
pub mod run;

pub use ast::{Scenario, Value};
pub use parse::parse;
pub use run::{Outcome, Report, Runner, World, eval, fault_names};
