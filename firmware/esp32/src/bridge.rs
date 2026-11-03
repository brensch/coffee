//! Bridge mode (`--features bridge`): hardware-in-the-loop.
//!
//! Sensor frames arrive from `coffee-emu` over USB (lockstep wire protocol) and
//! actuator frames go back, so the real chip runs the real engine against the
//! emulated machine. The machine outputs are driven low and stay low; there is
//! no Wi-Fi and storage is RAM. The watchdog is fed while the host is quiet.

use coffee_board::status_led;
use coffee_io::wire::{self, MAX_FRAME, Message, PROTOCOL_VERSION};
use coffee_node::{MemStorage, Node, NodeOptions};
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::rtc_cntl::{Rtc, RwdtStage};
use esp_hal::time::Duration;
use esp_hal::usb::usb_serial_jtag::UsbSerialJtag;

const WATCHDOG_MS: u64 = 250;

#[esp_hal::main]
fn main() -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    esp_alloc::heap_allocator!(size: 160 * 1024);

    let mut rtc = Rtc::new(p.RTC_TIMER);
    rtc.rwdt.set_timeout(RwdtStage::Stage0, Duration::from_millis(WATCHDOG_MS));
    rtc.rwdt.enable();

    // Machine outputs held off for the whole session.
    let _valve = Output::new(p.GPIO18, Level::Low, OutputConfig::default());
    let _pump = Output::new(p.GPIO19, Level::Low, OutputConfig::default());
    let _heater = Output::new(p.GPIO20, Level::Low, OutputConfig::default());
    let mut led = Output::new(p.GPIO23, Level::Low, OutputConfig::default());
    let mut usb = UsbSerialJtag::new(p.USB_DEVICE);

    let mut node = Node::new(
        MemStorage::default(),
        NodeOptions {
            info: crate::info(),
            max_shots: 20,
            persist_shots: false,
            history_capacity: 1200,
            history_hz: 5.0,
        },
    );

    let mut buf = [0u8; MAX_FRAME];
    let mut out = [0u8; MAX_FRAME];
    loop {
        for b in buf[..3].iter_mut() {
            *b = read_byte(&mut usb, &mut rtc);
        }
        let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        if len > MAX_FRAME - 3 {
            continue; // resync: drop garbage
        }
        for b in buf[3..3 + len].iter_mut() {
            *b = read_byte(&mut usb, &mut rtc);
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
                led.set_level(status_led(a.status.state, a.status.faults, s.t_us / 1000).into());
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

/// Read one byte, feeding the watchdog while the host is quiet.
fn read_byte(usb: &mut UsbSerialJtag<'_, esp_hal::Blocking>, rtc: &mut Rtc<'_>) -> u8 {
    loop {
        match usb.read_byte() {
            Ok(b) => return b,
            Err(_) => rtc.rwdt.feed(),
        }
    }
}
