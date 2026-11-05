//! Drip-tray scale: two load cells on two HX711s sharing one clock (SCK on RXD0/GPIO17) with a data
//! line each (DOUT1 on GPIO4, DOUT2 on GPIO5): the layout Gaggiuino and GaggiMate use, with the
//! HX711s on a small board next to the cells so only these slow digital lines reach the controller.
//!
//! HX711 (Avia datasheet): a reading is ready when DOUT goes low; 24 clock pulses shift the
//! two's-complement result out MSB first, and a 25th selects channel A at gain 128 for the next
//! conversion. Holding SCK high for more than 60 us powers the chip down, so each reading is
//! clocked inside a critical section (25 pulses at ~2 us each: ~50 us).

use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, Output};

/// Cycles without a fresh reading from either chip before the scale counts as disconnected.
/// The modules run at 10 or 80 samples/s; at 60 Hz mains, 30 cycles is half a second.
const STALE_CYCLES: u32 = 30;

pub struct Scale<'a> {
    sck: Output<'a>,
    dout: [Input<'a>; 2],
    delay: Delay,
    raw: [i32; 2],
    stale: u32,
}

impl<'a> Scale<'a> {
    pub fn new(sck: Output<'a>, dout1: Input<'a>, dout2: Input<'a>) -> Self {
        Self { sck, dout: [dout1, dout2], delay: Delay::new(), raw: [0, 0], stale: STALE_CYCLES + 1 }
    }

    /// Standby: SCK held high, which powers the HX711s down.
    pub fn sleep(&mut self) {
        self.sck.set_high();
    }

    /// Leave standby: SCK low powers the HX711s back up (first conversion after ~400 ms).
    pub fn wake(&mut self) {
        if self.sck.is_set_high() {
            self.sck.set_low();
        }
    }

    /// Call once per mains cycle. Reads both chips when both have a conversion ready.
    pub fn poll(&mut self) {
        if self.dout[0].is_high() || self.dout[1].is_high() {
            self.stale = self.stale.saturating_add(1);
            return;
        }
        let mut v = [0u32; 2];
        critical_section::with(|_| {
            for _ in 0..24 {
                self.sck.set_high();
                self.delay.delay_micros(1);
                for (i, d) in self.dout.iter().enumerate() {
                    v[i] = (v[i] << 1) | d.is_high() as u32;
                }
                self.sck.set_low();
                self.delay.delay_micros(1);
            }
            // 25th pulse: channel A, gain 128 for the next conversion.
            self.sck.set_high();
            self.delay.delay_micros(1);
            self.sck.set_low();
        });
        for i in 0..2 {
            // sign-extend 24-bit two's complement
            self.raw[i] = ((v[i] << 8) as i32) >> 8;
        }
        self.stale = 0;
    }

    /// `(milligrams, ok)`: the sum of both cells, calibrated; `ok` is false while the HX711s
    /// aren't answering (unplugged, or not fitted).
    pub fn read_mg(&self, cfg: &coffee_core::ScaleConfig) -> (i32, bool) {
        let mg: f32 = (0..2).map(|i| (self.raw[i] - cfg.zero[i]) as f32 * cfg.mg_per_count[i]).sum();
        (mg as i32, self.stale <= STALE_CYCLES)
    }
}
