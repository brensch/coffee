//! Executes a parsed scenario against anything that implements [`World`].

use crate::ast::*;
use anyhow::{Result, anyhow, bail};
use coffee_io::{EngineState, fault};
use std::collections::HashMap;

/// What a scenario can see and poke. Implemented by the emulator.
pub trait World {
    /// Read a signal (physics truth, sensor, or `engine.*` status).
    fn get(&self, name: &str) -> Option<f64>;
    /// Set a knob, fault or model parameter.
    fn set(&mut self, path: &str, v: &Value) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Pending,
    Pass,
    Fail(String),
}

#[derive(Clone, Debug)]
pub struct ExpectResult {
    pub line: usize,
    pub source: String,
    pub outcome: Outcome,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub name: String,
    pub sim_time_s: f64,
    pub expects: Vec<ExpectResult>,
    pub events: Vec<(f64, String)>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.expects.iter().all(|e| e.outcome == Outcome::Pass)
    }
}

struct Ramp {
    path: String,
    from: f64,
    to: f64,
    t0: f64,
    over: f64,
}

pub struct Runner {
    pub sc: Scenario,
    labels: HashMap<String, f64>,
    fired: Vec<bool>,
    hold_since: Vec<Option<f64>>,
    ramps: Vec<Ramp>,
    outcomes: Vec<Outcome>,
    pub events: Vec<(f64, String)>,
    pub ended: bool,
}

fn word_value(other: Option<&str>, w: &str) -> Result<f64> {
    if let Some(name) = other {
        if name == "engine.state" {
            return EngineState::from_name(w)
                .map(|s| s as u8 as f64)
                .ok_or_else(|| anyhow!("unknown engine state `{w}`"));
        }
        if name == "engine.faults" {
            return fault::from_name(w)
                .map(|b| b as f64)
                .ok_or_else(|| anyhow!("unknown fault `{w}`"));
        }
    }
    match w {
        "on" | "true" | "open" | "yes" => Ok(1.0),
        "off" | "false" | "closed" | "no" | "none" => Ok(0.0),
        _ => bail!("cannot compare with word `{w}`"),
    }
}

fn operand(world: &dyn World, o: &Operand, other: &Operand) -> Result<f64> {
    let other_name = match other {
        Operand::Signal(s) => Some(s.as_str()),
        _ => None,
    };
    match o {
        Operand::Num(n) => Ok(*n),
        Operand::Signal(s) => world.get(s).ok_or_else(|| anyhow!("unknown signal `{s}`")),
        Operand::Word(w) => word_value(other_name, w),
    }
}

/// Evaluate a condition. Returns the truth value and a human description of the
/// operands for failure messages.
pub fn eval(world: &dyn World, c: &Cond) -> Result<(bool, String)> {
    Ok(match c {
        Cond::Cmp(a, op, b) => {
            let x = operand(world, a, b)?;
            let y = operand(world, b, a)?;
            let r = match op {
                CmpOp::Lt => x < y,
                CmpOp::Le => x <= y,
                CmpOp::Gt => x > y,
                CmpOp::Ge => x >= y,
                CmpOp::Eq => (x - y).abs() < 1e-9,
                // NaN (signal undefined) counts as "not equal".
                CmpOp::Ne => (x - y).abs().is_nan() || (x - y).abs() >= 1e-9,
                CmpOp::Has => (x as u64) & (y as u64) != 0,
            };
            let name = match a {
                Operand::Signal(s) => s.clone(),
                _ => "lhs".into(),
            };
            let shown = match (a, x) {
                (Operand::Signal(s), x) if s == "engine.state" => {
                    EngineState::from_u8(x as u8).name().to_string()
                }
                (Operand::Signal(s), x) if s == "engine.faults" => fault_names(x as u16),
                _ => format!("{x:.3}"),
            };
            (r, format!("{name}={shown}"))
        }
        Cond::And(a, b) => {
            let (x, dx) = eval(world, a)?;
            let (y, dy) = eval(world, b)?;
            (x && y, format!("{dx}, {dy}"))
        }
        Cond::Or(a, b) => {
            let (x, dx) = eval(world, a)?;
            let (y, dy) = eval(world, b)?;
            (x || y, format!("{dx}, {dy}"))
        }
        Cond::Not(a) => {
            let (x, d) = eval(world, a)?;
            (!x, d)
        }
    })
}

pub fn fault_names(mask: u16) -> String {
    let v: Vec<&str> = fault::NAMES
        .iter()
        .filter(|(b, _)| mask & b != 0)
        .map(|(_, n)| *n)
        .collect();
    if v.is_empty() {
        "none".into()
    } else {
        v.join("|")
    }
}

fn signals_in(c: &Cond, out: &mut Vec<String>) {
    match c {
        Cond::Cmp(a, _, b) => {
            for o in [a, b] {
                if let Operand::Signal(s) = o {
                    out.push(s.clone());
                }
            }
        }
        Cond::And(a, b) | Cond::Or(a, b) => {
            signals_in(a, out);
            signals_in(b, out);
        }
        Cond::Not(a) => signals_in(a, out),
    }
}

impl Runner {
    /// Validate signal names against the world and prepare to run.
    pub fn new(sc: Scenario, world: &dyn World) -> Result<Self> {
        let mut sigs = vec![];
        for r in &sc.rules {
            if let Trigger::When { cond, .. } = &r.trigger {
                signals_in(cond, &mut sigs);
            }
        }
        for e in &sc.expects {
            signals_in(&e.cond, &mut sigs);
        }
        sigs.extend(sc.trace.iter().cloned());
        for s in sigs {
            if world.get(&s).is_none() {
                bail!("unknown signal `{s}`");
            }
        }
        let n = sc.rules.len();
        let m = sc.expects.len();
        Ok(Self {
            sc,
            labels: HashMap::new(),
            fired: vec![false; n],
            hold_since: vec![None; n],
            ramps: vec![],
            outcomes: vec![Outcome::Pending; m],
            events: vec![],
            ended: false,
        })
    }

    fn resolve(&self, t: &TimeRef) -> Option<f64> {
        match &t.label {
            None => Some(t.offset_s),
            Some(l) => self.labels.get(l).map(|base| base + t.offset_s),
        }
    }

    pub fn label_time(&self, label: &str) -> Option<f64> {
        self.labels.get(label).copied()
    }

    fn apply(&mut self, t: f64, actions: &[Action], world: &mut dyn World) -> Result<()> {
        for a in actions {
            match a {
                Action::Set(path, v) => {
                    world.set(path, v)?;
                    let shown = match v {
                        Value::Num(n) => format!("{n}"),
                        Value::Word(w) => w.clone(),
                    };
                    self.events.push((t, format!("set {path} = {shown}")));
                }
                Action::Ramp { path, to, over_s } => {
                    let from = world
                        .get(path)
                        .ok_or_else(|| anyhow!("cannot ramp `{path}`: not readable"))?;
                    self.ramps.retain(|r| &r.path != path);
                    self.ramps.push(Ramp {
                        path: path.clone(),
                        from,
                        to: *to,
                        t0: t,
                        over: over_s.max(1e-6),
                    });
                    self.events
                        .push((t, format!("ramp {path} {from} -> {to} over {over_s}s")));
                }
                Action::Mark(text) => self.events.push((t, format!("mark: {text}"))),
                Action::End => {
                    self.events.push((t, "end".into()));
                    self.ended = true;
                }
            }
        }
        Ok(())
    }

    /// Call once per simulation step, after the world advanced to time `t`.
    pub fn step(&mut self, t: f64, world: &mut dyn World) -> Result<()> {
        // Rules.
        for i in 0..self.sc.rules.len() {
            if self.fired[i] {
                continue;
            }
            let rule = self.sc.rules[i].clone();
            let fire = match &rule.trigger {
                Trigger::At(tr) => self.resolve(tr).is_some_and(|at| t >= at),
                Trigger::When { cond, hold_s, from } => {
                    let armed = from
                        .as_ref()
                        .is_none_or(|f| self.resolve(f).is_some_and(|ft| t >= ft));
                    let (ok, _) = if armed {
                        eval(world, cond)?
                    } else {
                        (false, String::new())
                    };
                    if ok {
                        let since = *self.hold_since[i].get_or_insert(t);
                        t - since >= *hold_s
                    } else {
                        self.hold_since[i] = None;
                        false
                    }
                }
            };
            if fire {
                self.fired[i] = true;
                if let Some(l) = &rule.label {
                    self.labels.insert(l.clone(), t);
                    self.events.push((t, format!("label {l}")));
                }
                self.apply(t, &rule.actions, world)?;
            }
        }

        // Ramps.
        let mut done = vec![];
        for (i, r) in self.ramps.iter().enumerate() {
            let x = ((t - r.t0) / r.over).clamp(0.0, 1.0);
            world.set(&r.path, &Value::Num(r.from + (r.to - r.from) * x))?;
            if x >= 1.0 {
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            self.ramps.remove(i);
        }

        // Expectations.
        for i in 0..self.sc.expects.len() {
            if self.outcomes[i] != Outcome::Pending {
                if !matches!(
                    self.sc.expects[i].kind,
                    ExpectKind::Always { .. } | ExpectKind::Never { .. }
                ) {
                    continue;
                }
                if matches!(self.outcomes[i], Outcome::Fail(_)) {
                    continue;
                }
            }
            let e = self.sc.expects[i].clone();
            let in_window = |from: &Option<TimeRef>, until: &Option<TimeRef>| -> bool {
                let after = match from {
                    None => true,
                    Some(f) => self.resolve(f).is_some_and(|ft| t >= ft),
                };
                let before = match until {
                    None => true,
                    Some(u) => self.resolve(u).is_none_or(|ut| t <= ut),
                };
                after && before
            };
            match &e.kind {
                ExpectKind::Always { from, until } => {
                    if in_window(from, until) {
                        let (ok, d) = eval(world, &e.cond)?;
                        if !ok {
                            self.outcomes[i] =
                                Outcome::Fail(format!("violated at t={t:.2}s ({d})"));
                        }
                    }
                }
                ExpectKind::Never { from, until } => {
                    if in_window(from, until) {
                        let (ok, d) = eval(world, &e.cond)?;
                        if ok {
                            self.outcomes[i] =
                                Outcome::Fail(format!("happened at t={t:.2}s ({d})"));
                        }
                    }
                }
                ExpectKind::Eventually { from, by } => {
                    if in_window(from, &None) {
                        let (ok, _) = eval(world, &e.cond)?;
                        if ok {
                            self.outcomes[i] = Outcome::Pass;
                            self.events.push((t, format!("satisfied: {}", e.source)));
                        } else if let Some(b) = by.as_ref().and_then(|b| self.resolve(b))
                            && t > b
                        {
                            let (_, d) = eval(world, &e.cond)?;
                            self.outcomes[i] = Outcome::Fail(format!("not met by t={b:.2}s ({d})"));
                        }
                    }
                }
                ExpectKind::At(tr) => {
                    if let Some(at) = self.resolve(tr)
                        && t >= at
                    {
                        let (ok, d) = eval(world, &e.cond)?;
                        self.outcomes[i] = if ok {
                            Outcome::Pass
                        } else {
                            Outcome::Fail(format!("at t={t:.2}s: {d}"))
                        };
                    }
                }
            }
        }
        Ok(())
    }

    /// Close out the run: anything still pending is resolved.
    pub fn finish(mut self, t: f64) -> Report {
        for (i, e) in self.sc.expects.iter().enumerate() {
            if self.outcomes[i] != Outcome::Pending {
                continue;
            }
            let window_started = |f: &Option<TimeRef>| match f {
                None => true,
                Some(f) => match &f.label {
                    None => true,
                    Some(l) => self.labels.contains_key(l),
                },
            };
            self.outcomes[i] = match &e.kind {
                ExpectKind::Always { from, .. } | ExpectKind::Never { from, .. } => {
                    if window_started(from) {
                        Outcome::Pass
                    } else {
                        Outcome::Fail("window never started (label never fired)".into())
                    }
                }
                ExpectKind::Eventually { .. } => {
                    Outcome::Fail(format!("never happened (ran to t={t:.1}s)"))
                }
                ExpectKind::At(_) => {
                    Outcome::Fail("time never reached (label never fired or run too short)".into())
                }
            };
        }
        Report {
            name: self.sc.name.clone(),
            sim_time_s: t,
            expects: self
                .sc
                .expects
                .iter()
                .zip(self.outcomes)
                .map(|(e, o)| ExpectResult {
                    line: e.line,
                    source: e.source.clone(),
                    outcome: o,
                })
                .collect(),
            events: self.events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    struct Toy {
        t: f64,
        temp: f64,
        heater: f64,
        state: f64,
    }
    impl World for Toy {
        fn get(&self, n: &str) -> Option<f64> {
            match n {
                "time" => Some(self.t),
                "boiler.temp" => Some(self.temp),
                "heater.power" => Some(self.heater),
                "engine.state" => Some(self.state),
                _ => None,
            }
        }
        fn set(&mut self, p: &str, v: &Value) -> Result<()> {
            match (p, v) {
                ("heater.power", Value::Num(n)) => self.heater = *n,
                _ => bail!("nope"),
            }
            Ok(())
        }
    }

    #[test]
    fn runs_triggers_ramps_and_expectations() {
        let sc = parse(
            r#"
at 1s: set heater.power = 1000
when boiler.temp > 50 for 2s as hot: ramp heater.power to 0 over 1s
expect always boiler.temp < 200
expect eventually engine.state == ready by 20s
expect at hot+1.5s: heater.power < 600
expect never boiler.temp > 90
"#,
        )
        .unwrap();
        let mut w = Toy {
            t: 0.0,
            temp: 20.0,
            heater: 0.0,
            state: 1.0,
        };
        let mut r = Runner::new(sc, &w).unwrap();
        while w.t < 30.0 {
            w.t += 0.01;
            w.temp += w.heater * 0.01 * 0.01;
            if w.temp > 60.0 {
                w.state = EngineState::Ready as u8 as f64;
            }
            r.step(w.t, &mut w).unwrap();
        }
        let rep = r.finish(w.t);
        for e in &rep.expects {
            assert_eq!(e.outcome, Outcome::Pass, "{}", e.source);
        }
    }

    #[test]
    fn unknown_signal_is_rejected() {
        let sc = parse("expect always bogus.signal < 1").unwrap();
        let w = Toy {
            t: 0.0,
            temp: 0.0,
            heater: 0.0,
            state: 0.0,
        };
        assert!(Runner::new(sc, &w).is_err());
    }
}
