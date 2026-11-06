//! Compact per-cycle log of the hardware boundary (`.clog`, version 4).
//!
//! Only what crosses the boundary is stored: actuator bits, raw sensor values
//! (thermocouple uV, cold junction, pressure mV and its in-cycle ripple), switches and timing. Everything the engine derives (temperature, state, duties,
//! ...) is reproducible by replaying these frames through the deterministic
//! engine, so it isn't logged.
//!
//! The stream is a sequence of self-contained chunks (default 1 s), so a serial
//! capture can start mid-stream and resynchronise on the magic bytes:
//!
//! ```text
//! chunk    := MAGIC(2) VERSION(1) len:u16 baseline ticks:u16 tick*
//! baseline := cycle:u32 t0_us:u64 period_ns:u32 tc_uv:i32 cj_cc:i16 p_mv:u16 p_pp:u16 scale:i32 inputs:u8
//!             (the state "before" tick 0; t0_us is the time of cycle 0; tc/cj the last samples)
//! tick     := flags:u8 [t Δ zz-varint] [aux] [p Δ zz-varint] [scale Δ zz-varint] [inputs u8]
//! aux      := have:u8 (b0 tc, b1 cj, b2 p_pp changed) [tc Δ] [cj Δ] [p_pp Δ] (zz-varints)
//! flags    := b0 heater | b1 pump | b2 valve | b3 t off-grid | b4 aux follows
//!             | b5 p changed | b6 scale changed | b7 inputs changed
//! ```
//!
//! The thermocouple ADC converts every ~16th tick, so most ticks carry no temperature: a
//! sample is logged on the tick it arrived (`Some` in the frame), as a delta from the last one.
//!
//! ```text
//! ```
//!
//! Ticks are crystal-timed, one per mains cycle: `cycle` just counts, and firmware
//! stamps each tick with its scheduled time `t0 + round(cycle * period_ns / 1000)`,
//! so `t_us` is predicted exactly. Only a tick that lands elsewhere stores its
//! offset, and moves `t0` so the following ticks are predicted from it.
//! A steady tick is 1-3 bytes, versus ~60 bytes as CSV.

use alloc::vec::Vec;
use coffee_io::SensorFrame;

pub const MAGIC: [u8; 2] = [0xC0, 0xF7];
pub const VERSION: u8 = 4;
const BASELINE_LEN: usize = 4 + 8 + 4 + 4 + 2 + 2 + 2 + 4 + 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outputs {
    pub heater: bool,
    pub pump: bool,
    pub valve: bool,
}

#[derive(Clone, Copy, Default)]
struct State {
    cycle: u32,
    t_us: u64,
    tc: i32,
    cj: i16,
    p: u16,
    pp: u16,
    scale: i32,
    inputs: u8,
}

/// Predicts tick times on the crystal grid.
#[derive(Clone, Copy)]
struct Clock {
    period_ns: u32,
    /// Time of cycle 0 (wrapping arithmetic: may be "before" boot).
    t0_us: u64,
}

impl Clock {
    fn offset(&self, cycle: u32) -> u64 {
        (cycle as u64 * self.period_ns as u64 + 500) / 1000
    }
    fn predict(&self, cycle: u32) -> u64 {
        self.t0_us.wrapping_add(self.offset(cycle))
    }
    /// Re-anchor so that `cycle` lands on `t_us`.
    fn anchor(&mut self, cycle: u32, t_us: u64) {
        self.t0_us = t_us.wrapping_sub(self.offset(cycle));
    }
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return;
        }
        out.push(b | 0x80);
    }
}

fn zz(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzz(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

struct Cur<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Cur<'_> {
    fn u8(&mut self) -> Option<u8> {
        let b = *self.d.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }
    fn bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.d.get(self.pos..self.pos + N)?;
        self.pos += N;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Some(a)
    }
    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= ((b & 0x7F) as u64) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
}

/// Builds chunks. Call [`LogWriter::push`] every tick and periodically drain
/// finished chunks with [`LogWriter::take`].
pub struct LogWriter {
    ticks_per_chunk: u16,
    period_ns: u32,
    out: Vec<u8>,
    body: Vec<u8>,
    baseline: State,
    prev: State,
    clock: Clock,
    ticks: u16,
}

impl LogWriter {
    /// `mains_hz` sets the nominal tick period used to predict timestamps.
    pub fn new(ticks_per_chunk: u16, mains_hz: f32) -> Self {
        let period_ns = (1e9 / mains_hz.max(1.0) as f64 + 0.5) as u32;
        Self {
            ticks_per_chunk: ticks_per_chunk.max(1),
            period_ns,
            out: Vec::new(),
            body: Vec::new(),
            baseline: State::default(),
            prev: State::default(),
            clock: Clock {
                period_ns,
                t0_us: 0,
            },
            ticks: 0,
        }
    }

    pub fn push(&mut self, s: &SensorFrame, o: Outputs) {
        if self.ticks == 0 {
            self.clock.anchor(s.cycle, s.t_us);
            let b = State {
                cycle: s.cycle.wrapping_sub(1),
                t_us: self.clock.t0_us,
                tc: s.tc_uv.unwrap_or(0),
                cj: s.cj_cc.unwrap_or(0),
                p: s.pressure_mv,
                pp: s.pressure_pp_mv,
                scale: s.scale_mg,
                inputs: s.inputs,
            };
            self.baseline = b;
            self.prev = b;
        }
        let p = self.prev;
        let mut flags = o.heater as u8 | (o.pump as u8) << 1 | (o.valve as u8) << 2;
        let flag_pos = self.body.len();
        self.body.push(0);
        // Cycles are consecutive within a chunk.
        let cycle = p.cycle.wrapping_add(1);
        let predicted = self.clock.predict(cycle);
        if s.t_us != predicted {
            flags |= 1 << 3;
            put_varint(&mut self.body, zz(s.t_us.wrapping_sub(predicted) as i64));
            self.clock.anchor(cycle, s.t_us);
        }
        let (mut tc, mut cj) = (p.tc, p.cj);
        let pp_changed = s.pressure_pp_mv != p.pp;
        if s.tc_uv.is_some() || s.cj_cc.is_some() || pp_changed {
            flags |= 1 << 4;
            self.body.push(s.tc_uv.is_some() as u8 | (s.cj_cc.is_some() as u8) << 1 | (pp_changed as u8) << 2);
            if let Some(v) = s.tc_uv {
                put_varint(&mut self.body, zz(v as i64 - p.tc as i64));
                tc = v;
            }
            if let Some(v) = s.cj_cc {
                put_varint(&mut self.body, zz(v as i64 - p.cj as i64));
                cj = v;
            }
            if pp_changed {
                put_varint(&mut self.body, zz(s.pressure_pp_mv as i64 - p.pp as i64));
            }
        }
        if s.pressure_mv != p.p {
            flags |= 1 << 5;
            put_varint(&mut self.body, zz(s.pressure_mv as i64 - p.p as i64));
        }
        if s.scale_mg != p.scale {
            flags |= 1 << 6;
            put_varint(&mut self.body, zz(s.scale_mg as i64 - p.scale as i64));
        }
        if s.inputs != p.inputs {
            flags |= 1 << 7;
            self.body.push(s.inputs);
        }
        self.body[flag_pos] = flags;
        self.prev = State {
            cycle: s.cycle,
            t_us: s.t_us,
            tc,
            cj,
            p: s.pressure_mv,
            pp: s.pressure_pp_mv,
            scale: s.scale_mg,
            inputs: s.inputs,
        };
        self.ticks += 1;
        if self.ticks >= self.ticks_per_chunk {
            self.seal();
        }
    }

    fn seal(&mut self) {
        if self.ticks == 0 {
            return;
        }
        let b = self.baseline;
        let len = BASELINE_LEN + 2 + self.body.len();
        self.out.extend_from_slice(&MAGIC);
        self.out.push(VERSION);
        self.out
            .extend_from_slice(&(len.min(u16::MAX as usize) as u16).to_le_bytes());
        self.out.extend_from_slice(&b.cycle.to_le_bytes());
        self.out.extend_from_slice(&b.t_us.to_le_bytes());
        self.out.extend_from_slice(&self.period_ns.to_le_bytes());
        self.out.extend_from_slice(&b.tc.to_le_bytes());
        self.out.extend_from_slice(&b.cj.to_le_bytes());
        self.out.extend_from_slice(&b.p.to_le_bytes());
        self.out.extend_from_slice(&b.pp.to_le_bytes());
        self.out.extend_from_slice(&b.scale.to_le_bytes());
        self.out.push(b.inputs);
        self.out.extend_from_slice(&self.ticks.to_le_bytes());
        self.out.extend_from_slice(&self.body);
        self.body.clear();
        self.ticks = 0;
    }

    /// Finished chunks so far.
    pub fn take(&mut self) -> Vec<u8> {
        core::mem::take(&mut self.out)
    }

    /// Seal the partial chunk and return everything.
    pub fn finish(&mut self) -> Vec<u8> {
        self.seal();
        self.take()
    }
}

/// Decodes a `.clog` byte stream, skipping garbage between chunks.
pub fn read(data: &[u8]) -> Vec<(SensorFrame, Outputs)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 5 <= data.len() {
        if data[i..i + 2] != MAGIC || data[i + 2] != VERSION {
            i += 1;
            continue;
        }
        let len = u16::from_le_bytes([data[i + 3], data[i + 4]]) as usize;
        let start = i + 5;
        let Some(chunk) = data.get(start..start + len) else {
            break;
        };
        match decode_chunk(chunk, &mut out) {
            Some(()) => i = start + len,
            None => i += 1,
        }
    }
    out
}

fn decode_chunk(d: &[u8], out: &mut Vec<(SensorFrame, Outputs)>) -> Option<()> {
    let mut c = Cur { d, pos: 0 };
    let cycle = u32::from_le_bytes(c.bytes()?);
    let t_us = u64::from_le_bytes(c.bytes()?);
    let period_ns = u32::from_le_bytes(c.bytes()?);
    let mut st = State {
        cycle,
        t_us,
        tc: i32::from_le_bytes(c.bytes()?),
        cj: i16::from_le_bytes(c.bytes()?),
        p: u16::from_le_bytes(c.bytes()?),
        pp: u16::from_le_bytes(c.bytes()?),
        scale: i32::from_le_bytes(c.bytes()?),
        inputs: c.u8()?,
    };
    let mut clock = Clock {
        period_ns,
        t0_us: t_us,
    };
    let n = u16::from_le_bytes(c.bytes()?);
    let mark = out.len();
    let mut tick = || -> Option<(SensorFrame, Outputs)> {
        let f = c.u8()?;
        let cycle = st.cycle.wrapping_add(1);
        let mut t = clock.predict(cycle);
        if f & (1 << 3) != 0 {
            t = t.wrapping_add(unzz(c.varint()?) as u64);
            clock.anchor(cycle, t);
        }
        let (mut tc_uv, mut cj_cc) = (None, None);
        if f & (1 << 4) != 0 {
            let have = c.u8()?;
            if have & 1 != 0 {
                st.tc = (st.tc as i64 + unzz(c.varint()?)) as i32;
                tc_uv = Some(st.tc);
            }
            if have & 2 != 0 {
                st.cj = (st.cj as i64 + unzz(c.varint()?)) as i16;
                cj_cc = Some(st.cj);
            }
            if have & 4 != 0 {
                st.pp = (st.pp as i64 + unzz(c.varint()?)) as u16;
            }
        }
        if f & (1 << 5) != 0 {
            st.p = (st.p as i64 + unzz(c.varint()?)) as u16;
        }
        if f & (1 << 6) != 0 {
            st.scale = (st.scale as i64 + unzz(c.varint()?)) as i32;
        }
        if f & (1 << 7) != 0 {
            st.inputs = c.u8()?;
        }
        st.cycle = cycle;
        st.t_us = t;
        Some((
            SensorFrame {
                cycle: st.cycle,
                t_us: st.t_us,
                tc_uv,
                cj_cc,
                pressure_mv: st.p,
                pressure_pp_mv: st.pp,
                scale_mg: st.scale,
                inputs: st.inputs,
            },
            Outputs {
                heater: f & 1 != 0,
                pump: f & 2 != 0,
                valve: f & 4 != 0,
            },
        ))
    };
    for _ in 0..n {
        match tick() {
            Some(x) => out.push(x),
            None => {
                out.truncate(mark);
                return None;
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_garbage_and_partial_chunks() {
        let mut w = LogWriter::new(60, 60.0);
        let mut frames = Vec::new();
        for i in 0..1234u32 {
            // Crystal grid at 16666.67 us, with a step from tick 601 on.
            let mut t_us = 5_000_000 + ((i as u64 + 7) * 50_000_000 + 1_500) / 3_000;
            if i >= 601 {
                t_us += 3;
            }
            let s = SensorFrame {
                cycle: i + 7,
                t_us,
                tc_uv: (i % 16 == 0).then_some(2_150 + (i / 10) as i32),
                cj_cc: (i % 16 == 3).then_some(3_500 - (i / 50) as i16),
                pressure_mv: 333 + ((i * 37) % 200) as u16,
                // ripple: sample noise, and the pump's strokes for a stretch
                pressure_pp_mv: (i * 13 % 9) as u16 + if (500..700).contains(&i) { 300 } else { 12 },
                scale_mg: (i as i32 / 10) * 100,
                inputs: if (300..400).contains(&i) { 1 } else { 0 },
            };
            let o = Outputs {
                heater: i % 3 == 0,
                pump: i % 2 == 0,
                valve: i > 300,
            };
            w.push(&s, o);
            frames.push((s, o));
        }
        let mut bytes = alloc::vec![0x55, 0xC0, 0x00, 0xC0, 0xF7, 0x01];
        bytes.extend(w.finish());
        let got = read(&bytes);
        assert_eq!(got.len(), frames.len());
        assert!(got == frames);
        // a few bytes per tick for noisy pressure and ripple, nothing for on-grid timing.
        assert!(bytes.len() < frames.len() * 6, "{} bytes", bytes.len());
    }

    #[test]
    fn steady_ticks_are_one_byte() {
        let mut w = LogWriter::new(60, 60.0);
        for i in 0..600u32 {
            let s = SensorFrame {
                cycle: i,
                t_us: (i as u64 * 16_666_667 + 500) / 1000, // on the 60 Hz grid
                pressure_mv: 340,
                ..Default::default()
            };
            w.push(&s, Outputs::default());
        }
        let bytes = w.finish();
        let overhead = 10 * (5 + BASELINE_LEN + 2);
        assert_eq!(bytes.len(), overhead + 600);
        assert_eq!(read(&bytes).len(), 600);
    }
}
