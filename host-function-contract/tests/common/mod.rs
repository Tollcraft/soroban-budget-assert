//! Shared helper for the host-function-contract integration tests (#480, #717).
//!
//! # Why this module exists
//!
//! The tests that exercise the benchmark fixture do not only run it as native
//! Rust: they also register the pre-built `host_function_contract.wasm` artifact
//! into the test environment, so the fixture is verified against the same
//! `wasm32v1-none` target the rest of the workspace uses for budget
//! measurement. Trusting a stale or hand-built artifact would let the tests
//! pass against code that is no longer in the tree, so this module resolves the
//! artifact path and loads the bytes, building the artifact on demand whenever
//! it is missing or older than the contract sources.
//!
//! # How it fits together
//!
//! 1. [`workspace_root`] locates the workspace root from this crate's
//!    `CARGO_MANIFEST_DIR` (the crate sits directly under the root).
//! 2. [`wasm_path`] turns a target triple into the expected artifact path under
//!    `target/<target>/release/`, honouring `CARGO_TARGET_DIR` when set.
//! 3. [`artifact_needs_rebuild`] compares the artifact's mtime against the
//!    contract sources in [`STALE_SOURCES`].
//! 4. [`load_contract_wasm`] serialises the check-then-build sequence under
//!    [`BUILD_LOCK`] and returns the artifact bytes.
//!
//! The artifact path honours `CARGO_TARGET_DIR` when it is set and otherwise
//! falls back to the workspace-root `target/` directory, so the helper works
//! from any test working directory and with any custom target-dir setup.

// Each integration-test binary in this directory declares `mod common;` and
// uses only the helpers it needs, so the rest can look unused to a single
// binary. Same idiom as `amm-pool-contract/tests/common/mod.rs`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, PoisonError};

/// WASM artifact file name produced by building the `host-function-contract`
/// cdylib for the `wasm32v1-none` target.
const WASM_FILE_NAME: &str = "host_function_contract.wasm";

/// Contract source files whose changes can make a previously built artifact
/// stale. When any of them is newer than the artifact, the artifact is
/// rebuilt before the tests read it.
const STALE_SOURCES: &[&str] = &["src/lib.rs", "Cargo.toml"];

/// Serialises the check-then-build sequence so tests running in parallel
/// inside one test binary never race on the same artifact.
///
/// Without this, two tests could both observe a missing artifact and spawn
/// concurrent `cargo build` invocations, or one could read the file while the
/// other is truncating it.
static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// The workspace root: this crate's manifest directory sits directly under it.
///
/// `CARGO_MANIFEST_DIR` is expanded at compile time, so this requires no
/// runtime environment lookup and cannot fail for a missing variable.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("host-function-contract manifest must sit directly under the workspace root")
}

/// Absolute path to the `host_function_contract.wasm` artifact for `target`,
/// honouring `CARGO_TARGET_DIR` when it is set.
///
/// The returned path is not guaranteed to exist: callers that need the bytes
/// should use [`load_contract_wasm`] instead.
pub fn wasm_path(target: &str) -> PathBuf {
    // Mirror Cargo's own precedence: an explicit `CARGO_TARGET_DIR` wins,
    // otherwise the workspace-root `target/` directory is used.
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"));
    target_dir.join(target).join("release").join(WASM_FILE_NAME)
}

/// Loads the `host_function_contract.wasm` bytes for `target`, building the
/// artifact first when it is missing or stale.
///
/// The check and the build happen under [`BUILD_LOCK`], so concurrent callers
/// observe either a complete artifact or a single build, never a partial write.
///
/// # Panics
///
/// Panics with a clear message if the build or the read fails.
pub fn load_contract_wasm(target: &str) -> Vec<u8> {
    let _guard = BUILD_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let path = wasm_path(target);
    if artifact_needs_rebuild(&path) {
        build_contract_wasm(target);
    }
    std::fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "failed to read WASM artifact at {}: {err} \
             (a `cargo build` was attempted first; check its output above)",
            path.display()
        )
    })
}

/// Whether the artifact at `wasm` is missing or older than the contract
/// sources it was built from.
///
/// A missing artifact or an artifact whose mtime cannot be read is treated as
/// stale, so the caller rebuilds. A source whose mtime cannot be read is
/// skipped: an unreadable source cannot make a readable artifact stale.
fn artifact_needs_rebuild(wasm: &Path) -> bool {
    let Ok(wasm_meta) = std::fs::metadata(wasm) else {
        // Artifact does not exist yet — must build.
        return true;
    };
    let Ok(wasm_mtime) = wasm_meta.modified() else {
        // The platform cannot report an mtime — rebuild rather than risk
        // trusting an artifact we cannot compare.
        return true;
    };
    STALE_SOURCES.iter().any(|source| {
        let source_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(source);
        match std::fs::metadata(source_path).and_then(|meta| meta.modified()) {
            Ok(source_mtime) => source_mtime > wasm_mtime,
            // A missing source file cannot make the artifact stale; skip it.
            Err(_) => false,
        }
    })
}

/// Builds the `host-function-contract` cdylib for `target` in release mode.
///
/// Uses the `CARGO` executable Cargo set for this build (`env!("CARGO")`)
/// rather than assuming `cargo` is on `PATH`, so the correct toolchain is used
/// even when the tests are launched through a rustup proxy.
///
/// # Panics
///
/// Panics if the build command cannot be spawned or exits unsuccessfully.
fn build_contract_wasm(target: &str) {
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "host-function-contract",
            "--release",
            "--target",
            target,
        ])
        .status()
        .expect("failed to spawn `cargo build` for the host-function-contract WASM");
    assert!(
        status.success(),
        "`cargo build -p host-function-contract --release --target {target}` failed"
    );
}
