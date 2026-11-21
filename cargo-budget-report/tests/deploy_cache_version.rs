//! A cache written by a newer schema version is discarded, never migrated
//! (issue #1044).
//!
//! `DeployCache::load` trusts a file only when its `version` equals
//! `CACHE_VERSION` (currently `1`); any other version starts a fresh empty
//! cache. These tests pin that invalidation and the subsequent save, so
//! accidentally introducing migration logic is caught by CI.
//!
//! `cargo-budget-report` is a binary-only crate, so the module is compiled
//! into this test directly rather than through a library import.

#[path = "../src/deploy_cache.rs"]
mod deploy_cache;

use deploy_cache::{DeployCache, CACHE_FILE};
use std::fs;

const FRESH_ID: &str = "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";

const VERSION_2_CACHE: &str = "\
version = 2

[[entry]]
package = \"amm-pool-contract\"
wasm_sha256 = \"stale-hash\"
network = \"testnet\"
source = \"alice\"
contract_id = \"CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\"
";

#[test]
fn advanced_version_discards_entries_and_saves_as_version_1() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = dir.path().join(CACHE_FILE);
    fs::write(&path, VERSION_2_CACHE).expect("failed to write the stale cache");

    // Load: the version mismatch is a cold cache — the stale entry is not
    // migrated or exposed.
    let mut cache = DeployCache::load(dir.path());
    assert_eq!(
        cache.get("stale-hash", "testnet", "alice"),
        None,
        "entries from an unrecognised cache version must be discarded"
    );

    // Save: the rewritten file carries the current version and only entries
    // recorded by this build.
    cache.put(
        "amm-pool-contract",
        "fresh-hash",
        "testnet",
        "alice",
        FRESH_ID,
    );
    cache
        .save()
        .expect("saving the reloaded cache should succeed");

    let rewritten = fs::read_to_string(&path).expect("cache file should exist after save");
    assert!(
        rewritten.contains("version = 1"),
        "save must rewrite the file with CACHE_VERSION = 1:\n{rewritten}"
    );
    assert!(
        !rewritten.contains("version = 2"),
        "the stale version marker must be gone:\n{rewritten}"
    );

    let reloaded = DeployCache::load(dir.path());
    assert_eq!(
        reloaded.get("stale-hash", "testnet", "alice"),
        None,
        "the stale entry must not survive a save after invalidation"
    );
    assert_eq!(
        reloaded.get("fresh-hash", "testnet", "alice"),
        Some(FRESH_ID)
    );
}

#[test]
fn a_load_that_is_never_written_leaves_the_file_untouched() {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let path = dir.path().join(CACHE_FILE);
    fs::write(&path, VERSION_2_CACHE).expect("failed to write the stale cache");

    let mut cache = DeployCache::load(dir.path());
    // Not dirty: `save` is a no-op, so a measurement run that never deploys
    // anything does not rewrite (or later mislabel) the stale file.
    cache
        .save()
        .expect("saving an unmodified cache should succeed");
    let unchanged = fs::read_to_string(&path).expect("cache file should still exist");
    assert_eq!(unchanged, VERSION_2_CACHE);
    assert!(cache.get("stale-hash", "testnet", "alice").is_none());
}
