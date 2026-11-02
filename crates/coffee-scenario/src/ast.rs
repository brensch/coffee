#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Num(f64),
    Word(String),
}

/// A point in time: absolute, or relative to a label set by a `when ... as label`.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeRef {
    pub label: Option<String>,
    pub offset_s: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    Signal(String),
    Num(f64),
    Word(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    /// Bitmask contains (e.g. `engine.faults has over_temp`).
    Has,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cond {
    Cmp(Operand, CmpOp, Operand),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Set(String, Value),
    Ramp { path: String, to: f64, over_s: f64 },
    Mark(String),
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Trigger {
    At(TimeRef),
    /// Fires once when `cond` has held for `hold_s`, but only after `from`.
    When {
        cond: Cond,
        hold_s: f64,
        from: Option<TimeRef>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub line: usize,
    pub trigger: Trigger,
    pub label: Option<String>,
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExpectKind {
    Always {
        from: Option<TimeRef>,
        until: Option<TimeRef>,
    },
    Never {
        from: Option<TimeRef>,
        until: Option<TimeRef>,
    },
    Eventually {
        from: Option<TimeRef>,
        by: Option<TimeRef>,
    },
    At(TimeRef),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expect {
    pub line: usize,
    pub source: String,
    pub kind: ExpectKind,
    pub cond: Cond,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scenario {
    pub name: String,
    pub machine: Option<String>,
    pub engine_args: Vec<String>,
    pub seed: u64,
    pub duration_s: f64,
    /// `model path = value` overrides applied before the run.
    pub model: Vec<(String, f64)>,
    pub trace: Vec<String>,
    pub trace_every_s: f64,
    pub rules: Vec<Rule>,
    pub expects: Vec<Expect>,
}
