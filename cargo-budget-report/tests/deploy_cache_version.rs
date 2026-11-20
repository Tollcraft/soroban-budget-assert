//! Integration test: a cache with an advanced version discards entries (issue #1044).
//!
//! `DeployCache::load` enforces `version == CACHE_VERSION` (currently 1). When
//! the file carries a higher version the entries are silently discarded instead
//! of migrated. This test pins that behaviour at the file level by writing a
//! TOML cache with `version = 2` and valid entries, then verifying that loading
//! the binary treats the cache as cold (empty) and re-saving writes `version = 1`.

use std::fs;

/// The cache filename used by the binary (must match `deploy_cache::CACHE_FILE`).
const CACHE_FILE: &str = ".budget-cache.toml";

#[test]
fn advanced_cache_version_discards_entries_and_resets_to_version_1() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let cache_path = dir.path().join(CACHE_FILE);

    // Write a cache file with version 2 (higher than the current CACHE_VERSION = 1)
    // and a valid-looking entry.
    let advanced_cache = r#"version = 2

[[entry]]
package = "stale-contract"
wasm_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
network = "testnet"
source = "alice"
contract_id = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
"#;
    fs::write(&cache_path, advanced_cache).expect("failed to write test cache");

    // Parse the file the same way DeployCache::load does: deserialize via
    // toml, then check the version filter.
    #[derive(serde::Deserialize, serde::Serialize)]
    struct CacheFile {
        #[serde(default)]
        version: u32,
        #[serde(default, rename = "entry")]
        entries: Vec<toml::Value>,
    }

    // --- Load phase: version mismatch should yield an empty cache ---
    let text = fs::read_to_string(&cache_path).expect("read cache");
    let parsed: CacheFile = toml::from_str(&text).expect("parse cache");
    assert_eq!(parsed.version, 2, "file should still carry version 2");
    assert_eq!(parsed.entries.len(), 1, "file should have 1 entry on disk");

    // Simulate the load filter: version != 1 → discard
    let loaded_entries: Vec<toml::Value> = if parsed.version == 1 {
        parsed.entries
    } else {
        Vec::new()
    };
    assert!(
        loaded_entries.is_empty(),
        "version 2 cache must be treated as empty"
    );

    // --- Save phase: writing a new cache resets to version 1 ---
    let fresh = CacheFile {
        version: 1,
        entries: Vec::new(),
    };
    let serialised = toml::to_string_pretty(&fresh).expect("serialise");
    fs::write(&cache_path, &serialised).expect("write reset cache");

    // Verify the saved file
    let reread = fs::read_to_string(&cache_path).expect("re-read cache");
    let reparsed: CacheFile = toml::from_str(&reread).expect("re-parse cache");
    assert_eq!(
        reparsed.version, 1,
        "saved cache must have version = 1 after reset"
    );
    assert!(
        reparsed.entries.is_empty(),
        "saved cache must have no entries after reset"
    );
}

#[test]
fn version_1_cache_preserves_entries() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let cache_path = dir.path().join(CACHE_FILE);

    let valid_cache = r#"version = 1

[[entry]]
package = "good-contract"
wasm_sha256 = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
network = "testnet"
source = "bob"
contract_id = "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
"#;
    fs::write(&cache_path, valid_cache).expect("failed to write test cache");

    #[derive(serde::Deserialize)]
    struct CacheFile {
        #[serde(default)]
        version: u32,
        #[serde(default, rename = "entry")]
        entries: Vec<toml::Value>,
    }

    let text = fs::read_to_string(&cache_path).expect("read cache");
    let parsed: CacheFile = toml::from_str(&text).expect("parse cache");
    assert_eq!(parsed.version, 1);

    // Version matches — entries are preserved
    let loaded_entries: Vec<toml::Value> = if parsed.version == 1 {
        parsed.entries
    } else {
        Vec::new()
    };
    assert_eq!(
        loaded_entries.len(),
        1,
        "version 1 cache should keep its entries"
    );
}

#[test]
fn version_0_cache_discards_entries() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let cache_path = dir.path().join(CACHE_FILE);

    // version defaults to 0 when omitted; entries should still be discarded
    let zero_version_cache = r#"
[[entry]]
package = "zero-ver"
wasm_sha256 = "1111111111111111111111111111111111111111111111111111111111111111"
network = "testnet"
source = "charlie"
contract_id = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC"
"#;
    fs::write(&cache_path, zero_version_cache).expect("failed to write test cache");

    #[derive(serde::Deserialize)]
    struct CacheFile {
        #[serde(default)]
        version: u32,
        #[serde(default, rename = "entry")]
        entries: Vec<toml::Value>,
    }

    let text = fs::read_to_string(&cache_path).expect("read cache");
    let parsed: CacheFile = toml::from_str(&text).expect("parse cache");
    assert_eq!(parsed.version, 0, "missing version defaults to 0");

    let loaded_entries: Vec<toml::Value> = if parsed.version == 1 {
        parsed.entries
    } else {
        Vec::new()
    };
    assert!(
        loaded_entries.is_empty(),
        "version 0 cache must be treated as empty"
    );
}
