//! Line-oriented parser. One statement per line; `#` starts a comment.

use crate::ast::*;
use crate::lex::{Tok, check_value_unit, lex, to_seconds};
use anyhow::{Context, Result, anyhow, bail};

struct P {
    toks: Vec<Tok>,
    pos: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Result<Tok> {
        let t = self
            .toks
            .get(self.pos)
            .cloned()
            .ok_or_else(|| anyhow!("unexpected end of line"))?;
        self.pos += 1;
        Ok(t)
    }
    fn done(&self) -> bool {
        self.pos >= self.toks.len()
    }
    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s == kw)
    }
    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Sym(x)) if *x == s)
    }
    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.is_kw(kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str) -> Result<()> {
        match self.next()? {
            Tok::Sym(x) if x == s => Ok(()),
            t => bail!("expected `{s}`, found {t}"),
        }
    }
    fn expect_kw(&mut self, kw: &str) -> Result<()> {
        match self.next()? {
            Tok::Ident(x) if x == kw => Ok(()),
            t => bail!("expected `{kw}`, found {t}"),
        }
    }
    fn ident(&mut self) -> Result<String> {
        match self.next()? {
            Tok::Ident(s) => Ok(s),
            t => bail!("expected a name, found {t}"),
        }
    }
    fn string(&mut self) -> Result<String> {
        match self.next()? {
            Tok::Str(s) => Ok(s),
            t => bail!("expected a \"string\", found {t}"),
        }
    }
    fn end(&self) -> Result<()> {
        match self.peek() {
            None => Ok(()),
            Some(t) => bail!("unexpected {t}"),
        }
    }

    fn signed_num(&mut self) -> Result<(f64, Option<String>)> {
        let neg = self.eat_sym("-");
        match self.next()? {
            Tok::Num(n, u) => Ok((if neg { -n } else { n }, u)),
            t => bail!("expected a number, found {t}"),
        }
    }

    fn duration(&mut self) -> Result<f64> {
        let (n, u) = self.signed_num()?;
        to_seconds(n, u.as_deref())
    }

    fn value_num(&mut self) -> Result<f64> {
        let (n, u) = self.signed_num()?;
        Ok(n * check_value_unit(u.as_deref())?)
    }

    /// `12s`, `warm`, `warm+12s`, `warm-1s`
    fn time_ref(&mut self) -> Result<TimeRef> {
        match self.peek() {
            Some(Tok::Ident(_)) => {
                let label = self.ident()?;
                let offset = if self.eat_sym("+") {
                    self.duration()?
                } else if self.eat_sym("-") {
                    -self.duration()?
                } else {
                    0.0
                };
                Ok(TimeRef {
                    label: Some(label),
                    offset_s: offset,
                })
            }
            _ => Ok(TimeRef {
                label: None,
                offset_s: self.duration()?,
            }),
        }
    }

    fn value(&mut self) -> Result<Value> {
        match self.peek() {
            Some(Tok::Ident(_)) => Ok(Value::Word(self.ident()?)),
            _ => Ok(Value::Num(self.value_num()?)),
        }
    }

    fn operand(&mut self) -> Result<Operand> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.pos += 1;
                // Dotted names are signals; bare words are enum values (or signals
                // like `time`).
                if s.contains('.') || s == "time" {
                    Ok(Operand::Signal(s))
                } else {
                    Ok(Operand::Word(s))
                }
            }
            _ => Ok(Operand::Num(self.value_num()?)),
        }
    }

    fn cmp(&mut self) -> Result<Cond> {
        if self.eat_kw("not") {
            return Ok(Cond::Not(Box::new(self.cmp()?)));
        }
        if self.eat_sym("(") {
            let c = self.cond()?;
            self.expect_sym(")")?;
            return Ok(c);
        }
        let lhs = self.operand()?;
        let op = match self.next()? {
            Tok::Sym("<") => CmpOp::Lt,
            Tok::Sym("<=") => CmpOp::Le,
            Tok::Sym(">") => CmpOp::Gt,
            Tok::Sym(">=") => CmpOp::Ge,
            Tok::Sym("==") | Tok::Sym("=") => CmpOp::Eq,
            Tok::Sym("!=") => CmpOp::Ne,
            Tok::Ident(s) if s == "has" => CmpOp::Has,
            t => bail!("expected a comparison (< <= > >= == != has), found {t}"),
        };
        let rhs = self.operand()?;
        Ok(Cond::Cmp(lhs, op, rhs))
    }

    fn and(&mut self) -> Result<Cond> {
        let mut c = self.cmp()?;
        while self.eat_kw("and") {
            c = Cond::And(Box::new(c), Box::new(self.cmp()?));
        }
        Ok(c)
    }

    fn cond(&mut self) -> Result<Cond> {
        let mut c = self.and()?;
        while self.eat_kw("or") {
            c = Cond::Or(Box::new(c), Box::new(self.and()?));
        }
        Ok(c)
    }

    fn action(&mut self) -> Result<Action> {
        let kw = self.ident()?;
        Ok(match kw.as_str() {
            "brew" | "steam" | "water" => {
                let v = self.ident()?;
                let on = match v.as_str() {
                    "on" => 1.0,
                    "off" => 0.0,
                    _ => bail!("`{kw}` takes on|off"),
                };
                Action::Set(format!("switch.{kw}"), Value::Num(on))
            }
            "wand" => {
                let v = self.ident()?;
                let on = match v.as_str() {
                    "open" => 1.0,
                    "close" | "closed" => 0.0,
                    _ => bail!("`wand` takes open|close"),
                };
                Action::Set("wand.open".into(), Value::Num(on))
            }
            "fresh" => {
                self.expect_kw("puck")?;
                Action::Set("puck.fresh".into(), Value::Num(1.0))
            }
            "set" => {
                let path = self.ident()?;
                self.expect_sym("=")?;
                Action::Set(path, self.value()?)
            }
            "ramp" => {
                let path = self.ident()?;
                self.expect_kw("to")?;
                let to = self.value_num()?;
                self.expect_kw("over")?;
                let over_s = self.duration()?;
                Action::Ramp { path, to, over_s }
            }
            "mark" => Action::Mark(self.string()?),
            "end" => Action::End,
            _ => bail!(
                "unknown action `{kw}` (brew, steam, water, wand, fresh puck, set, ramp, mark, end)"
            ),
        })
    }

    fn actions(&mut self) -> Result<Vec<Action>> {
        let mut v = vec![self.action()?];
        while self.eat_sym(";") {
            v.push(self.action()?);
        }
        self.end()?;
        Ok(v)
    }

    fn window(&mut self, a: &str, b: &str) -> Result<(Option<TimeRef>, Option<TimeRef>)> {
        let mut first = None;
        let mut second = None;
        loop {
            if self.eat_kw(a) {
                first = Some(self.time_ref()?);
            } else if self.eat_kw(b) {
                second = Some(self.time_ref()?);
            } else {
                break;
            }
        }
        Ok((first, second))
    }
}

pub fn parse(src: &str) -> Result<Scenario> {
    let mut sc = Scenario {
        seed: 1,
        duration_s: 600.0,
        trace_every_s: 0.1,
        ..Default::default()
    };
    for (i, raw) in src.lines().enumerate() {
        let line_no = i + 1;
        let res: Result<()> = (|| {
            let toks = lex(raw)?;
            if toks.is_empty() {
                return Ok(());
            }
            let mut p = P { toks, pos: 0 };
            let kw = p.ident()?;
            match kw.as_str() {
                "name" => {
                    sc.name = p.string()?;
                    p.end()?;
                }
                "machine" => {
                    sc.machine = Some(p.string()?);
                    p.end()?;
                }
                "engine_arg" => {
                    sc.engine_args.push(p.string()?);
                    p.end()?;
                }
                "seed" => {
                    sc.seed = p.value_num()? as u64;
                    p.end()?;
                }
                "duration" => {
                    sc.duration_s = p.duration()?;
                    p.end()?;
                }
                "model" => {
                    let path = p.ident()?;
                    p.expect_sym("=")?;
                    sc.model.push((path, p.value_num()?));
                    p.end()?;
                }
                "trace" => {
                    loop {
                        sc.trace.push(p.ident()?);
                        if !p.eat_sym(",") {
                            break;
                        }
                    }
                    p.end()?;
                }
                "trace_every" => {
                    sc.trace_every_s = p.duration()?;
                    p.end()?;
                }
                "at" => {
                    let t = p.time_ref()?;
                    p.expect_sym(":")?;
                    sc.rules.push(Rule {
                        line: line_no,
                        trigger: Trigger::At(t),
                        label: None,
                        actions: p.actions()?,
                    });
                }
                "when" => {
                    let cond = p.cond()?;
                    let hold_s = if p.eat_kw("for") { p.duration()? } else { 0.0 };
                    let from = if p.eat_kw("from") {
                        Some(p.time_ref()?)
                    } else {
                        None
                    };
                    let label = if p.eat_kw("as") {
                        Some(p.ident()?)
                    } else {
                        None
                    };
                    p.expect_sym(":")?;
                    let actions = if p.done() { vec![] } else { p.actions()? };
                    sc.rules.push(Rule {
                        line: line_no,
                        trigger: Trigger::When { cond, hold_s, from },
                        label,
                        actions,
                    });
                }
                "expect" => {
                    let kind_kw = p.ident()?;
                    let (kind, cond) = match kind_kw.as_str() {
                        "always" | "never" => {
                            let cond = p.cond()?;
                            let (from, until) = p.window("from", "until")?;
                            let k = if kind_kw == "always" {
                                ExpectKind::Always { from, until }
                            } else {
                                ExpectKind::Never { from, until }
                            };
                            (k, cond)
                        }
                        "eventually" => {
                            let cond = p.cond()?;
                            let (from, by) = p.window("from", "by")?;
                            (ExpectKind::Eventually { from, by }, cond)
                        }
                        "at" => {
                            let t = p.time_ref()?;
                            p.expect_sym(":")?;
                            (ExpectKind::At(t), p.cond()?)
                        }
                        k => bail!("unknown expectation `{k}` (always, never, eventually, at)"),
                    };
                    p.end()?;
                    sc.expects.push(Expect {
                        line: line_no,
                        source: raw.trim().to_string(),
                        kind,
                        cond,
                    });
                }
                k => bail!("unknown statement `{k}`"),
            }
            Ok(())
        })();
        res.with_context(|| format!("line {line_no}: {}", raw.trim()))?;
    }
    // Labels used must be defined somewhere.
    let defined: Vec<&str> = sc.rules.iter().filter_map(|r| r.label.as_deref()).collect();
    let mut used = vec![];
    for r in &sc.rules {
        match &r.trigger {
            Trigger::At(t) => used.push((r.line, t.label.clone())),
            Trigger::When { from: Some(t), .. } => used.push((r.line, t.label.clone())),
            _ => {}
        }
    }
    for e in &sc.expects {
        let refs: Vec<&Option<TimeRef>> = match &e.kind {
            ExpectKind::Always { from, until } | ExpectKind::Never { from, until } => {
                vec![from, until]
            }
            ExpectKind::Eventually { from, by } => vec![from, by],
            ExpectKind::At(_) => vec![],
        };
        for t in refs.into_iter().flatten() {
            used.push((e.line, t.label.clone()));
        }
        if let ExpectKind::At(t) = &e.kind {
            used.push((e.line, t.label.clone()));
        }
    }
    for (line, l) in used {
        if let Some(l) = l
            && !defined.contains(&l.as_str())
        {
            bail!("line {line}: label `{l}` is never defined (define it with `when ... as {l}:`)");
        }
    }
    Ok(sc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_example() {
        let src = r#"
name "pump wears mid-shot"
machine "../models/gaggia_classic_pro_e24.toml"
seed 42
duration 5min
model thermal.boiler_mass_kg = 1.1
trace boiler.temp, engine.state

when engine.state == ready for 10s as warm: brew on
at warm+12s: set pump.wear = 0.6; mark "wear"
at warm+20s: set tc.fault = detached
ramp_dummy_should_not_parse
"#;
        assert!(parse(src).is_err());
        let src = src.replace(
            "ramp_dummy_should_not_parse",
            "at warm+30s: ramp mains.voltage_v to 200V over 5s",
        );
        let src = src
            + "expect always boiler.temp < 165C\n"
            + "expect eventually engine.faults has heater_ineffective from warm+20s by warm+120s\n"
            + "expect at warm+35s: cup.mass > 30g and not (engine.state == fault)\n";
        let sc = parse(&src).unwrap();
        assert_eq!(sc.name, "pump wears mid-shot");
        assert_eq!(sc.duration_s, 300.0);
        assert_eq!(sc.rules.len(), 4);
        assert_eq!(sc.expects.len(), 3);
        assert_eq!(sc.rules[1].actions.len(), 2);
        assert!(matches!(sc.rules[3].actions[0], Action::Ramp { over_s, .. } if over_s == 5.0));
    }

    #[test]
    fn undefined_label() {
        let e = parse("at nope+1s: brew on").unwrap_err();
        assert!(format!("{e:#}").contains("never defined"));
    }

    #[test]
    fn good_errors() {
        let e = parse("at 1s: brew maybe").unwrap_err();
        assert!(format!("{e:#}").contains("line 1"));
        assert!(format!("{e:#}").contains("on|off"));
    }
}
