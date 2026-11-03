//! ESP32-C6 firmware (ESP32-C6-MINI-1U-H4: 4 MB flash, 105 C rated).
//!
//! This is the ESP32 shell around `coffee_node::Node`, the same node the PC binary
//! runs. All behaviour (engine, profiles, shot recording, API) lives there; the
//! pure timing / filtering pieces of this shell live in `coffee_board` and are
//! unit-tested on the host.
//!
//! Default build: drives the real machine. Ticks come from the crystal, once per
//! nominal mains cycle (`EngineConfig::mains_hz`); there is no zero-cross detector,
//! because the MOC3063 opto-triacs only ever fire their triacs at a zero crossing.
//! During each cycle both analog channels are sampled `SAMPLES` times, evenly
//! spread over the cycle, with ADC1 curve calibration (eFuse) giving millivolts.
//! At the tick that ends cycle `k` the averages and the debounced switches form a
//! `SensorFrame`, the node is ticked, and its `ActuatorFrame` is applied at once,
//! i.e. for cycle `k + 1`. Each tick is also streamed over USB as `.clog`
//! (`coffee-emu capture`), so `coffee-emu fit` can calibrate the model.
//!
//! `--features bridge`: hardware-in-the-loop. Sensor frames arrive from the
//! emulator over USB (lockstep wire protocol) instead of from the pins, so the
//! real chip runs the real engine against the emulated machine. Machine outputs
//! are held off.
//!
//! Pin map (controller v6):
//!
//! | Signal          | GPIO  | Notes                                                 |
//! |-----------------|-------|-------------------------------------------------------|
//! | Pressure        | 0     | ADC1_CH0, transducer through 10k/20k (x2/3)           |
//! | Boiler NTC      | 1     | ADC1_CH1, 100k B3950 to GND, 8.2k 0.1 % to 3.3 V       |
//! | Valve           | 18    | push-pull, HIGH = on, MOC3063 LED via 150 R           |
//! | Pump            | 19    | push-pull, HIGH = on, MOC3063 LED via 150 R           |
//! | Heater          | 20    | push-pull, HIGH = on, MOC3063 LED via 150 R           |
//! | Brew switch     | 21    | dry contact to GND, internal pull-up, low = closed    |
//! | Steam switch    | 22    | dry contact to GND, internal pull-up, low = closed    |
//! | Status LED      | 23    | heartbeat heating, solid ready, fast blink fault      |
//! | USB D- / D+     | 12/13 | native USB Serial/JTAG (flashing, log, bridge)        |
//!
//! Strapping pins 4, 5, 8, 9 and 15 are unused (GPIO8 has its external pull-up,
//! GPIO9 is the BOOT pad).

#![no_std]
#![cfg_attr(
    feature = "bridge",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]
#![no_main]

extern crate alloc;

use alloc::collections::VecDeque;
use coffee_api::Info;
use coffee_board::{CycleClock, Debounce, Oversample, status_led};
use coffee_io::{SensorFrame, input};
use coffee_node::tlog::{LogWriter, Outputs};
use coffee_node::{MemStorage, Node, NodeOptions};
use esp_backtrace as _;
use esp_hal::analog::adc::{Adc, AdcCalCurve, AdcConfig, Attenuation};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::peripherals::ADC1;
use esp_hal::rtc_cntl::{Rtc, RwdtStage};
use esp_hal::time::{Duration, Instant};
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;

esp_bootloader_esp_idf::esp_app_desc!();

/// ADC samples per channel per mains cycle (each one-shot conversion takes
/// ~50 us on the C6, so 2 x 32 use ~3.2 ms of the 16.7 ms cycle).
const SAMPLES: u32 = 32;
/// Switch levels must hold this many cycles to count (50 ms at 60 Hz).
const DEBOUNCE_CYCLES: u8 = 3;
/// The watchdog resets the chip (all outputs drop to off) if the loop stalls.
const WATCHDOG_MS: u64 = 250;
/// USB log backlog. When no host drains it, whole chunks are dropped: the
/// control loop never blocks on USB.
const TX_QUEUE_BYTES: usize = 8 * 1024;

fn now_us() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

/// Non-blocking USB transmit queue.
struct Tx {
    q: VecDeque<u8>,
}

impl Tx {
    /// Queue a whole chunk, or drop it if it doesn't fit.
    fn push(&mut self, chunk: &[u8]) {
        if self.q.len() + chunk.len() <= TX_QUEUE_BYTES {
            self.q.extend(chunk);
        }
    }
    /// Move as much as the USB FIFO takes right now.
    fn pump(&mut self, usb: &mut UsbSerialJtag<'_, esp_hal::Blocking>) {
        let mut sent = false;
        while let Some(&b) = self.q.front() {
            if usb.write_byte_nb(b).is_err() {
                break;
            }
            self.q.pop_front();
            sent = true;
        }
        if sent {
            let _ = usb.flush_tx_nb();
        }
    }
}

#[esp_hal::main]
fn main() -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 160 * 1024);

    let mut rtc = Rtc::new(p.RTC_TIMER);
    rtc.rwdt
        .set_timeout(RwdtStage::Stage0, Duration::from_millis(WATCHDOG_MS));
    rtc.rwdt.enable();

    let mut usb = UsbSerialJtag::new(p.USB_DEVICE);

    // Outputs first, driven low: every load off until the engine says otherwise.
    let mut valve = Output::new(p.GPIO18, Level::Low, OutputConfig::default());
    let mut pump = Output::new(p.GPIO19, Level::Low, OutputConfig::default());
    let mut heater = Output::new(p.GPIO20, Level::Low, OutputConfig::default());
    let mut led = Output::new(p.GPIO23, Level::Low, OutputConfig::default());
    let brew_sw = Input::new(p.GPIO21, InputConfig::default().with_pull(Pull::Up));
    let steam_sw = Input::new(p.GPIO22, InputConfig::default().with_pull(Pull::Up));

    // ADC1 at 12 dB (the `_11dB` setting): the 0-3.3 V range, curve-calibrated
    // from eFuse to millivolts.
    let mut adc_cfg = AdcConfig::new();
    let mut ch_pressure =
        adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.GPIO0, Attenuation::_11dB);
    let mut ch_ntc =
        adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.GPIO1, Attenuation::_11dB);
    let mut adc = Adc::new(p.ADC1, adc_cfg);

    // Storage is RAM for now; flash-backed storage (esp-storage) slots in here.
    let mut node = Node::new(
        MemStorage::default(),
        NodeOptions {
            info: Info {
                name: "coffee".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                platform: "esp32c6".into(),
                emulated: cfg!(feature = "bridge"),
                lab_url: None,
            },
            max_shots: 20, // compressed, ~2.6 KB each
            persist_shots: false,
            history_capacity: 1200, // 4 min at 5 Hz, 29 KB
            history_hz: 5.0,
        },
    );

    #[cfg(feature = "bridge")]
    {
        let _ = (&mut valve, &mut pump, &mut heater, &brew_sw, &steam_sw);
        let _ = (&mut adc, &mut ch_ntc, &mut ch_pressure);
        bridge::run(&mut usb, &mut node, &mut rtc, &mut led);
    }

    #[cfg(not(feature = "bridge"))]
    {
        let mains_hz = node.engine.cfg.mains_hz;
        let mut log = LogWriter::new(mains_hz as u16, mains_hz);
        let mut tx = Tx {
            q: VecDeque::with_capacity(TX_QUEUE_BYTES),
        };
        tx.push(
            alloc::format!(
                "# coffee firmware {} mains {mains_hz} Hz\n",
                env!("CARGO_PKG_VERSION")
            )
            .as_bytes(),
        );

        let clock = CycleClock::new(now_us(), mains_hz);
        let mut brew = Debounce::new(DEBOUNCE_CYCLES);
        let mut steam = Debounce::new(DEBOUNCE_CYCLES);
        let (mut ntc_acc, mut p_acc) = (Oversample::default(), Oversample::default());
        let (mut ntc_mv, mut pressure_mv) = (0u16, 0u16);
        let mut k: u32 = 0;

        loop {
            // ---- one mains cycle of sampling: [tick k, tick k+1) ----
            for i in 0..SAMPLES {
                let at = clock.sample_us(k, i, SAMPLES);
                while now_us() < at {
                    tx.pump(&mut usb);
                }
                if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_pressure)) {
                    p_acc.push(mv);
                }
                if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_ntc)) {
                    ntc_acc.push(mv);
                }
            }
            k = k.wrapping_add(1);
            let t = clock.tick_us(k);
            while now_us() < t {
                tx.pump(&mut usb);
            }

            // ---- tick k: frame for the cycle that just ended ----
            ntc_mv = ntc_acc.take(ntc_mv);
            pressure_mv = p_acc.take(pressure_mv);
            let mut inputs = 0u8;
            if brew.update(brew_sw.is_low()) {
                inputs |= input::BREW;
            }
            if steam.update(steam_sw.is_low()) {
                inputs |= input::STEAM;
            }
            let s = SensorFrame {
                cycle: k,
                t_us: t,
                ntc_mv,
                pressure_mv,
                scale_mg: 0,
                inputs,
            };
            let a = node.tick(&s);
            // Applied now, for cycle k+1: each opto-triac fires at its next zero
            // crossing, so a load raised here conducts for one whole cycle.
            heater.set_level(a.heater.into());
            pump.set_level(a.pump.into());
            valve.set_level(a.valve.into());
            led.set_level(status_led(a.status.state, a.status.faults, t / 1000).into());
            rtc.rwdt.feed();

            // No Wi-Fi yet: events have nowhere to go, so drop them.
            let _ = node.drain_events();
            log.push(
                &s,
                Outputs {
                    heater: a.heater,
                    pump: a.pump,
                    valve: a.valve,
                },
            );
            let chunk = log.take();
            if !chunk.is_empty() {
                tx.push(&chunk);
            }
        }
    }
}

#[cfg(feature = "bridge")]
mod bridge {
    use super::*;
    use coffee_io::wire::{self, MAX_FRAME, Message, PROTOCOL_VERSION};

    /// Read one byte, feeding the watchdog while the host is quiet.
    fn read_byte(usb: &mut UsbSerialJtag<'_, esp_hal::Blocking>, rtc: &mut Rtc<'_>) -> u8 {
        loop {
            match usb.read_byte() {
                Ok(b) => return b,
                Err(_) => rtc.rwdt.feed(),
            }
        }
    }

    pub fn run(
        usb: &mut UsbSerialJtag<'_, esp_hal::Blocking>,
        node: &mut Node<MemStorage>,
        rtc: &mut Rtc<'_>,
        led: &mut Output<'_>,
    ) -> ! {
        let mut buf = [0u8; MAX_FRAME];
        let mut out = [0u8; MAX_FRAME];
        loop {
            for b in buf[..3].iter_mut() {
                *b = read_byte(usb, rtc);
            }
            let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
            if len > MAX_FRAME - 3 {
                continue; // resync: drop garbage
            }
            for b in buf[3..3 + len].iter_mut() {
                *b = read_byte(usb, rtc);
            }
            let reply = match wire::decode(&buf[..3 + len]) {
                Ok((Message::Hello { mains_hz, .. }, _)) => {
                    node.reset_session(mains_hz as f32);
                    Some(Message::Hello {
                        version: PROTOCOL_VERSION,
                        mains_hz,
                    })
                }
                Ok((Message::Sensors(s), _)) => {
                    let a = node.tick(&s);
                    let _ = node.drain_events();
                    led.set_level(
                        status_led(a.status.state, a.status.faults, s.t_us / 1000).into(),
                    );
                    Some(Message::Actuators(a))
                }
                _ => None,
            };
            if let Some(m) = reply {
                let n = wire::encode(&m, &mut out);
                let _ = usb.write(&out[..n]);
                let _ = usb.flush_tx();
            }
        }
    }
}
