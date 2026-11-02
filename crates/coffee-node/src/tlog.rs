//! Compact per-tick log of the hardware boundary (`.clog`).
//!
//! Only what crosses the boundary is stored: actuator bits, raw sensor readings,
//! switches and timing. Everything the engine derives (filtered temperature,
//! state, duties, ...) is reproducible by replaying these frames through the
//! deterministic engine, so it isn't logged.
//!
//! The stream is a sequence of self-contained chunks (default 1 s), so a serial
//! capture can start mid-stream and resynchronise on the magic bytes:
//!
//! ```text
//! chunk  := MAGIC(2) len:u16 baseline ticks:u16 tick*
//! baseline := half_cycle:u32 t_us:u64 dt:u32 tc:u32 p:i16 scale:i32 inputs:u8   (state "before" tick 0)
//! tick   := flags:u8 [dt Δ zz-varint] [p Δ zz-varint] [tc u32] [scale Δ zz-varint] [inputs u8]
//! flags  := b0 heater | b1 pump | b2 valve | b3 dt changed | b4 p changed
//!           | b5 tc changed | b6 scale changed | b7 inputs changed
//! ```
//!
//! A steady tick is 1-2 bytes, versus ~110 bytes as CSV.

use alloc::vec::Vec;
use coffee_io::SensorFrame;

pub const MAGIC: [u8; 2] = [0xC0, 0xF7];
const BASELINE_LEN: usize = 4 + 8 + 4 + 4 + 2 + 4 + 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outputs {
    pub heater: bool,
    pub pump: bool,
    pub valve: bool,
}

#[derive(Clone, Copy, Default)]
struct State {
    half_cycle: u32,
    t_us: u64,
    dt: u32,
    tc: u32,
    p: i16,
    scale: i32,
    inputs: u8,
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
    out: Vec<u8>,
    body: Vec<u8>,
    baseline: State,
    prev: State,
    ticks: u16,
}

impl LogWriter {
    pub fn new(ticks_per_chunk: u16) -> Self {
        Self {
            ticks_per_chunk: ticks_per_chunk.max(1),
            out: Vec::new(),
            body: Vec::new(),
            baseline: State::default(),
            prev: State::default(),
            ticks: 0,
        }
    }

    pub fn push(&mut self, s: &SensorFrame, o: Outputs) {
        if self.ticks == 0 {
            let b = State {
                half_cycle: s.half_cycle.wrapping_sub(1),
                t_us: s.t_us.saturating_sub(s.half_period_us as u64),
                dt: s.half_period_us,
                tc: s.max31855,
                p: s.pressure_adc,
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
        if s.half_period_us != p.dt {
            flags |= 1 << 3;
            put_varint(&mut self.body, zz(s.half_period_us as i64 - p.dt as i64));
        }
        if s.pressure_adc != p.p {
            flags |= 1 << 4;
            put_varint(&mut self.body, zz(s.pressure_adc as i64 - p.p as i64));
        }
        if s.max31855 != p.tc {
            flags |= 1 << 5;
            self.body.extend_from_slice(&s.max31855.to_le_bytes());
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
            half_cycle: s.half_cycle,
            t_us: s.t_us,
            dt: s.half_period_us,
            tc: s.max31855,
            p: s.pressure_adc,
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
        self.out
            .extend_from_slice(&(len.min(u16::MAX as usize) as u16).to_le_bytes());
        self.out.extend_from_slice(&b.half_cycle.to_le_bytes());
        self.out.extend_from_slice(&b.t_us.to_le_bytes());
        self.out.extend_from_slice(&b.dt.to_le_bytes());
        self.out.extend_from_slice(&b.tc.to_le_bytes());
        self.out.extend_from_slice(&b.p.to_le_bytes());
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
    while i + 4 <= data.len() {
        if data[i..i + 2] != MAGIC {
            i += 1;
            continue;
        }
        let len = u16::from_le_bytes([data[i + 2], data[i + 3]]) as usize;
        let start = i + 4;
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
    let mut st = State {
        half_cycle: u32::from_le_bytes(c.bytes()?),
        t_us: u64::from_le_bytes(c.bytes()?),
        dt: u32::from_le_bytes(c.bytes()?),
        tc: u32::from_le_bytes(c.bytes()?),
        p: i16::from_le_bytes(c.bytes()?),
        scale: i32::from_le_bytes(c.bytes()?),
        inputs: c.u8()?,
    };
    let n = u16::from_le_bytes(c.bytes()?);
    let mark = out.len();
    for _ in 0..n {
        let Some(f) = c.u8() else {
            out.truncate(mark);
            return None;
        };
        if f & (1 << 3) != 0 {
            st.dt = (st.dt as i64 + unzz(c.varint()?)) as u32;
        }
        if f & (1 << 4) != 0 {
            st.p = (st.p as i64 + unzz(c.varint()?)) as i16;
        }
        if f & (1 << 5) != 0 {
            st.tc = u32::from_le_bytes(c.bytes()?);
        }
        if f & (1 << 6) != 0 {
            st.scale = (st.scale as i64 + unzz(c.varint()?)) as i32;
        }
        if f & (1 << 7) != 0 {
            st.inputs = c.u8()?;
        }
        st.half_cycle = st.half_cycle.wrapping_add(1);
        st.t_us += st.dt as u64;
        out.push((
            SensorFrame {
                half_cycle: st.half_cycle,
                t_us: st.t_us,
                half_period_us: st.dt,
                max31855: st.tc,
                pressure_adc: st.p,
                scale_mg: st.scale,
                inputs: st.inputs,
            },
            Outputs {
                heater: f & 1 != 0,
                pump: f & 2 != 0,
                valve: f & 4 != 0,
            },
        ));
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_with_garbage_and_partial_chunks() {
        let mut w = LogWriter::new(100);
        let mut frames = Vec::new();
        for i in 0..1234u32 {
            let s = SensorFrame {
                half_cycle: i + 7,
                t_us: 5_000_000 + i as u64 * 10_000 + if i > 600 { 3 } else { 0 },
                half_period_us: if i == 601 { 10_003 } else { 10_000 },
                max31855: 0x1000_0000 + (i / 10) * 4,
                pressure_adc: 3300 + ((i * 37) % 200) as i16,
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
        let mut bytes = alloc::vec![0x55, 0xC0, 0x00];
        bytes.extend(w.finish());
        let got = read(&bytes);
        assert_eq!(got.len(), frames.len());
        assert!(got == frames);
        // ~2-3 bytes per tick for noisy pressure.
        assert!(bytes.len() < frames.len() * 4, "{} bytes", bytes.len());
    }
}
