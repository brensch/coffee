//! ESP32-C6 firmware.
//!
//! This is the ESP32 shell around `coffee_node::Node`, the same node the PC
//! binary runs. All behaviour (engine, profiles, shot recording, API) lives there.
//!
//! Default build: drives the real machine. Every mains zero crossing it samples
//! the sensors into a `SensorFrame`, ticks the node, and applies the
//! `ActuatorFrame` to the SSR / triac / solenoid pins. Each tick is also streamed
//! over USB as the binary `.clog` format (`coffee-emu capture`), so
//! `coffee-emu fit` can calibrate the model from real hardware.
//!
//! `--features bridge`: hardware-in-the-loop. Sensor frames arrive from the
//! emulator over USB (lockstep wire protocol) instead of from the pins, so the
//! real chip runs the real engine against the emulated machine. Machine outputs
//! are held off.
//!
//! Pin map (ESP32-C6-DevKitC-1, avoiding strapping/USB/flash pins):
//!
//! | Signal                    | GPIO | Notes                                  |
//! |---------------------------|------|----------------------------------------|
//! | Zero-cross in             | 4    | dimmer module Z-C output, pulse/crossing|
//! | Heater SSR                | 5    | zero-cross SSR input, active high      |
//! | Pump triac gate           | 18   | dimmer PWM input, active high          |
//! | 3-way solenoid            | 19   | SSR/relay input, active high           |
//! | Brew switch in            | 20   | to GND through optocoupler, pull-up    |
//! | Steam switch in           | 21   | to GND through optocoupler, pull-up    |
//! | MAX31855 SCK / MISO / CS  | 6 / 2 / 7 |                                   |
//! | ADS1115 SDA / SCL         | 22 / 23 | address 0x48, AIN0 = pressure       |

#![no_std]
#![cfg_attr(feature = "bridge", allow(dead_code, unused_imports))]
#![no_main]

extern crate alloc;

use coffee_api::Info;
use coffee_io::{SensorFrame, input};
use coffee_node::tlog::{LogWriter, Outputs};
use coffee_node::{MemStorage, Node, NodeOptions};
use core::fmt::Write as _;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::spi::Mode;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::{Instant, Rate};
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;

esp_bootloader_esp_idf::esp_app_desc!();

const ADS1115_ADDR: u8 = 0x48;
/// Continuous conversion, AIN0 vs GND, +/-4.096 V, 860 SPS, comparator off.
const ADS1115_CONFIG: [u8; 3] = [0x01, 0x42, 0xE3];
/// Zero-cross edges closer than this are treated as noise.
const ZC_DEBOUNCE_US: u64 = 6_000;
/// No zero crossing for this long => mains lost or detector broken: all off.
const ZC_TIMEOUT_US: u64 = 50_000;
/// Apply outputs this long before the predicted next zero crossing, so
/// zero-cross SSRs (which can only turn on near 0 V) switch exactly at it.
/// Calibrate against the zero-cross module's pulse offset with a scope.
const OUTPUT_LEAD_US: u64 = 1_500;

fn now_us() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

#[esp_hal::main]
fn main() -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 160 * 1024);

    let mut usb = UsbSerialJtag::new(p.USB_DEVICE);

    let mut heater = Output::new(p.GPIO5, Level::Low, OutputConfig::default());
    let mut pump = Output::new(p.GPIO18, Level::Low, OutputConfig::default());
    let mut valve = Output::new(p.GPIO19, Level::Low, OutputConfig::default());
    let zc = Input::new(p.GPIO4, InputConfig::default().with_pull(Pull::Down));
    let brew_sw = Input::new(p.GPIO20, InputConfig::default().with_pull(Pull::Up));
    let steam_sw = Input::new(p.GPIO21, InputConfig::default().with_pull(Pull::Up));

    let mut tc_cs = Output::new(p.GPIO7, Level::High, OutputConfig::default());
    let mut spi = Spi::new(
        p.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(4))
            .with_mode(Mode::_0),
    )
    .expect("spi")
    .with_sck(p.GPIO6)
    .with_miso(p.GPIO2);
    let mut i2c = I2c::new(
        p.I2C0,
        I2cConfig::default().with_frequency(Rate::from_khz(400)),
    )
    .expect("i2c")
    .with_sda(p.GPIO22)
    .with_scl(p.GPIO23);
    let ads_ok = i2c.write(ADS1115_ADDR, &ADS1115_CONFIG).is_ok();

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
    let _ = writeln!(
        Usb(&mut usb),
        "# coffee firmware {} ads1115={}",
        env!("CARGO_PKG_VERSION"),
        ads_ok
    );

    #[cfg(feature = "bridge")]
    {
        let _ = (&zc, &brew_sw, &steam_sw, &mut tc_cs, &mut spi, &mut i2c);
        heater.set_low();
        pump.set_low();
        valve.set_low();
        bridge::run(&mut usb, &mut node);
    }

    #[cfg(not(feature = "bridge"))]
    {
        let mut log = LogWriter::new(100);
        // Outputs decided at crossing k, applied just before crossing k+1.
        let mut pending: Option<(u64, bool, bool, bool)> = None;
        let mut half_cycle: u32 = 0;
        let mut last_zc = now_us();
        let mut zc_prev = zc.is_high();
        let mut tc_word: u32 = 0;
        let mut last_tc_read = 0u64;
        let mut pressure_adc: i16 = 0;

        loop {
            let t = now_us();

            // Mains watchdog: no zero crossings => everything off.
            if t - last_zc > ZC_TIMEOUT_US {
                heater.set_low();
                pump.set_low();
                valve.set_low();
            }

            if let Some((at, h, p, v)) = pending
                && t >= at {
                    heater.set_level(h.into());
                    pump.set_level(p.into());
                    valve.set_level(v.into());
                    pending = None;
                }

            // Rising edge on the zero-cross detector = start of a half-cycle.
            let level = zc.is_high();
            let edge = level && !zc_prev;
            zc_prev = level;
            if !edge || t - last_zc < ZC_DEBOUNCE_US {
                continue;
            }
            let half_period = (t - last_zc) as u32;
            last_zc = t;

            // Sensors. MAX31855 converts every ~100 ms; read at 10 Hz.
            if t - last_tc_read >= 100_000 {
                last_tc_read = t;
                let mut b = [0u8; 4];
                tc_cs.set_low();
                let ok = spi.read(&mut b).is_ok();
                tc_cs.set_high();
                tc_word = if ok { u32::from_be_bytes(b) } else { 0 };
            }
            let mut buf = [0u8; 2];
            if i2c.write_read(ADS1115_ADDR, &[0x00], &mut buf).is_ok() {
                pressure_adc = i16::from_be_bytes(buf);
            }
            let mut inputs = 0u8;
            if brew_sw.is_low() {
                inputs |= input::BREW;
            }
            if steam_sw.is_low() {
                inputs |= input::STEAM;
            }

            let s = SensorFrame {
                half_cycle,
                t_us: t,
                half_period_us: half_period,
                max31855: tc_word,
                pressure_adc,
                scale_mg: 0,
                inputs,
            };
            let a = node.tick(&s);
            // No Wi-Fi yet: events have nowhere to go, so drop them.
            let _ = node.drain_events();
            let apply_at = t + (half_period as u64).saturating_sub(OUTPUT_LEAD_US);
            pending = Some((apply_at, a.heater, a.pump, a.valve));
            half_cycle = half_cycle.wrapping_add(1);

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
                let _ = usb.write(&chunk);
            }
        }
    }
}

struct Usb<'a, 'd>(&'a mut UsbSerialJtag<'d, esp_hal::Blocking>);
impl core::fmt::Write for Usb<'_, '_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.write(s.as_bytes()).map_err(|_| core::fmt::Error)
    }
}

#[cfg(feature = "bridge")]
mod bridge {
    use super::*;
    use coffee_io::wire::{self, MAX_FRAME, Message, PROTOCOL_VERSION};

    fn read_exact(usb: &mut UsbSerialJtag<'_, esp_hal::Blocking>, buf: &mut [u8]) {
        for b in buf.iter_mut() {
            *b = nb::block!(usb.read_byte()).unwrap_or(0);
        }
    }

    pub fn run(usb: &mut UsbSerialJtag<'_, esp_hal::Blocking>, node: &mut Node<MemStorage>) -> ! {
        let mut buf = [0u8; MAX_FRAME];
        let mut out = [0u8; MAX_FRAME];
        loop {
            read_exact(usb, &mut buf[..3]);
            let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
            if len > MAX_FRAME - 3 {
                continue; // resync: drop garbage
            }
            read_exact(usb, &mut buf[3..3 + len]);
            let reply = match wire::decode(&buf[..3 + len]) {
                Ok((Message::Hello { .. }, _)) => {
                    node.reset_session();
                    Some(Message::Hello {
                        version: PROTOCOL_VERSION,
                    })
                }
                Ok((Message::Sensors(s), _)) => {
                    let a = node.tick(&s);
                    let _ = node.drain_events();
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
