//! Lightweight SVG line charts, rendered straight from Rust signals.

use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Axis {
    Left,
    Right,
}

#[derive(Clone)]
pub struct Line {
    pub color: &'static str,
    pub dashed: bool,
    pub axis: Axis,
    pub data: Signal<Vec<(f64, f64)>>,
}

impl Line {
    pub fn new(color: &'static str, axis: Axis, data: Signal<Vec<(f64, f64)>>) -> Self {
        Self {
            color,
            dashed: false,
            axis,
            data,
        }
    }
    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }
}

/// A shaded x-range with a label (profile phases).
#[derive(Clone, PartialEq)]
pub struct Band {
    pub x0: f64,
    pub x1: f64,
    pub label: String,
}

const PAD_L: f64 = 38.0;
const PAD_T: f64 = 10.0;
const PAD_B: f64 = 22.0;

fn nice_step(range: f64, target_ticks: f64) -> f64 {
    let raw = (range / target_ticks).max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    let n = raw / mag;
    let s = if n < 1.5 {
        1.0
    } else if n < 3.0 {
        2.0
    } else if n < 7.0 {
        5.0
    } else {
        10.0
    };
    s * mag
}

fn fmt_tick(v: f64, step: f64) -> String {
    if step >= 1.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

/// Build an SVG path, breaking the line at NaNs.
fn path(data: &[(f64, f64)], sx: impl Fn(f64) -> f64, sy: impl Fn(f64) -> f64) -> String {
    let mut d = String::with_capacity(data.len() * 14);
    let mut pen_down = false;
    for &(x, y) in data {
        if !x.is_finite() || !y.is_finite() {
            pen_down = false;
            continue;
        }
        let (px, py) = (sx(x), sy(y));
        d.push(if pen_down { 'L' } else { 'M' });
        d.push_str(&format!("{px:.1},{py:.1}"));
        pen_down = true;
    }
    d
}

#[component]
pub fn Chart(
    lines: Vec<Line>,
    #[prop(into)] x: Signal<(f64, f64)>,
    y_left: (f64, f64),
    #[prop(optional)] y_right: Option<(f64, f64)>,
    #[prop(default = 280.0)] height: f64,
    #[prop(optional, into)] bands: Option<Signal<Vec<Band>>>,
    #[prop(default = "s")] x_unit: &'static str,
    /// viewBox width: use ~400 for charts in narrow cards so labels stay legible.
    #[prop(default = 800.0)]
    width: f64,
) -> impl IntoView {
    #[allow(non_snake_case)]
    let W = width;
    let pad_r = if y_right.is_some() { 38.0 } else { 12.0 };
    let plot_w = W - PAD_L - pad_r;
    let plot_h = height - PAD_T - PAD_B;
    let sy_l = move |v: f64| PAD_T + plot_h * (1.0 - (v - y_left.0) / (y_left.1 - y_left.0));
    let yr = y_right.unwrap_or(y_left);
    let sy_r = move |v: f64| PAD_T + plot_h * (1.0 - (v - yr.0) / (yr.1 - yr.0));

    let step_l = nice_step(y_left.1 - y_left.0, 5.0);
    let mut yticks = vec![];
    let mut v = (y_left.0 / step_l).ceil() * step_l;
    while v <= y_left.1 + 1e-9 {
        yticks.push(v);
        v += step_l;
    }
    let step_r = nice_step(yr.1 - yr.0, 5.0);
    let rticks: Vec<f64> = if y_right.is_some() {
        let n = yticks.len().max(2) - 1;
        (0..=n)
            .map(|i| yr.0 + (yr.1 - yr.0) * (yticks[i] - y_left.0) / (y_left.1 - y_left.0))
            .collect()
    } else {
        vec![]
    };

    let grid = yticks
        .iter()
        .map(|&t| {
            let y = sy_l(t);
            view! {
                <line class="gridline" x1=PAD_L x2=W - pad_r y1=y y2=y />
                <text class="axis" x=PAD_L - 6.0 y=y + 3.0 text-anchor="end">{fmt_tick(t, step_l)}</text>
            }
        })
        .collect_view();
    let rgrid = rticks
        .iter()
        .map(|&t| {
            let y = sy_r(t);
            view! { <text class="axis" x=W - pad_r + 6.0 y=y + 3.0>{fmt_tick(t, step_r)}</text> }
        })
        .collect_view();

    let sx = move |xv: f64, (x0, x1): (f64, f64)| PAD_L + plot_w * (xv - x0) / (x1 - x0).max(1e-9);

    let xticks = move || {
        let (x0, x1) = x.get();
        let step = nice_step(x1 - x0, 8.0);
        let mut out = vec![];
        let mut v = (x0 / step).ceil() * step;
        while v <= x1 + 1e-9 {
            out.push(v);
            v += step;
        }
        out.into_iter()
            .map(|t| {
                let px = sx(t, (x0, x1));
                view! {
                    <line class="gridline" x1=px x2=px y1=PAD_T y2=PAD_T + plot_h opacity="0.5" />
                    <text class="axis" x=px y=height - 6.0 text-anchor="middle">{format!("{}{}", fmt_tick(t, step), x_unit)}</text>
                }
            })
            .collect_view()
    };

    let band_view = move || {
        let Some(b) = bands else { return vec![] };
        let (x0, x1) = x.get();
        b.get()
            .into_iter()
            .enumerate()
            .filter(|(_, b)| b.x1 > x0 && b.x0 < x1)
            .map(|(i, b)| {
                let a = sx(b.x0.max(x0), (x0, x1));
                let z = sx(b.x1.min(x1), (x0, x1));
                let fill = if i % 2 == 0 { "var(--accent)" } else { "var(--pressure)" };
                view! {
                    <rect class="phase-band" x=a y=PAD_T width=(z - a).max(0.0) height=plot_h fill=fill />
                    <text class="phase-label" x=a + 4.0 y=PAD_T + 12.0>{b.label.clone()}</text>
                }
                .into_any()
            })
            .collect::<Vec<_>>()
    };

    let paths = lines
        .into_iter()
        .map(|l| {
            let d = move || {
                let r = x.get();
                l.data.with(|data| {
                    if l.axis == Axis::Left {
                        path(data, |v| sx(v, r), sy_l)
                    } else {
                        path(data, |v| sx(v, r), sy_r)
                    }
                })
            };
            let class = if l.dashed { "line target" } else { "line" };
            view! { <path class=class d=d stroke=l.color /> }
        })
        .collect_view();

    view! {
        <svg class="chart" viewBox=format!("0 0 {W} {height}") preserveAspectRatio="none">
            <defs>
                <clipPath id="plot-clip">
                    <rect x=PAD_L y=PAD_T width=plot_w height=plot_h />
                </clipPath>
            </defs>
            {band_view}
            {grid}
            {rgrid}
            {xticks}
            <g clip-path="url(#plot-clip)">{paths}</g>
        </svg>
    }
}
