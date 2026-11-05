//! Tiny length-prefixed binary protocol used between the engine and the emulator
//! (over stdio pipes or TCP), and available to firmware for a USB "hardware bridge".
//!
//! Frame: `u16 LE payload length | u8 kind | payload`. All integers little-endian,
//! floats IEEE-754 LE. The protocol is lockstep: the emulator sends `Sensors`, the
//! engine answers with exactly one `Actuators` carrying the same `cycle`.

use crate::{ActuatorFrame, EngineState, EngineStatus, SensorFrame};

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_FRAME: usize = 3 + 96;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Message {
    /// Opens a session. Sent by the machine side (emulator, or the PC end of a
    /// hardware bridge) with the mains frequency its frames are ticked at; the
    /// engine side answers with its own Hello.
    Hello {
        version: u16,
        mains_hz: u8,
    },
    Sensors(SensorFrame),
    Actuators(ActuatorFrame),
    Bye,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    /// Not enough bytes yet for a whole frame.
    Incomplete,
    UnknownKind(u8),
    BadLength,
}

const K_HELLO: u8 = 1;
const K_SENSORS: u8 = 2;
const K_ACTUATORS: u8 = 3;
const K_BYE: u8 = 4;

struct W<'a> {
    buf: &'a mut [u8],
    pos: usize,
}
impl W<'_> {
    fn put(&mut self, b: &[u8]) {
        self.buf[self.pos..self.pos + b.len()].copy_from_slice(b);
        self.pos += b.len();
    }
    fn u8(&mut self, v: u8) {
        self.put(&[v]);
    }
    fn u16(&mut self, v: u16) {
        self.put(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.put(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.put(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.put(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.put(&v.to_le_bytes());
    }
}

struct R<'a> {
    buf: &'a [u8],
    pos: usize,
}
impl R<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        let s = self
            .buf
            .get(self.pos..self.pos + N)
            .ok_or(WireError::BadLength)?;
        self.pos += N;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn i32(&mut self) -> Result<i32, WireError> {
        Ok(i32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Result<u64, WireError> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_le_bytes(self.take()?))
    }
}

/// Encode into `out` (must be at least [`MAX_FRAME`] bytes). Returns bytes written.
pub fn encode(msg: &Message, out: &mut [u8]) -> usize {
    let mut w = W { buf: out, pos: 3 };
    let kind = match msg {
        Message::Hello { version, mains_hz } => {
            w.u16(*version);
            w.u8(*mains_hz);
            K_HELLO
        }
        Message::Sensors(s) => {
            w.u32(s.cycle);
            w.u64(s.t_us);
            w.u16(s.ntc_mv);
            w.u16(s.pressure_mv);
            w.i32(s.scale_mg);
            w.u8(s.inputs);
            K_SENSORS
        }
        Message::Actuators(a) => {
            w.u32(a.cycle);
            w.u8((a.heater as u8) | (a.pump as u8) << 1 | (a.valve as u8) << 2
                | (a.lamps[0] as u8) << 3 | (a.lamps[1] as u8) << 4 | (a.lamps[2] as u8) << 5);
            let s = &a.status;
            w.u8(s.state as u8);
            w.u16(s.faults);
            w.u8(s.phase);
            for v in [
                s.temp_c,
                s.setpoint_c,
                s.pressure_bar,
                s.target_pressure_bar,
                s.flow_ml_s,
                s.target_flow_ml_s,
                s.weight_g,
                s.shot_time_s,
                s.heater_duty,
                s.pump_duty,
                s.boost_c,
                s.heat_soak,
                s.water_c,
                s.group_c,
            ] {
                w.f32(v);
            }
            K_ACTUATORS
        }
        Message::Bye => K_BYE,
    };
    let end = w.pos;
    let len = end - 3;
    out[0..2].copy_from_slice(&(len as u16).to_le_bytes());
    out[2] = kind;
    end
}

/// Try to decode one frame from the front of `buf`. Returns the message and the
/// number of bytes consumed.
pub fn decode(buf: &[u8]) -> Result<(Message, usize), WireError> {
    if buf.len() < 3 {
        return Err(WireError::Incomplete);
    }
    let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
    if len > MAX_FRAME - 3 {
        return Err(WireError::BadLength);
    }
    if buf.len() < 3 + len {
        return Err(WireError::Incomplete);
    }
    let mut r = R {
        buf: &buf[3..3 + len],
        pos: 0,
    };
    let msg = match buf[2] {
        K_HELLO => Message::Hello {
            version: r.u16()?,
            mains_hz: r.u8()?,
        },
        K_SENSORS => Message::Sensors(SensorFrame {
            cycle: r.u32()?,
            t_us: r.u64()?,
            ntc_mv: r.u16()?,
            pressure_mv: r.u16()?,
            scale_mg: r.i32()?,
            inputs: r.u8()?,
        }),
        K_ACTUATORS => {
            let cycle = r.u32()?;
            let flags = r.u8()?;
            let state = EngineState::from_u8(r.u8()?);
            let faults = r.u16()?;
            let phase = r.u8()?;
            let mut f = [0f32; 14];
            for v in f.iter_mut() {
                *v = r.f32()?;
            }
            Message::Actuators(ActuatorFrame {
                cycle,
                heater: flags & 1 != 0,
                pump: flags & 2 != 0,
                valve: flags & 4 != 0,
                lamps: [flags & 8 != 0, flags & 16 != 0, flags & 32 != 0],
                status: EngineStatus {
                    state,
                    faults,
                    phase,
                    temp_c: f[0],
                    setpoint_c: f[1],
                    pressure_bar: f[2],
                    target_pressure_bar: f[3],
                    flow_ml_s: f[4],
                    target_flow_ml_s: f[5],
                    weight_g: f[6],
                    shot_time_s: f[7],
                    heater_duty: f[8],
                    pump_duty: f[9],
                    boost_c: f[10],
                    heat_soak: f[11],
                    water_c: f[12],
                    group_c: f[13],
                },
            })
        }
        K_BYE => Message::Bye,
        k => return Err(WireError::UnknownKind(k)),
    };
    if r.pos != len {
        return Err(WireError::BadLength);
    }
    Ok((msg, 3 + len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all() {
        let msgs = [
            Message::Hello {
                version: PROTOCOL_VERSION,
                mains_hz: 60,
            },
            Message::Sensors(SensorFrame {
                cycle: 7,
                t_us: 123_456_789,
                ntc_mv: 1650,
                pressure_mv: 3001,
                scale_mg: 36_100,
                inputs: 0b1001,
            }),
            Message::Actuators(ActuatorFrame {
                cycle: 7,
                heater: true,
                pump: false,
                valve: true,
                lamps: [true, false, true],
                status: EngineStatus {
                    state: EngineState::Brewing,
                    faults: 3,
                    phase: 2,
                    temp_c: 93.5,
                    target_flow_ml_s: f32::NAN,
                    ..Default::default()
                },
            }),
            Message::Bye,
        ];
        let mut buf = [0u8; MAX_FRAME];
        for m in msgs {
            let n = encode(&m, &mut buf);
            let (d, used) = decode(&buf[..n]).unwrap();
            assert_eq!(used, n);
            match (m, d) {
                (Message::Actuators(a), Message::Actuators(b)) => {
                    assert_eq!(a.status.temp_c, b.status.temp_c);
                    assert!(b.status.target_flow_ml_s.is_nan());
                    assert_eq!(a.valve, b.valve);
                    assert_eq!(a.lamps, b.lamps);
                }
                (a, b) => assert_eq!(a, b),
            }
            assert_eq!(decode(&buf[..n - 1]), Err(WireError::Incomplete));
        }
    }
}
