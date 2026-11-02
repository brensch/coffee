//! File-backed [`coffee_node::Storage`]: keys are paths under a data directory.

use anyhow::Result;
use coffee_node::Storage;
use std::path::PathBuf;

pub struct FsStorage {
    root: PathBuf,
}

impl FsStorage {
    pub fn new(root: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
}

impl Storage for FsStorage {
    fn load(&mut self, key: &str) -> Option<Vec<u8>> {
        std::fs::read(self.root.join(key)).ok()
    }

    fn save(&mut self, key: &str, data: &[u8]) -> Result<(), String> {
        let p = self.root.join(key);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        // Write-then-rename so a crash never leaves a half-written file.
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, data)
            .and_then(|_| std::fs::rename(&tmp, &p))
            .map_err(|e| e.to_string())
    }

    fn list(&mut self, prefix: &str) -> Vec<String> {
        let (dir, file_prefix) = match prefix.rfind('/') {
            Some(i) => (&prefix[..=i], &prefix[i + 1..]),
            None => ("", prefix),
        };
        let mut out: Vec<String> = std::fs::read_dir(self.root.join(dir))
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| n.starts_with(file_prefix) && !n.ends_with(".tmp"))
                    .map(|n| format!("{dir}{n}"))
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }
}
