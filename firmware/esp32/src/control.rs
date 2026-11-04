//! The control loop: runs on the high-priority interrupt executor and owns the node,
//! so Wi-Fi, HTTP and USB can never make it wait. It awaits only its own timers
//! (the sample points and the tick).
//!
//! Ticks come from the crystal, once per nominal mains cycle
//! (`EngineConfig::mains_hz`). During each cycle both analog channels are sampled
//! `SAMPLES` times, evenly spread over the cycle (averaging out the pump's ripple
//! exactly), with ADC1 curve calibration (eFuse) giving millivolts. Both rails are
//! sampled too (3.3 V through 1/2, 5 V through 1/3), and the thermistor and
//! pressure readings are rescaled to the nominal rails: the NTC divider is
//! ratiometric to 3.3 V and the transducer to 5 V, so this removes rail and ADC
//! gain error and makes an open thermistor read exactly full scale. At the tick
//! that ends cycle `k` the averages and the debounced switches form a
//! `SensorFrame`, the node is ticked, and its `ActuatorFrame` is applied at once,
//! for cycle `k + 1`: each zero-cross SSR switches at its next zero crossing.
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
use crate::store::{BUSY, NodeT};
use crate::usb;
use coffee_api::ServerMsg;
use coffee_board::{CycleClock, Debounce, Oversample, ratiometric, status_led};
use coffee_io::{EngineState, SensorFrame, input};
use coffee_node::tlog::{LogWriter, Outputs};
use core::sync::atomic::Ordering;
use embassy_time::{Instant, Timer};
use esp_hal::analog::adc::{Adc, AdcCalCurve, AdcConfig, Attenuation};
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::peripherals::{self, ADC1};
use esp_hal::rtc_cntl::{Rtc, RwdtStage};
use esp_hal::time::Duration;

/// ADC samples per channel per mains cycle (each one-shot conversion takes
/// ~50 us on the C6, so 2 x 32 use ~3.2 ms of the 16.7 ms cycle).
const SAMPLES: u32 = 32;
/// Switch levels must hold this many cycles to count (50 ms at 60 Hz).
const DEBOUNCE_CYCLES: u8 = 3;
/// The watchdog resets the chip (all outputs drop to off) if the loop stalls.
const WATCHDOG_MS: u64 = 250;

pub struct Pins {
    pub rtc: peripherals::RTC_TIMER<'static>,
    pub adc1: ADC1<'static>,
    pub pressure: peripherals::GPIO0<'static>,
    pub ntc: peripherals::GPIO1<'static>,
    pub v3_sense: peripherals::GPIO2<'static>,
    pub v5_sense: peripherals::GPIO3<'static>,
    pub valve: peripherals::GPIO18<'static>,
    pub pump: peripherals::GPIO19<'static>,
    pub heater: peripherals::GPIO20<'static>,
    pub brew: peripherals::GPIO6<'static>,
    pub steam: peripherals::GPIO7<'static>,
    pub led: peripherals::GPIO23<'static>,
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

    let mut rtc = Rtc::new(p.rtc);
    rtc.rwdt.set_timeout(RwdtStage::Stage0, Duration::from_millis(WATCHDOG_MS));
    rtc.rwdt.enable();

    // ADC1 at 12 dB (the `_11dB` setting): the 0-3.3 V range, curve-calibrated
    // from eFuse to millivolts.
    let mut adc_cfg = AdcConfig::new();
    let mut ch_pressure = adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.pressure, Attenuation::_11dB);
    let mut ch_ntc = adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.ntc, Attenuation::_11dB);
    let mut ch_v3 = adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.v3_sense, Attenuation::_11dB);
    let mut ch_v5 = adc_cfg.enable_pin_with_cal::<_, AdcCalCurve<ADC1>>(p.v5_sense, Attenuation::_11dB);
    let mut adc = Adc::new(p.adc1, adc_cfg);

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
    let (mut ntc_acc, mut p_acc) = (Oversample::default(), Oversample::default());
    let (mut v3_acc, mut v5_acc) = (Oversample::default(), Oversample::default());
    let (mut ntc_mv, mut pressure_mv) = (0u16, 0u16);
    let (mut v3_mv, mut v5_mv) = (0u16, 0u16);
    let mut k: u32 = 0;
    // Cycles in a row with no thermistor sample (ADC read errors): past STALE_CYCLES the reading is
    // reported open, so a stuck ADC can't freeze the temperature near setpoint.
    const STALE_CYCLES: u32 = 30;
    let mut ntc_stale: u32 = 0;

    loop {
        // ---- one mains cycle of sampling: [tick k, tick k+1) ----
        let mut ntc_fresh = false;
        for i in 0..SAMPLES {
            until(clock.sample_us(k, i, SAMPLES)).await;
            if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_pressure)) {
                p_acc.push(mv);
            }
            if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_ntc)) {
                ntc_acc.push(mv);
                ntc_fresh = true;
            }
            // The rails move slowly: one sample of each per 4 slots is plenty.
            if i % 4 == 0 {
                if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_v3)) {
                    v3_acc.push(mv);
                }
                if let Ok(mv) = nb::block!(adc.read_oneshot(&mut ch_v5)) {
                    v5_acc.push(mv);
                }
            }
        }
        k = k.wrapping_add(1);
        let t = clock.tick_us(k);
        until(t).await;

        // ---- tick k: frame for the cycle that just ended ----
        v3_mv = v3_acc.take(v3_mv);
        v5_mv = v5_acc.take(v5_mv);
        // Open probe: judged on the raw reading as well as the rail-corrected one, so a rail sense
        // that reads high can't scale an open probe (node at the rail) down below the threshold.
        let ntc_raw = ntc_acc.take(ntc_mv);
        ntc_stale = if ntc_fresh { 0 } else { ntc_stale.saturating_add(1) };
        let open_mv = node.engine.cfg.ntc.open_mv as u16;
        ntc_mv = if ntc_raw >= open_mv || ntc_stale > STALE_CYCLES {
            3300
        } else {
            ratiometric(ntc_raw, v3_mv, 2, 3300)
        };
        pressure_mv = ratiometric(p_acc.take(pressure_mv), v5_mv, 3, 5000);
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
        // Applied now, for cycle k+1: each zero-cross SSR switches at its next zero
        // crossing, so a load raised here conducts for one whole cycle.
        heater.set_level(a.heater.into());
        pump.set_level(a.pump.into());
        valve.set_level(a.valve.into());
        led.set_level(status_led(a.status.state, a.status.faults, t / 1000).into());
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
