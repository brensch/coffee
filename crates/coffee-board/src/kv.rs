//! Flash-backed key/value store for `coffee_node::Storage`, write-back cached.
//!
//! The storage partition holds a `sequential-storage` map (wear-levelled, power-
//! loss safe) keyed by strings. Values longer than one flash page are split into
//! chunks. The head record, `[n_chunks: u8][generation: u32][crc32: u32][chunk 0]`, is
//! written last and is the commit: chunks 1.. live under `"{key}\x01{generation}\x01{i}"`,
//! so a power cut mid-write leaves the previous generation intact and readable.
//! Reads check the CRC over the whole value and return nothing rather than a torn
//! or corrupted value. An empty record is a tombstone.
//!
//! Writes never touch flash directly. `Storage::save` only updates a RAM map of
//! dirty keys, so the node can save from inside the control loop at no cost.
//! A low-priority task calls [`Kv::flush_one`] later, while the machine is idle.
//! On the C6 an erase stalls the CPU (and so every interrupt) for tens of
//! milliseconds, so it must not happen mid-shot. Reads fall through to flash.
//!
//! When the map fills up, the oldest persisted shot is evicted and the write
//! retried; shots are the only thing that grows without bound.

#[cfg(test)]
extern crate std;
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
const HEAD: usize = 9; // n_chunks + generation + crc32

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}
/// Persisted shots kept before the oldest is evicted.
pub const KEEP_SHOTS: usize = 60;
const SHOT_PREFIX: &str = "shots/";

fn chunk_key(key: &str, generation: u32, i: usize) -> String {
    format!("{key}{SEP}{generation:08x}{SEP}{i}")
}

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
        // A power cut mid-write or mid-erase leaves a page half done. Listing all items doesn't
        // run sequential-storage's repair, but a single fetch does: so on "corrupted", let a fetch
        // repair the map and list again. Only a map that is still corrupted after repair (real
        // flash damage) is erased, and then the machine keeps working with default settings.
        let mut tries = 0;
        loop {
            match kv.index() {
                Ok(()) => break,
                Err(KvError::Corrupted) if tries < 3 => {
                    tries += 1;
                    let _ = kv.fetch_raw("\u{2}repair");
                }
                Err(KvError::Corrupted) => {
                    embassy_futures::block_on(kv.m().erase_all()).map_err(map_err)?;
                    kv.keys.clear();
                    break;
                }
                Err(e) => return Err(e),
            }
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
        self.read_value(key)
    }

    /// Read a committed value from flash: the head names its generation and CRC; a missing
    /// chunk or a CRC mismatch means "no value" (never a torn one).
    fn read_value(&mut self, key: &str) -> Option<Vec<u8>> {
        let head = self.fetch_raw(key).ok()??;
        if head.len() < HEAD {
            return None;
        }
        let n = head[0] as usize;
        let generation = u32::from_le_bytes(head[1..5].try_into().ok()?);
        let crc = u32::from_le_bytes(head[5..9].try_into().ok()?);
        let mut out = head[HEAD..].to_vec();
        for i in 1..n {
            out.extend(self.fetch_raw(&chunk_key(key, generation, i)).ok()??);
        }
        (crc32(&out) == crc).then_some(out)
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
            None => self.delete_value(&key),
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
        // The previous generation stays untouched until the new head (the commit) is on flash.
        let prev = self.fetch_raw(key).ok().flatten().filter(|h| h.len() >= HEAD)
            .map(|h| (h[0] as usize, u32::from_le_bytes([h[1], h[2], h[3], h[4]])));
        let generation = prev.map_or(1, |(_, g)| g.wrapping_add(1));
        let crc = crc32(data);
        for attempt in 0..4 {
            let r = (|| {
                for (i, c) in chunks.iter().enumerate().skip(1) {
                    self.store_raw(&chunk_key(key, generation, i), c)?;
                }
                let mut head = Vec::with_capacity(chunks[0].len() + HEAD);
                head.push(chunks.len() as u8);
                head.extend_from_slice(&generation.to_le_bytes());
                head.extend_from_slice(&crc.to_le_bytes());
                head.extend_from_slice(chunks[0]);
                self.store_raw(key, &head)
            })();
            if r.is_ok() {
                // committed: tombstone the old generation's chunks (a crash here only leaves garbage)
                if let Some((n, g)) = prev {
                    for i in 1..n {
                        let _ = self.store_raw(&chunk_key(key, g, i), &[]);
                    }
                }
            }
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

    /// Tombstone the head first (the value is gone from then on), then its chunks.
    fn delete_value(&mut self, key: &str) -> Result<(), KvError> {
        let prev = self.fetch_raw(key).ok().flatten().filter(|h| h.len() >= HEAD)
            .map(|h| (h[0] as usize, u32::from_le_bytes([h[1], h[2], h[3], h[4]])));
        self.store_raw(key, &[])?;
        if let Some((n, g)) = prev {
            for i in 1..n {
                let _ = self.store_raw(&chunk_key(key, g, i), &[]);
            }
        }
        Ok(())
    }

    fn evict_oldest_shot(&mut self, except: &str) -> Result<bool, KvError> {
        let Some(old) = self.list(SHOT_PREFIX).into_iter().find(|k| k != except) else {
            return Ok(false);
        };
        self.keys.remove(&old);
        self.dirty.remove(&old);
        self.delete_value(&old)?;
        Ok(true)
    }

    fn trim_shots(&mut self) {
        let shots = self.list(SHOT_PREFIX);
        if shots.len() > KEEP_SHOTS {
            for k in &shots[..shots.len() - KEEP_SHOTS] {
                self.keys.remove(k);
                let _ = self.delete_value(k);
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

    /// RAM flash that loses power after `budget` write/erase operations: the operation in
    /// flight is left half done (a write programs only its first half; an erase clears only
    /// its first half), and every later operation fails, like a chip whose supply collapsed.
    struct CutFlash {
        f: RamFlash,
        budget: usize,
    }

    impl ErrorType for CutFlash {
        type Error = RamFlashError;
    }

    impl ReadNorFlash for CutFlash {
        const READ_SIZE: usize = 1;
        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            self.f.read(offset, bytes)
        }
        fn capacity(&self) -> usize {
            self.f.capacity()
        }
    }

    impl NorFlash for CutFlash {
        const WRITE_SIZE: usize = 4;
        const ERASE_SIZE: usize = 4096;
        fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            if self.budget == 0 {
                return Err(RamFlashError);
            }
            self.budget -= 1;
            if self.budget == 0 {
                let mid = from + (to - from) / 2;
                self.f.data[from as usize..mid as usize].fill(0xFF);
                return Err(RamFlashError);
            }
            self.f.erase(from, to)
        }
        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            if self.budget == 0 {
                return Err(RamFlashError);
            }
            self.budget -= 1;
            if self.budget == 0 {
                let half = (bytes.len() / 2) & !3;
                let _ = self.f.write(offset, &bytes[..half]);
                return Err(RamFlashError);
            }
            self.f.write(offset, bytes)
        }
    }

    fn value(tag: u8, len: usize) -> Vec<u8> {
        (0..len).map(|i| tag.wrapping_mul(31).wrapping_add(i as u8)).collect()
    }

    /// Cut power at every single flash operation of a realistic save sequence (settings
    /// rewritten, multi-chunk shots added past the eviction point, a delete) and check that
    /// the store always reopens without being erased and every key reads as one of the
    /// complete values it ever held, never a torn mix.
    #[test]
    fn survives_power_cut_at_every_flash_operation() {
        const SECTORS: usize = 8;
        // the history each key may legitimately read back as
        let ops: Vec<(String, Option<Vec<u8>>)> = {
            let mut v = Vec::new();
            for i in 0..12u8 {
                v.push(("config.json".to_string(), Some(value(i, 300 + i as usize * 40))));
                v.push((format!("shots/{i:05}.cshot"), Some(value(100 + i, 2600 + i as usize * 900)))); // 1-2 chunks
                if i == 6 {
                    v.push(("shots/00002.cshot".to_string(), None));
                }
            }
            v
        };
        // run once with no cut to count the flash operations
        let total = {
            let mut base = RamFlash::new(SECTORS * 4096);
            let mut k = Kv::open(CutFlash { f: core::mem::replace(&mut base, RamFlash::new(0)), budget: usize::MAX }, 0..(SECTORS * 4096) as u32).unwrap();
            for (key, val) in &ops {
                match val { Some(d) => k.put(key, d), None => k.remove(key) }
                k.flush_all().unwrap();
            }
            usize::MAX - k.with_flash(|f| f.budget)
        };
        assert!(total > 50, "only {total} flash operations");
        let mut cuts = 0;
        for cut in 1..=total {
            let mut k = Kv::open(CutFlash { f: RamFlash::new(SECTORS * 4096), budget: cut }, 0..(SECTORS * 4096) as u32).unwrap();
            let mut done = 0; // ops fully on flash before the power went
            for (key, val) in &ops {
                match val { Some(d) => k.put(key, d), None => k.remove(key) }
                if k.flush_all().is_err() {
                    break;
                }
                done += 1;
            }
            let flash = k.with_flash(|f| core::mem::replace(&mut f.f, RamFlash::new(0)));
            let mut r = Kv::open(CutFlash { f: flash, budget: usize::MAX }, 0..(SECTORS * 4096) as u32)
                .unwrap_or_else(|e| panic!("cut at op {cut}: reopen failed: {e:?}"));
            // Every key: committed state = its last op among ops[..done]; the op in flight
            // (ops[done]) may or may not have landed. Shots may also have been evicted for space.
            let mut keys: Vec<&String> = ops.iter().map(|(k, _)| k).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let committed = ops[..done].iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.clone());
                let inflight = ops.get(done).filter(|(k, _)| k == key).map(|(_, v)| v.clone());
                let got = r.get(key);
                let ok = match (&got, &committed) {
                    (g, Some(c)) if g == c => true,
                    (g, _) if inflight.as_ref().is_some_and(|i| i == g) => true,
                    (None, _) if key.starts_with("shots/") => true, // evicted to make room
                    (None, None) => true,
                    _ => false,
                };
                assert!(ok, "cut at op {cut} (after {done} ops): {key} read {:?} bytes, committed {:?}",
                        got.as_ref().map(|g| g.len()), committed.as_ref().map(|c| c.as_ref().map(|c| c.len())));
            }
            assert!(r.get("config.json").is_some() || done == 0, "cut at op {cut}: settings lost");
            cuts += 1;
        }
        assert_eq!(cuts, total);
    }
}
