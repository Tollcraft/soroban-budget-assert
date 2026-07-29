use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const CACHE_FILE: &str = ".budget-cache.toml";

/// A cache entry for one function simulation of one package.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheEntry {
    /// CPU instructions from the last successful simulation.
    pub instructions: u32,
    /// Read bytes from the last successful simulation.
    pub read_bytes: u32,
    /// Write bytes from the last successful simulation.
    pub write_bytes: u32,
    /// WASM binary size in bytes at the time of caching.
    pub wasm_size: u32,
    /// Hash of the WASM file content used to detect staleness.
    pub wasm_hash: String,
    /// Unix timestamp (seconds since epoch) when this entry was cached.
    pub cached_at: String,
}

/// The top-level budget cache structure serialized to `.budget-cache.toml`.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct BudgetCache {
    /// Schema version for forward compatibility.
    #[serde(default = "default_version")]
    pub version: u64,
    /// Unix timestamp of initial cache file creation.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Unix timestamp of the most recent cache update.
    #[serde(default)]
    pub updated_at: Option<String>,
    /// Map of cache entries keyed by `"<package>::<function>"`.
    #[serde(default)]
    pub entry: HashMap<String, CacheEntry>,
}

fn default_version() -> u64 {
    1
}

pub fn now_unix_str() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| String::new())
}

impl BudgetCache {
    /// Load the cache from `.budget-cache.toml` in the current directory.
    /// Returns an empty cache if the file does not exist or cannot be parsed.
    pub fn load() -> Self {
        let path = Path::new(CACHE_FILE);
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    return Self::default();
                }
                toml::from_str(trimmed).unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: failed to parse {}: {}; starting with empty cache",
                        CACHE_FILE, e
                    );
                    Self::default()
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => {
                eprintln!(
                    "Warning: failed to read {}: {}; starting with empty cache",
                    CACHE_FILE, e
                );
                Self::default()
            }
        }
    }

    /// Save the cache to `.budget-cache.toml` in the current directory.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Path::new(CACHE_FILE);
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Look up a cached entry by key. Returns `Some` only if the stored
    /// `wasm_hash` matches the provided `wasm_hash`, indicating the entry
    /// is still valid.
    pub fn get(&self, key: &str, wasm_hash: &str) -> Option<&CacheEntry> {
        self.entry
            .get(key)
            .filter(|entry| entry.wasm_hash == wasm_hash)
    }

    /// Insert or update a cache entry and update the `updated_at` timestamp.
    pub fn set(&mut self, key: String, entry: CacheEntry) {
        if self.created_at.is_none() {
            self.created_at = Some(now_unix_str());
        }
        self.updated_at = Some(now_unix_str());
        self.entry.insert(key, entry);
    }
}

/// Compute the composite cache key for a package and function.
pub fn cache_key(package: &str, function: &str) -> String {
    format!("{}::{}", package, function)
}

/// Compute a content-based hash of the WASM binary.
/// Uses `std::hash::DefaultHasher` and returns a hex-encoded 64-bit hash.
pub fn hash_wasm(bytes: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
