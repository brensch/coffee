//! Compact binary encoding for stored shots (`.cshot`).
//!
//! Metadata is a small JSON header; samples (10 Hz) are quantised and
//! delta + zig-zag varint encoded column by column:
//!
//! | column          | unit stored | quantum  |
//! |-----------------|-------------|----------|
//! | t               | 0.01 s      | 10 ms    |
//! | pressure, flow  | 0.01        |          |
//! | targets, weight | 0.01 / 0.1g | `i32::MIN` = none |
//! | temp            | 0.01 C      | `i32::MIN` = none |
//! | phase           | raw u8      |          |
//!
//! ```text
//! MAGIC "CSH1" | meta_len:u32 | meta JSON | phases JSON len:u32 | phases JSON | n:u32 | columns...
//! ```

use alloc::string::String;
use alloc::vec::Vec;
use coffee_api::{Shot, ShotMeta, ShotSample};

const MAGIC: &[u8; 4] = b"CSH1";
const NONE: i32 = i32::MIN;

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn zz(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzz(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

fn q(x: f32, scale: f32) -> i32 {
    let v = x * scale;
    if v.is_finite() {
        (if v >= 0.0 { v + 0.5 } else { v - 0.5 }) as i32
    } else {
        NONE
    }
}

fn qo(x: Option<f32>, scale: f32) -> i32 {
    x.map(|v| q(v, scale)).unwrap_or(NONE)
}

fn dq(v: i32, scale: f32) -> Option<f32> {
    (v != NONE).then(|| v as f32 / scale)
}

fn put_col(out: &mut Vec<u8>, vals: impl Iterator<Item = i32>) {
    let mut prev = 0i64;
    for v in vals {
        put_varint(out, zz(v as i64 - prev));
        prev = v as i64;
    }
}

struct Cur<'a> {
    d: &'a [u8],
    pos: usize,
}

impl Cur<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.d.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(s)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn varint(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *self.d.get(self.pos)?;
            self.pos += 1;
            v |= ((b & 0x7F) as u64) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    fn col(&mut self, n: usize) -> Option<Vec<i32>> {
        let mut prev = 0i64;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            prev += unzz(self.varint()?);
            v.push(prev as i32);
        }
        Some(v)
    }
}

pub fn encode(shot: &Shot) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    let meta = serde_json::to_vec(&shot.meta).unwrap_or_default();
    out.extend_from_slice(&(meta.len() as u32).to_le_bytes());
    out.extend_from_slice(&meta);
    let phases = serde_json::to_vec(&shot.phases).unwrap_or_default();
    out.extend_from_slice(&(phases.len() as u32).to_le_bytes());
    out.extend_from_slice(&phases);
    let s = &shot.samples;
    out.extend_from_slice(&(s.len() as u32).to_le_bytes());
    put_col(&mut out, s.iter().map(|x| q(x.t, 100.0)));
    put_col(&mut out, s.iter().map(|x| q(x.pressure, 100.0)));
    put_col(&mut out, s.iter().map(|x| qo(x.target_pressure, 100.0)));
    put_col(&mut out, s.iter().map(|x| q(x.flow, 100.0)));
    put_col(&mut out, s.iter().map(|x| qo(x.target_flow, 100.0)));
    put_col(&mut out, s.iter().map(|x| qo(x.weight, 10.0)));
    put_col(&mut out, s.iter().map(|x| qo(x.temp, 100.0)));
    put_col(&mut out, s.iter().map(|x| x.phase as i32));
    out
}

pub fn decode(d: &[u8]) -> Option<Shot> {
    if d.get(..4)? != MAGIC {
        return None;
    }
    let mut c = Cur { d, pos: 4 };
    let ml = c.u32()? as usize;
    let meta: ShotMeta = serde_json::from_slice(c.take(ml)?).ok()?;
    let pl = c.u32()? as usize;
    let phases: Vec<String> = serde_json::from_slice(c.take(pl)?).ok()?;
    let n = c.u32()? as usize;
    let t = c.col(n)?;
    let p = c.col(n)?;
    let tp = c.col(n)?;
    let f = c.col(n)?;
    let tf = c.col(n)?;
    let w = c.col(n)?;
    let temp = c.col(n)?;
    let ph = c.col(n)?;
    let samples = (0..n)
        .map(|i| ShotSample {
            t: t[i] as f32 / 100.0,
            pressure: p[i] as f32 / 100.0,
            target_pressure: dq(tp[i], 100.0),
            flow: f[i] as f32 / 100.0,
            target_flow: dq(tf[i], 100.0),
            weight: dq(w[i], 10.0),
            temp: dq(temp[i], 100.0),
            phase: ph[i] as u8,
        })
        .collect();
    Some(Shot {
        meta,
        phases,
        samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn roundtrip_and_size() {
        let samples: Vec<ShotSample> = (0..300)
            .map(|i| ShotSample {
                t: i as f32 * 0.1,
                pressure: 9.0 + (i % 7) as f32 * 0.03,
                target_pressure: if i < 50 { None } else { Some(9.0) },
                flow: 2.0 + (i % 5) as f32 * 0.02,
                target_flow: None,
                weight: if i < 80 {
                    None
                } else {
                    Some((i - 80) as f32 * 0.17)
                },
                temp: Some(93.25),
                phase: (i / 80) as u8,
            })
            .collect();
        let shot = Shot {
            meta: ShotMeta {
                id: 3,
                started_t: 12.5,
                profile: "Classic 9 bar".into(),
                duration_s: 30.0,
                weight_g: Some(36.1),
                pumped_ml: 60.0,
                end: "target_weight".to_string(),
                mean_resistance: Some(4.4),
                peak_pressure_bar: 9.2,
                advice: None,
            },
            phases: alloc::vec!["fill".into(), "extract".into()],
            samples,
        };
        let b = encode(&shot);
        let back = decode(&b).unwrap();
        assert_eq!(back.meta, shot.meta);
        assert_eq!(back.samples.len(), 300);
        for (a, b) in shot.samples.iter().zip(&back.samples) {
            assert!((a.pressure - b.pressure).abs() < 0.006);
            assert_eq!(a.target_pressure.is_some(), b.target_pressure.is_some());
            assert_eq!(a.phase, b.phase);
        }
        let json = serde_json::to_vec(&shot).unwrap();
        assert!(
            b.len() * 8 < json.len(),
            "binary {} vs json {}",
            b.len(),
            json.len()
        );
    }
}
