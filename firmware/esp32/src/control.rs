//! The control loop: runs on the high-priority interrupt executor and owns the node,
//! so Wi-Fi, HTTP and USB can never make it wait. It awaits only its own timers
//! (the sample points and the tick).
//!
//! Ticks come from the crystal, once per nominal mains cycle
//! (`EngineConfig::mains_hz`). During each cycle the pressure channel is sampled
//! `SAMPLES` times, evenly spread over the cycle (averaging out the pump's ripple
//! exactly), with ADC1 curve calibration (eFuse) giving millivolts; the 5 V rail is
//! sampled too (through 1/3) and the reading rescaled to the nominal rail, since the
//! transducer is ratiometric to 5 V. The thermocouple is on I2C (GPIO8 SDA, GPIO9 SCL):
//! the MCP3421 converts continuously at 3.75 SPS and is polled at every tick; when it
//! has a fresh conversion, the TMP117 (cold junction) is read too, and both go in that
//! tick's frame. At the tick that ends cycle `k` the readings and the debounced
//! switches form a `SensorFrame`, the node is ticked, and its `ActuatorFrame` is
//! applied at once, for cycle `k + 1`: each zero-cross opto-triac or SSR switches at
//! its next zero crossing.
//!
//! Safety:
//! - outputs are driven low at start;
//! - API requests are handled *after* the cycle's outputs are applied, at most
//!   `link::MAX_PER_CYCLE` per cycle and only while within `link::BUDGET_US`;
//!   the handler (`Node::prepare_http`) only copies data out, and the JSON
//!   encoding happens in the requesting task;
//! - the RTC watchdog (250 ms) is fed once per completed tick, so a stalled loop
//!   resets the chip, and every pin drops to off.

use crate::link::{self, BUDGET_US, LINK, MAX_PER_CYCLE, TELEMETRY};
use crate::store::{BUSY, NodeT, OTA_ARMED};
use crate::usb;
use coffee_api::ServerMsg;
use coffee_board::tc::{self, Mcp3421};
use coffee_board::{CycleClock, Debounce, Oversample, ratiometric, status_led};
use coffee_io::{EngineState, SensorFrame, input};
use coffee_node::tlog::{LogWriter, Outputs};
use core::sync::atomic::Ordering;
use embassy_time::{Instant, Timer};
use esp_hal::analog::adc::{Adc, AdcCalCurve, AdcConfig, Attenuation};
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::peripherals::{self, ADC1};
use esp_hal::rtc_cntl::{Rtc, RwdtStage};
use esp_hal::time::{Duration, Rate};

/// ADC samples per mains cycle (each one-shot conversion takes ~50 us on the
/// C6, so 32 use ~1.6 ms of the 16.7 ms cycle).
const SAMPLES: u32 = 32;
/// Switch levels must hold this many cycles to count (50 ms at 60 Hz).
const DEBOUNCE_CYCLES: u8 = 3;
/// The watchdog resets the chip (all outputs drop to off) if the loop stalls.
const WATCHDOG_MS: u64 = 250;

pub struct Pins {
    pub rtc: peripherals::RTC_TIMER<'static>,
    pub adc1: ADC1<'static>,
    pub pressure: peripherals::GPIO0<'static>,
    pub v5_sense: peripherals::GPIO3<'static>,
    /// Thermocouple ADC (MCP3421) and cold junction (TMP117), 4.7k pull-ups to 3.3 V.
    pub i2c: peripherals::I2C0<'static>,
    pub sda: peripherals::GPIO8<'static>,
    pub scl: peripherals::GPIO9<'static>,
    pub valve: peripherals::GPIO18<'static>,
    pub pump: peripherals::GPIO19<'static>,
    pub heater: peripherals::GPIO20<'static>,
    pub brew: peripherals::GPIO6<'static>,
    pub steam: peripherals::GPIO7<'static>,
    /// Status LED (v24: on U0TXD, so it also flickers with the ROM boot log)
    pub led: peripherals::GPIO16<'static>,
    pub power: peripherals::GPIO14<'static>,
    pub led_power: peripherals::GPIO21<'static>,
    pub led_coffee: peripherals::GPIO22<'static>,
    pub led_steam: peripherals::GPIO23<'static>,
    pub scale_sck: peripherals::GPIO17<'static>,
    pub scale_dout1: peripherals::GPIO4<'static>,
    pub scale_dout2: peripherals::GPIO5<'static>,
}

fn now_us() -> u64 {
    Instant::now().as_micros()
}

async fn until(t_us: u64) {
    Timer::at(Instant::from_micros(t_us)).await
}

#[embassy_executor::task]
pub async fn run(p: Pins, mut node: NodeT) {
    // Outputs first, driven low: every load off until the engine says otherwise.
    let mut valve = Output::new(p.valve, Level::Low, OutputConfig::default());
    let mut pump = Output::new(p.pump, Level::Low, OutputConfig::default());
    let mut heater = Output::new(p.heater, Level::Low, OutputConfig::default());
    let mut led = Output::new(p.led, Level::Low, OutputConfig::default());
    let brew_sw = Input::new(p.brew, InputConfig::default().with_pull(Pull::Up));
    let steam_sw = Input::new(p.steam, InputConfig::default().with_pull(Pull::Up));
    let power_sw = Input::new(p.power, InputConfig::default().with_pull(Pull::Up));
    let mut panel = [
        Output::new(p.led_power, Level::Low, OutputConfig::default()),
        Output::new(p.led_coffee, Level::Low, OutputConfig::default()),
        Output::new(p.led_steam, Level::Low, OutputConfig::default()),
    ];
    let mut scale = crate::scale::Scale::new(
        Output::new(p.scale_sck, Level::Low, OutputConfig::default()),
        // pulled up: with no scale on J10 the lines read "not ready", not noise (review S5)
        Input::new(p.scale_dout1, InputConfig::default().with_pull(Pull::Up)),
        Input::new(p.scale_dout2, InputConfig::default().with_pull(Pull::Up)),
    );

    let mut rtc = Rtc::new(p.rtc);
    rtc.rwdt.set_timeout(RwdtStage::Stage0, Duration::from_millis(WATCHDOG_MS));
    rtc.rwdt.enable();

    // ADC1 at 12 dB (the `_11dB` setting): the 0-3.3 V range, curve-calibrated
    // from eFuse to millivolts.
    let mut adc_cfg = AdcConfig::new();
    let mut ch_pressure = adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.pressure, Attenuation::_11dB);
    let mut ch_v5 = adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.v5_sense, Attenuation::_11dB);
    let mut adc = Adc::new(p.adc1, adc_cfg);

    // I2C at 100 kHz: a 4-byte MCP3421 poll is ~0.5 ms, well inside the tick's budget, and the
    // slow edges suit the short bus beside the mains section. A stuck bus returns an error (the
    // driver's timeout), which reads as "no conversion": the engine's TC_ADC fault after 1 s.
    let mut i2c = I2c::new(p.i2c, I2cConfig::default().with_frequency(Rate::from_khz(100)))
        .unwrap()
        .with_sda(p.sda)
        .with_scl(p.scl);
    let mut thermo = Thermo::default();

    let mains_hz = node.engine.cfg.mains_hz;
    TELEMETRY.publish(node.telemetry().clone());
    let mut log = LogWriter::new(mains_hz as u16, mains_hz);
    usb::event(&alloc::format!(
        r#"{{"event":"boot","version":"{}","mains_hz":{mains_hz}}}"#,
        crate::VERSION
    ));

    let clock = CycleClock::new(now_us(), mains_hz);
    let mut brew = Debounce::new(DEBOUNCE_CYCLES);
    let mut steam = Debounce::new(DEBOUNCE_CYCLES);
    let mut power = Debounce::new(DEBOUNCE_CYCLES);
    let (mut p_acc, mut v5_acc) = (Oversample::default(), Oversample::default());
    // Last raw (uncorrected) readings: the fallback when a cycle has no samples, so the rail
    // correction is applied once per reading and never compounds.
    let (mut p_raw, mut v5_mv) = (0u16, 0u16);
    let mut k: u64 = 0;   // u64: a u32 cycle count wraps after ~828 days at 60 Hz
    // Cycles in a row with no pressure sample (ADC read errors): past STALE_CYCLES the reading
    // goes to 0 V, the transducer-disconnected fault.
    const STALE_CYCLES: u32 = 30;
    let mut p_stale: u32 = 0;

    loop {
        // ---- one mains cycle of sampling: [tick k, tick k+1) ----
        let mut p_fresh = false;
        for i in 0..SAMPLES {
            until(clock.sample_us(k, i, SAMPLES)).await;
            if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_pressure)) {
                p_acc.push(mv);
                p_fresh = true;
            }
            // The rail moves slowly: one sample per 4 slots is plenty.
            if i % 4 == 0 {
                if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_v5)) {
                    v5_acc.push(mv);
                }
            }
        }
        k = k.wrapping_add(1);
        let t = clock.tick_us(k);
        until(t).await;

        // ---- tick k: frame for the cycle that just ended ----
        v5_mv = v5_acc.take(v5_mv);
        p_raw = p_acc.take(p_raw);
        p_stale = if p_fresh { 0 } else { p_stale.saturating_add(1) };
        let (tc_uv, cj_cc) = thermo.poll(&mut i2c, k);
        // Pressure: no samples for STALE_CYCLES reads as 0 V, the transducer-disconnected fault.
        // (+5 V is 4.4-5.25 V between USB and the module, so its sense keeps ratiometric's 15 %.)
        let pressure_mv = if p_stale > STALE_CYCLES { 0 } else { ratiometric(p_raw, v5_mv, 3, 5000) };
        let mut inputs = 0u8;
        if brew.update(brew_sw.is_low()) {
            inputs |= input::BREW;
        }
        if steam.update(steam_sw.is_low()) {
            inputs |= input::STEAM;
        }
        // Power rocker open (pulled up) = standby; the controller itself stays powered.
        if !power.update(power_sw.is_low()) {
            inputs |= input::STANDBY;
        }
        OTA_ARMED.store(inputs & input::STANDBY != 0 && inputs & input::STEAM != 0, Ordering::Relaxed);
        // In standby SCK is held high (the HX711s power down after 60 us): with the front lead out of
        // J7 a mis-plugged lead may face SCK, and a static high is the state that stays harmless even
        // with a bare LED there (docs/SWAP-SAFETY.md). The last reading is kept, not counted stale.
        if inputs & input::STANDBY == 0 {
            scale.wake();
            scale.poll();
        } else {
            scale.sleep();
        }
        let (scale_mg, scale_ok) = scale.read_mg(&node.engine.cfg.scale);
        if scale_ok {
            inputs |= input::SCALE_OK;
        }
        let s = SensorFrame {
            cycle: k as u32,
            t_us: t,
            tc_uv,
            cj_cc,
            pressure_mv,
            scale_mg,
            inputs,
        };

        let a = node.tick(&s);
        // Applied now, for cycle k+1: each zero-cross SSR switches at its next zero
        // crossing, so a load raised here conducts for one whole cycle.
        heater.set_level(a.heater.into());
        pump.set_level(a.pump.into());
        valve.set_level(a.valve.into());
        led.set_level(status_led(a.status.state, a.status.faults, t / 1000).into());
        for (o, on) in panel.iter_mut().zip(a.lamps) {   // the engine's lamp outputs, as telemetry reports them
            o.set_level(on.into());
        }
        rtc.rwdt.feed();
        BUSY.store(a.status.state == EngineState::Brewing, Ordering::Relaxed);

        for e in node.drain_events() {
            if let ServerMsg::Telemetry(tm) = &e {
                TELEMETRY.publish(tm.clone());
            }
            crate::net::publish(e);
        }
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
            usb::push(&chunk);
        }

        // ---- API requests, after the outputs: bounded count and time ----
        let start = now_us();
        LINK.serve(
            MAX_PER_CYCLE,
            || now_us().saturating_sub(start) < BUDGET_US,
            |q: link::HttpReq| node.prepare_http(&q.method, &q.path, &q.body),
        );
    }
}

/// The thermocouple chips' driver state: polled once per tick, never blocking beyond one
/// short I2C transaction each.
#[derive(Default)]
struct Thermo {
    /// The MCP3421 has our config (continuous, 18 bit, PGA 8) as far as we know.
    adc_configured: bool,
    /// The TMP117 answered with its device ID.
    cj_present: bool,
}

impl Thermo {
    /// Re-probe a chip that's missing or misconfigured this often (ticks): ~1 s.
    const RETRY_TICKS: u64 = 64;

    /// One tick's poll: the conversion (if a fresh one landed) and, with it, the cold junction.
    /// Every error is a `None`: the engine times the faults (TC_ADC, COLD_JUNCTION), so a chip
    /// that stops answering can never freeze the temperature.
    fn poll(&mut self, i2c: &mut I2c<'_, esp_hal::Blocking>, k: u64) -> (Option<i32>, Option<i16>) {
        if !self.adc_configured {
            if k % Self::RETRY_TICKS != 0 {
                return (None, None);
            }
            self.adc_configured = i2c.write(tc::MCP3421_ADDR, &[tc::MCP3421_CONFIG]).is_ok();
            return (None, None);
        }
        let mut b = [0u8; 4];
        let uv = match i2c.read(tc::MCP3421_ADDR, &mut b).map(|_| tc::mcp3421_decode(b)) {
            Ok(Mcp3421::Fresh(uv)) => uv,
            Ok(Mcp3421::Stale) => return (None, None),
            Ok(Mcp3421::Misconfigured) | Err(_) => {
                self.adc_configured = false;   // rewrite the config on the next retry slot
                return (None, None);
            }
        };
        (Some(uv), self.cold_junction(i2c))
    }

    fn cold_junction(&mut self, i2c: &mut I2c<'_, esp_hal::Blocking>) -> Option<i16> {
        let mut b = [0u8; 2];
        if !self.cj_present {
            // At power-on the TMP117 runs continuous conversions by itself (1 s cycle, 8 averages):
            // nothing to configure, only check it's really there.
            self.cj_present = i2c.write_read(tc::TMP117_ADDR, &[tc::TMP117_REG_ID], &mut b).is_ok() && tc::tmp117_id_ok(b);
            if !self.cj_present {
                return None;
            }
        }
        match i2c.write_read(tc::TMP117_ADDR, &[tc::TMP117_REG_TEMP], &mut b) {
            Ok(()) => tc::tmp117_centi(b),
            Err(_) => {
                self.cj_present = false;
                None
            }
        }
    }
}
