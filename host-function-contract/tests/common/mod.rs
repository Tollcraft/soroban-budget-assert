//! Shared helper for the host-function-contract integration tests.
//!
//! The tests exercise the contract as WASM — not just as native Rust — so the
//! fixture is verified to build for the `wasm32v1-none` target used by the
//! rest of the workspace (issue #480). This module resolves and loads the
//! pre-built `host_function_contract.wasm` artifact, building it on demand
//! when it is missing or stale so the tests run as part of `cargo test
//! --workspace` with no extra setup.
//!
//! The artifact path honours `CARGO_TARGET_DIR` when it is set and otherwise
//! falls back to the workspace-root `target/` directory.

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
static BUILD_LOCK: Mutex<()> = Mutex::new(());

/// The workspace root: this crate's manifest directory sits directly under it.
fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("host-function-contract manifest must sit directly under the workspace root")
}

/// Absolute path to the `host_function_contract.wasm` artifact for `target`,
/// honouring `CARGO_TARGET_DIR` when it is set.
pub fn wasm_path(target: &str) -> PathBuf {
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target"));
    target_dir.join(target).join("release").join(WASM_FILE_NAME)
}

/// Loads the `host_function_contract.wasm` bytes for `target`, building the
/// artifact first when it is missing or stale.
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
fn artifact_needs_rebuild(wasm: &Path) -> bool {
    let Ok(wasm_meta) = std::fs::metadata(wasm) else {
        return true;
    };
    let Ok(wasm_mtime) = wasm_meta.modified() else {
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
