//! Shared state: the flash key/value store and network status. (The node itself is
//! owned by the control task; see `link.rs` for how the network side reaches it.)
//!
//! - The key/value store (`KV`) sits on the `storage` partition. Saves go to RAM
//!   and `flusher` writes them to flash a little later, and only while no shot is
//!   running: an erase stalls the CPU for tens of milliseconds.
//! - Wi-Fi credentials are a key in the same store (`wifi.json`).
//! - OTA writes go straight to the next app partition, via the store's flash.

use alloc::string::String;
use coffee_board::KvStorage;
use coffee_board::kv::Kv;
use coffee_node::Node;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};
use critical_section::Mutex as CsMutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use embedded_storage::nor_flash::NorFlash;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_bootloader_esp_idf::partitions::{self, DataPartitionSubType, PartitionType};
use esp_storage::FlashStorage;
use serde::{Deserialize, Serialize};
use static_cell::StaticCell;

pub type Flash = FlashStorage<'static>;
pub type KvCell = CsMutex<RefCell<Kv<Flash>>>;
pub type NodeT = Node<KvStorage<'static, Flash>>;

static KV: StaticCell<KvCell> = StaticCell::new();
static KV_REF: CsMutex<RefCell<Option<&'static KvCell>>> = CsMutex::new(RefCell::new(None));

/// A shot is running: flash writes wait.
pub static BUSY: AtomicBool = AtomicBool::new(false);
/// OTA physical-presence gate: set by the control loop (coffee_board::OtaGate) only after the steam
/// rocker and then the brew rocker have been held on for 3 s with the pump and valve idle (in steam
/// mode the brew rocker does nothing, so normal use never arms it). Nobody on the network can flip a
/// rocker, so a LAN client can't replace the firmware.
pub static OTA_ARMED: AtomicBool = AtomicBool::new(false);
/// Wi-Fi credentials changed (console `wifi set` / `wifi clear`).
pub static WIFI_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

const KEY_WIFI: &str = "wifi.json";

#[derive(Serialize, Deserialize, Clone)]
pub struct WifiCreds {
    pub ssid: String,
    pub password: String,
}

/// Live network state for `status`.
#[derive(Clone, Default, Serialize)]
pub struct NetState {
    pub configured: bool,
    pub ssid: Option<String>,
    pub connected: bool,
    pub ip: Option<String>,
    pub rssi: Option<i32>,
    pub last_error: Option<String>,
}

pub static NET: CsMutex<RefCell<Option<NetState>>> = CsMutex::new(RefCell::new(None));

pub fn net_state() -> NetState {
    critical_section::with(|cs| NET.borrow_ref(cs).clone().unwrap_or_default())
}

pub fn update_net(f: impl FnOnce(&mut NetState)) {
    critical_section::with(|cs| {
        let mut n = NET.borrow_ref_mut(cs);
        f(n.get_or_insert_with(NetState::default));
    })
}

/// Open the key/value store on the `storage` partition. A missing partition table
/// means the board was flashed without `partitions.csv`: that panics with a clear
/// message over USB rather than running without storage. A corrupted store is
/// erased (settings lost, machine still works).
pub fn init(flash: esp_hal::peripherals::FLASH<'static>) -> &'static KvCell {
    let mut flash = FlashStorage::new(flash);
    let mut buf = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
    let range = {
        let pt = partitions::read_partition_table(&mut flash, &mut buf)
            .expect("no partition table: flash with `coffee flash` (uses partitions.csv)");
        let part = pt
            .find_partition(PartitionType::Data(DataPartitionSubType::Spiffs))
            .ok()
            .flatten()
            .expect("no `storage` partition: flash with `coffee flash` (uses partitions.csv)");
        part.offset()..part.offset() + part.len()
    };
    let kv = Kv::open(flash, range).expect("storage partition unusable");
    let cell: &'static KvCell = KV.init(CsMutex::new(RefCell::new(kv)));
    critical_section::with(|cs| *KV_REF.borrow_ref_mut(cs) = Some(cell));
    mark_boot_valid();
    cell
}

fn kv() -> &'static KvCell {
    critical_section::with(|cs| KV_REF.borrow_ref(cs).expect("store not initialised"))
}

/// Run `f` with the store (inside a critical section: keep it short, or a flash op).
pub fn with_kv<R>(f: impl FnOnce(&mut Kv<Flash>) -> R) -> R {
    critical_section::with(|cs| f(&mut kv().borrow_ref_mut(cs)))
}

pub fn wifi_creds() -> Option<WifiCreds> {
    let b = with_kv(|k| k.get(KEY_WIFI))?;
    serde_json::from_slice(&b).ok()
}

pub fn set_wifi(c: Option<&WifiCreds>) -> Result<(), String> {
    with_kv(|k| {
        match c {
            Some(c) => k.put(KEY_WIFI, &serde_json::to_vec(c).map_err(|e| alloc::format!("{e}"))?),
            None => k.remove(KEY_WIFI),
        }
        // Provisioning is a deliberate act: persist now, not at the next idle moment.
        k.flush_all().map_err(|e| alloc::format!("{e}"))
    })?;
    WIFI_CHANGED.signal(());
    Ok(())
}

/// Write pending saves to flash, one key at a time, while no shot is running.
#[embassy_executor::task]
pub async fn flusher() {
    loop {
        Timer::after(Duration::from_secs(2)).await;
        while !BUSY.load(Ordering::Relaxed) && with_kv(|k| k.pending()) > 0 {
            if let Err(e) = with_kv(|k| k.flush_one()) {
                crate::usb::event(&alloc::format!(r#"{{"event":"storage_error","error":"{e}"}}"#));
                break;
            }
            Timer::after(Duration::from_millis(50)).await; // let the network breathe between erases
        }
    }
}

/// Flush everything and reset (pending saves survive the reboot).
pub fn reboot() -> ! {
    let _ = with_kv(|k| k.flush_all());
    esp_hal::system::software_reset()
}

/// Mark this image good, so a rollback-capable bootloader keeps it.
fn mark_boot_valid() {
    with_kv(|k| {
        k.with_flash(|f| {
            let mut buf = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
            if let Ok(mut ota) = OtaUpdater::new(f, &mut buf)
                && let Ok(OtaImageState::New | OtaImageState::PendingVerify) = ota.current_ota_state()
            {
                let _ = ota.set_current_ota_state(OtaImageState::Valid);
            }
        })
    });
}

/// The app slot an OTA image goes to: (flash offset, size).
pub fn ota_target() -> Result<(u32, u32), String> {
    with_kv(|k| {
        k.with_flash(|f| {
            let mut buf = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
            let sub = {
                let mut ota = OtaUpdater::new(f, &mut buf).map_err(|e| alloc::format!("no OTA partitions: {e:?}"))?;
                ota.next_partition().map_err(|e| alloc::format!("{e:?}"))?.1
            };
            let mut buf = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
            let pt = partitions::read_partition_table(f, &mut buf).map_err(|e| alloc::format!("{e:?}"))?;
            let e = pt
                .find_partition(PartitionType::App(sub))
                .map_err(|e| alloc::format!("{e:?}"))?
                .ok_or_else(|| String::from("next OTA slot missing"))?;
            Ok((e.offset(), e.len()))
        })
    })
}

/// Erase-and-write one sector-aligned chunk of an OTA image (absolute address).
pub fn ota_write(addr: u32, data: &[u8]) -> Result<(), String> {
    let sector = FlashStorage::SECTOR_SIZE;
    with_kv(|k| {
        k.with_flash(|f| {
            if addr.is_multiple_of(sector) {
                NorFlash::erase(f, addr, addr + sector).map_err(|e| alloc::format!("erase: {e:?}"))?;
            }
            // Pad the tail to the 4-byte write size with erased bytes.
            let mut tmp;
            let d = if data.len().is_multiple_of(4) {
                data
            } else {
                tmp = alloc::vec::Vec::from(data);
                tmp.resize(data.len().next_multiple_of(4), 0xFF);
                &tmp
            };
            NorFlash::write(f, addr, d).map_err(|e| alloc::format!("write: {e:?}"))
        })
    })
}

/// Switch the bootloader to the freshly written slot.
pub fn ota_activate() -> Result<(), String> {
    with_kv(|k| {
        k.with_flash(|f| {
            let mut buf = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
            let mut ota = OtaUpdater::new(f, &mut buf).map_err(|e| alloc::format!("{e:?}"))?;
            ota.activate_next_partition().map_err(|e| alloc::format!("{e:?}"))?;
            ota.set_current_ota_state(OtaImageState::New).map_err(|e| alloc::format!("{e:?}"))
        })
    })
}
