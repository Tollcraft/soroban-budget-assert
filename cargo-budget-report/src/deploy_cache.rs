//! On-disk cache of deployed contract ids (#79).
//!
//! Every `cargo budget-report` run previously redeployed every workspace
//! contract from scratch, paying the full `deploy_contract_with_retry` cost
//! (up to four attempts with 2/4/8s backoff to tolerate flaky friendbot
//! funding) even when the compiled wasm had not changed since the last run.
//! For the iterative "change a function, re-measure, repeat" workflow this
//! tool is built for, that is the dominant cost and it is avoidable.
//!
//! ## Cache key
//!
//! An entry is keyed on everything that changes what the deployment would
//! produce:
//!
//! * `wasm_sha256` — the hex SHA-256 of the compiled `.wasm` bytes. **Any
//!   change to the wasm invalidates the entry** — this is the property the
//!   whole feature stands on: a stale contract id silently measuring the
//!   previous build is worse than no cache.
//! * `network` — a testnet contract id must never be reused against a
//!   different network.
//! * `source` — the deploying account.
//!
//! ## Staleness
//!
//! A cached contract on testnet can be reclaimed by ledger state this tool
//! does not control. This cache **trusts a hit**: it does not round-trip to
//! the RPC to confirm the id still resolves before reusing it. If a cached
//! id no longer exists on-chain, the fix is `--no-deploy-cache` (or deleting
//! `.budget-cache.toml`), which forces a redeploy and rewrites the entry.
//! Proactive liveness checking is a deliberate follow-up, not part of this
//! change.
//!
//! ## Format
//!
//! `.budget-cache.toml` at the workspace root:
//!
//! ```toml
//! version = 1
//!
//! [[entry]]
//! package = "amm-pool-contract"
//! wasm_sha256 = "b1946ac92492d2347c6235b4d2611184"
//! network = "testnet"
//! source = "alice"
//! contract_id = "CA6H6..."
//! ```
//!
//! The file is git-ignored.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Default cache file name, matching the name the developer guide already used.
pub const CACHE_FILE: &str = ".budget-cache.toml";

/// Current on-disk schema version. Bump on any breaking format change; an
/// unrecognised version is treated as a cold cache rather than an error.
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CacheEntry {
    package: String,
    wasm_sha256: String,
    network: String,
    source: String,
    contract_id: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    #[serde(default)]
    version: u32,
    #[serde(default, rename = "entry")]
    entries: Vec<CacheEntry>,
}

/// A loaded deploy cache bound to a path on disk.
#[derive(Debug)]
pub struct DeployCache {
    path: PathBuf,
    file: CacheFile,
    dirty: bool,
}

/// Hex SHA-256 of a compiled wasm file's bytes.
pub fn wasm_hash(wasm_path: &Path) -> Result<String> {
    let bytes = std::fs::read(wasm_path)
        .with_context(|| format!("failed to read wasm for hashing: {}", wasm_path.display()))?;
    Ok(hex_sha256(&bytes))
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl DeployCache {
    /// Load the cache at `dir/.budget-cache.toml`, or an empty cache if the
    /// file is absent, unreadable, malformed, or a version this build does
    /// not understand. A cache is best-effort: a broken file must never
    /// fail a run, only lose its warm entries.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(CACHE_FILE);
        let file = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| toml::from_str::<CacheFile>(&text).ok())
            .filter(|f| f.version == CACHE_VERSION)
            .unwrap_or_default();
        DeployCache {
            path,
            file,
            dirty: false,
        }
    }

    /// Look up a previously deployed contract id for this exact
    /// (wasm hash, network, source) triple.
    pub fn get(&self, wasm_sha256: &str, network: &str, source: &str) -> Option<&str> {
        self.file
            .entries
            .iter()
            .find(|e| e.wasm_sha256 == wasm_sha256 && e.network == network && e.source == source)
            .map(|e| e.contract_id.as_str())
    }

    /// Record a fresh deployment, replacing any prior entry for the same
    /// (package, network, source) — a rebuild changes `wasm_sha256`, and the
    /// old id for that package on that network/source is now dead weight.
    pub fn put(
        &mut self,
        package: &str,
        wasm_sha256: &str,
        network: &str,
        source: &str,
        contract_id: &str,
    ) {
        self.file
            .entries
            .retain(|e| !(e.package == package && e.network == network && e.source == source));
        self.file.entries.push(CacheEntry {
            package: package.to_string(),
            wasm_sha256: wasm_sha256.to_string(),
            network: network.to_string(),
            source: source.to_string(),
            contract_id: contract_id.to_string(),
        });
        self.dirty = true;
    }

    /// Persist the cache to disk if it changed since load. Best-effort: a
    /// write failure is surfaced to the caller but is not itself fatal to a
    /// completed measurement run.
    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.file.version = CACHE_VERSION;
        let text =
            toml::to_string_pretty(&self.file).context("failed to serialise the deploy cache")?;
        std::fs::write(&self.path, text)
            .with_context(|| format!("failed to write deploy cache {}", self.path.display()))?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    const ID_A: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const ID_B: &str = "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

    #[test]
    fn cold_cache_is_a_miss() {
        let dir = tempdir().unwrap();
        let cache = DeployCache::load(dir.path());
        assert_eq!(cache.get("deadbeef", "testnet", "alice"), None);
    }

    #[test]
    fn warm_hit_after_put_and_reload() {
        let dir = tempdir().unwrap();
        let mut cache = DeployCache::load(dir.path());
        cache.put("pkg", "hash-1", "testnet", "alice", ID_A);
        cache.save().unwrap();

        let reloaded = DeployCache::load(dir.path());
        assert_eq!(reloaded.get("hash-1", "testnet", "alice"), Some(ID_A));
    }

    #[test]
    fn changed_wasm_invalidates_the_entry() {
        let dir = tempdir().unwrap();
        let mut cache = DeployCache::load(dir.path());
        cache.put("pkg", "hash-old", "testnet", "alice", ID_A);
        cache.save().unwrap();

        // A rebuild produces a different wasm hash — the old id must not be
        // served for it.
        let reloaded = DeployCache::load(dir.path());
        assert_eq!(reloaded.get("hash-new", "testnet", "alice"), None);
        assert_eq!(reloaded.get("hash-old", "testnet", "alice"), Some(ID_A));
    }

    #[test]
    fn changed_network_invalidates_the_entry() {
        let dir = tempdir().unwrap();
        let mut cache = DeployCache::load(dir.path());
        cache.put("pkg", "hash-1", "testnet", "alice", ID_A);
        cache.save().unwrap();

        let reloaded = DeployCache::load(dir.path());
        assert_eq!(reloaded.get("hash-1", "futurenet", "alice"), None);
        assert_eq!(reloaded.get("hash-1", "testnet", "alice"), Some(ID_A));
    }

    #[test]
    fn changed_source_invalidates_the_entry() {
        let dir = tempdir().unwrap();
        let mut cache = DeployCache::load(dir.path());
        cache.put("pkg", "hash-1", "testnet", "alice", ID_A);
        cache.save().unwrap();

        let reloaded = DeployCache::load(dir.path());
        assert_eq!(reloaded.get("hash-1", "testnet", "bob"), None);
    }

    #[test]
    fn redeploy_replaces_the_stale_package_entry() {
        let dir = tempdir().unwrap();
        let mut cache = DeployCache::load(dir.path());
        cache.put("pkg", "hash-old", "testnet", "alice", ID_A);
        cache.put("pkg", "hash-new", "testnet", "alice", ID_B);
        cache.save().unwrap();

        let reloaded = DeployCache::load(dir.path());
        assert_eq!(reloaded.get("hash-old", "testnet", "alice"), None);
        assert_eq!(reloaded.get("hash-new", "testnet", "alice"), Some(ID_B));
    }

    #[test]
    fn malformed_file_loads_as_cold_cache() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(CACHE_FILE), "this is not toml : : :").unwrap();
        let cache = DeployCache::load(dir.path());
        assert_eq!(cache.get("anything", "testnet", "alice"), None);
    }

    #[test]
    fn unknown_version_is_treated_as_cold() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join(CACHE_FILE),
            "version = 999\n[[entry]]\npackage=\"p\"\nwasm_sha256=\"h\"\nnetwork=\"testnet\"\nsource=\"alice\"\ncontract_id=\"CID\"\n",
        )
        .unwrap();
        let cache = DeployCache::load(dir.path());
        assert_eq!(cache.get("h", "testnet", "alice"), None);
    }

    #[test]
    fn hex_sha256_is_stable_and_length_64() {
        let h = hex_sha256(b"");
        assert_eq!(
            h,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(hex_sha256(b"abc").len(), 64);
        assert_ne!(hex_sha256(b"abc"), hex_sha256(b"abd"));
    }
}
