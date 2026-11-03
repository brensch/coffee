//! Flash-backed key/value store for `coffee_node::Storage`, write-back cached.
//!
//! The storage partition holds a `sequential-storage` map (wear-levelled, power-
//! loss safe) keyed by strings. Values longer than one flash page are split into
//! chunks: the first record is `[n_chunks: u8][data]`, the rest live under
//! `"{key}\x01{i}"`. An empty record is a tombstone.
//!
//! Writes never touch flash directly. `Storage::save` only updates a RAM map of
//! dirty keys, so the node can save from inside the control loop at no cost.
//! A low-priority task calls [`Kv::flush_one`] later, while the machine is idle.
//! On the C6 an erase stalls the CPU (and so every interrupt) for tens of
//! milliseconds, so it must not happen mid-shot. Reads fall through to flash.
//!
//! When the map fills up, the oldest persisted shot is evicted and the write
//! retried; shots are the only thing that grows without bound.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;
use embedded_storage::nor_flash::{ErrorType, NorFlash, ReadNorFlash};
use sequential_storage::cache::{Cache, Uncached};
use sequential_storage::map::{MapConfig, MapStorage};

/// Data bytes per flash record (a 4 KB page minus headers, with margin).
pub const CHUNK: usize = 3584;
const BUF: usize = 4096;
const SEP: char = '\u{1}';
/// Persisted shots kept before the oldest is evicted.
pub const KEEP_SHOTS: usize = 60;
const SHOT_PREFIX: &str = "shots/";

/// Blocking `NorFlash` seen through the async traits `sequential-storage` wants.
/// Every call completes immediately, so `block_on` drives it.
pub struct Blocking<F>(pub F);

impl<F: ErrorType> embedded_storage_async::nor_flash::ErrorType for Blocking<F> {
    type Error = F::Error;
}

impl<F: ReadNorFlash> embedded_storage_async::nor_flash::ReadNorFlash for Blocking<F> {
    const READ_SIZE: usize = F::READ_SIZE;
    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read(offset, bytes)
    }
    fn capacity(&self) -> usize {
        self.0.capacity()
    }
}

impl<F: NorFlash> embedded_storage_async::nor_flash::NorFlash for Blocking<F> {
    const WRITE_SIZE: usize = F::WRITE_SIZE;
    const ERASE_SIZE: usize = F::ERASE_SIZE;
    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.0.erase(from, to)
    }
    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.write(offset, bytes)
    }
}

type Map<F> = MapStorage<String, Blocking<F>, Cache<Uncached, Uncached, Uncached, String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KvError {
    Flash,
    Full,
    Corrupted,
    Other(String),
}

impl core::fmt::Display for KvError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            KvError::Flash => write!(f, "flash error"),
            KvError::Full => write!(f, "storage full"),
            KvError::Corrupted => write!(f, "storage corrupted"),
            KvError::Other(e) => write!(f, "{e}"),
        }
    }
}

fn map_err<E: core::fmt::Debug>(e: sequential_storage::Error<E>) -> KvError {
    match e {
        sequential_storage::Error::Storage { .. } => KvError::Flash,
        sequential_storage::Error::FullStorage => KvError::Full,
        sequential_storage::Error::Corrupted { .. } => KvError::Corrupted,
        other => KvError::Other(format!("{other:?}")),
    }
}

pub struct Kv<F: NorFlash> {
    /// `None` only while `with_flash` lends the flash out.
    map: Option<Map<F>>,
    range: Range<u32>,
    /// Pending writes (None = delete), applied by `flush_one`.
    dirty: BTreeMap<String, Option<Vec<u8>>>,
    /// Every live user-visible key (flash ∪ dirty, minus deletions).
    keys: BTreeSet<String>,
}

impl<F: NorFlash> Kv<F>
where
    F::Error: core::fmt::Debug,
{
    /// Open the map in `range` (absolute flash addresses, erase-aligned) and index its keys.
    /// A corrupted map is erased (settings are lost, the machine still works).
    pub fn open(flash: F, range: Range<u32>) -> Result<Self, KvError> {
        let cfg = MapConfig::try_new(range.clone()).map_err(|e| KvError::Other(format!("{e:?}")))?;
        let mut kv = Kv {
            map: Some(MapStorage::new(Blocking(flash), cfg, Cache::new_uncached())),
            range: range.clone(),
            dirty: BTreeMap::new(),
            keys: BTreeSet::new(),
        };
        match kv.index() {
            Ok(()) => {}
            Err(KvError::Corrupted) => {
                embassy_futures::block_on(kv.m().erase_all()).map_err(map_err)?;
                kv.keys.clear();
            }
            Err(e) => return Err(e),
        }
        Ok(kv)
    }

    fn m(&mut self) -> &mut Map<F> {
        self.map.as_mut().expect("flash lent out")
    }

    /// Lend the raw flash (for OTA writes outside the map's range), then rebuild the map.
    pub fn with_flash<R>(&mut self, f: impl FnOnce(&mut F) -> R) -> R {
        let (Blocking(mut flash), cache) = self.map.take().expect("flash lent out").destroy();
        let r = f(&mut flash);
        let cfg = MapConfig::try_new(self.range.clone()).unwrap_or_else(|_| unreachable!());
        self.map = Some(MapStorage::new(Blocking(flash), cfg, cache));
        r
    }

    fn index(&mut self) -> Result<(), KvError> {
        let mut buf = vec![0u8; BUF];
        let mut keys = BTreeSet::new();
        embassy_futures::block_on(async {
            let mut it = self.m().fetch_all_items(&mut buf).await.map_err(map_err)?;
            let mut item = vec![0u8; BUF];
            // The iterator yields every record, newest last: keep the final state per key.
            while let Some((k, v)) = it.next::<&[u8]>(&mut item).await.map_err(map_err)? {
                if k.contains(SEP) {
                    continue;
                }
                if v.is_empty() {
                    keys.remove(&k);
                } else {
                    keys.insert(k);
                }
            }
            Ok::<(), KvError>(())
        })?;
        self.keys = keys;
        Ok(())
    }

    fn fetch_raw(&mut self, key: &str) -> Result<Option<Vec<u8>>, KvError> {
        let mut buf = vec![0u8; BUF];
        let k = key.to_string();
        embassy_futures::block_on(async {
            let v: Option<&[u8]> = self.m().fetch_item(&mut buf, &k).await.map_err(map_err)?;
            Ok(v.map(|s| s.to_vec()))
        })
    }

    fn store_raw(&mut self, key: &str, data: &[u8]) -> Result<(), KvError> {
        let mut buf = vec![0u8; BUF];
        let k = key.to_string();
        embassy_futures::block_on(self.m().store_item(&mut buf, &k, &data)).map_err(map_err)
    }

    /// Read a value (pending writes first, then flash).
    pub fn get(&mut self, key: &str) -> Option<Vec<u8>> {
        if let Some(d) = self.dirty.get(key) {
            return d.clone();
        }
        let head = self.fetch_raw(key).ok()??;
        let (&n, first) = head.split_first()?;
        let mut out = first.to_vec();
        for i in 1..n as usize {
            out.extend(self.fetch_raw(&format!("{key}{SEP}{i}")).ok()??);
        }
        Some(out)
    }

    /// Queue a write. Takes effect for `get`/`list` at once, on flash at the next flush.
    pub fn put(&mut self, key: &str, data: &[u8]) {
        self.keys.insert(key.to_string());
        self.dirty.insert(key.to_string(), Some(data.to_vec()));
    }

    /// Queue a delete.
    pub fn remove(&mut self, key: &str) {
        self.keys.remove(key);
        self.dirty.insert(key.to_string(), None);
    }

    pub fn list(&self, prefix: &str) -> Vec<String> {
        self.keys.range(prefix.to_string()..).take_while(|k| k.starts_with(prefix)).cloned().collect()
    }

    pub fn pending(&self) -> usize {
        self.dirty.len()
    }

    /// Write one pending key to flash. Returns `Ok(false)` when nothing was pending.
    pub fn flush_one(&mut self) -> Result<bool, KvError> {
        let Some((key, val)) = self.dirty.pop_first() else {
            return Ok(false);
        };
        let r = match &val {
            Some(data) => self.write_chunks(&key, data),
            None => self.store_raw(&key, &[]),
        };
        match r {
            Ok(()) => {
                if val.is_some() && key.starts_with(SHOT_PREFIX) {
                    self.trim_shots();
                }
                Ok(true)
            }
            Err(e) => {
                self.dirty.insert(key, val); // keep it for the next attempt
                Err(e)
            }
        }
    }

    /// Flush everything (used before a reboot).
    pub fn flush_all(&mut self) -> Result<(), KvError> {
        while self.flush_one()? {}
        Ok(())
    }

    fn write_chunks(&mut self, key: &str, data: &[u8]) -> Result<(), KvError> {
        let chunks: Vec<&[u8]> = if data.is_empty() { vec![&[][..]] } else { data.chunks(CHUNK).collect() };
        if chunks.len() > 255 {
            return Err(KvError::Other("value too large".into()));
        }
        for attempt in 0..4 {
            let r = (|| {
                for (i, c) in chunks.iter().enumerate().skip(1) {
                    self.store_raw(&format!("{key}{SEP}{i}"), c)?;
                }
                let mut head = Vec::with_capacity(chunks[0].len() + 1);
                head.push(chunks.len() as u8);
                head.extend_from_slice(chunks[0]);
                self.store_raw(key, &head)
            })();
            match r {
                Err(KvError::Full) if attempt < 3 => {
                    if !self.evict_oldest_shot(key)? {
                        return Err(KvError::Full);
                    }
                }
                other => return other,
            }
        }
        Err(KvError::Full)
    }

    fn evict_oldest_shot(&mut self, except: &str) -> Result<bool, KvError> {
        let Some(old) = self.list(SHOT_PREFIX).into_iter().find(|k| k != except) else {
            return Ok(false);
        };
        self.keys.remove(&old);
        self.dirty.remove(&old);
        self.store_raw(&old, &[])?;
        Ok(true)
    }

    fn trim_shots(&mut self) {
        let shots = self.list(SHOT_PREFIX);
        if shots.len() > KEEP_SHOTS {
            for k in &shots[..shots.len() - KEEP_SHOTS] {
                self.keys.remove(k);
                let _ = self.store_raw(k, &[]);
            }
        }
    }
}

/// A RAM `NorFlash` for tests and the host (4 KB sectors, 4-byte words).
pub struct RamFlash {
    pub data: Vec<u8>,
    pub erases: usize,
}

impl RamFlash {
    pub fn new(size: usize) -> Self {
        RamFlash { data: vec![0xFF; size], erases: 0 }
    }
}

#[derive(Debug)]
pub struct RamFlashError;

impl embedded_storage::nor_flash::NorFlashError for RamFlashError {
    fn kind(&self) -> embedded_storage::nor_flash::NorFlashErrorKind {
        embedded_storage::nor_flash::NorFlashErrorKind::Other
    }
}

impl ErrorType for RamFlash {
    type Error = RamFlashError;
}

impl ReadNorFlash for RamFlash {
    const READ_SIZE: usize = 1;
    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        let o = offset as usize;
        bytes.copy_from_slice(self.data.get(o..o + bytes.len()).ok_or(RamFlashError)?);
        Ok(())
    }
    fn capacity(&self) -> usize {
        self.data.len()
    }
}

impl NorFlash for RamFlash {
    const WRITE_SIZE: usize = 4;
    const ERASE_SIZE: usize = 4096;
    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        self.data.get_mut(from as usize..to as usize).ok_or(RamFlashError)?.fill(0xFF);
        self.erases += 1;
        Ok(())
    }
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let o = offset as usize;
        let d = self.data.get_mut(o..o + bytes.len()).ok_or(RamFlashError)?;
        for (a, b) in d.iter_mut().zip(bytes) {
            *a &= *b; // NOR: bits only go 1 -> 0
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(pages: usize) -> Kv<RamFlash> {
        Kv::open(RamFlash::new(pages * 4096), 0..(pages * 4096) as u32).unwrap()
    }

    /// Reopen from the same flash contents (as after a reboot).
    fn reopen(k: Kv<RamFlash>, pages: usize) -> Kv<RamFlash> {
        let (Blocking(f), _) = k.map.unwrap().destroy();
        Kv::open(f, 0..(pages * 4096) as u32).unwrap()
    }

    #[test]
    fn write_back_then_persist() {
        let mut k = kv(8);
        k.put("config.json", b"{\"a\":1}");
        assert_eq!(k.get("config.json").as_deref(), Some(&b"{\"a\":1}"[..]));
        assert_eq!(k.pending(), 1);
        // not on flash yet: a reboot now loses it
        let mut k = reopen(k, 8);
        assert_eq!(k.get("config.json"), None);
        k.put("config.json", b"{\"a\":2}");
        k.flush_all().unwrap();
        let mut k = reopen(k, 8);
        assert_eq!(k.get("config.json").as_deref(), Some(&b"{\"a\":2}"[..]));
        assert_eq!(k.list(""), vec!["config.json".to_string()]);
    }

    #[test]
    fn large_values_are_chunked() {
        let mut k = kv(16);
        let big: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        k.put("profiles/big.json", &big);
        k.flush_all().unwrap();
        let mut k = reopen(k, 16);
        assert_eq!(k.get("profiles/big.json"), Some(big));
        assert_eq!(k.list("profiles/"), vec!["profiles/big.json".to_string()]);
    }

    #[test]
    fn delete_and_list_prefix() {
        let mut k = kv(8);
        k.put("profiles/a", b"1");
        k.put("profiles/b", b"2");
        k.put("shots/00001.cshot", b"s");
        k.flush_all().unwrap();
        k.remove("profiles/a");
        k.flush_all().unwrap();
        let k = reopen(k, 8);
        assert_eq!(k.list("profiles/"), vec!["profiles/b".to_string()]);
        assert_eq!(k.list("shots/").len(), 1);
    }

    #[test]
    fn shots_are_trimmed_and_evicted_when_full() {
        let mut k = kv(8); // 32 KB: fills long before KEEP_SHOTS
        let shot = vec![7u8; 2600];
        for id in 0..40 {
            k.put(&format!("shots/{id:05}.cshot"), &shot);
            k.flush_all().unwrap();
        }
        k.put("config.json", b"{}");
        k.flush_all().unwrap();
        let mut k = reopen(k, 8);
        let shots = k.list("shots/");
        assert!(!shots.is_empty() && shots.len() < 40, "kept {}", shots.len());
        // the newest survives, the oldest went first
        assert!(shots.contains(&"shots/00039.cshot".to_string()));
        assert!(!shots.contains(&"shots/00000.cshot".to_string()));
        assert_eq!(k.get("config.json").as_deref(), Some(&b"{}"[..]));
    }

    #[test]
    fn lending_flash_keeps_the_map() {
        let mut k = kv(8);
        k.put("a", b"1");
        k.flush_all().unwrap();
        let cap = k.with_flash(|f| f.capacity());
        assert_eq!(cap, 8 * 4096);
        assert_eq!(k.get("a").as_deref(), Some(&b"1"[..]));
    }

    #[test]
    fn overwrites_keep_the_latest() {
        let mut k = kv(4);
        for i in 0..200u32 {
            k.put("active_profile", format!("p{i}").as_bytes());
            k.flush_all().unwrap();
        }
        let mut k = reopen(k, 4);
        assert_eq!(k.get("active_profile").as_deref(), Some(&b"p199"[..]));
    }
}
