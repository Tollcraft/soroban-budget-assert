//! An rlib that depends on soroban-sdk but is not a cdylib: the tool
//! should say so rather than skip it in silence.
//!
//! This fixture is used to verify that `cargo-budget-report` correctly
//! identifies a workspace crate that declares `soroban-sdk` as a dependency
//! but lacks `crate-type = ["cdylib"]` in its `[lib]` section. Such a crate
//! produces no WASM binary and is skipped with a diagnostic message.
#![no_std]

/// Returns whether this crate is an rlib (not a cdylib).
///
/// This is a compile-time constant that reflects the fixture's
/// configuration: `helper-only` is built as an `rlib` per its
/// `Cargo.toml`, which is what triggers the `not_a_cdylib` diagnostic
/// in `cargo-budget-report`.
pub const IS_RLIB: bool = true;

/// The name of the dependency on `soroban-sdk` as declared in
/// `Cargo.toml`. This is used by the fixture's test suite to verify
/// that the dependency is correctly detected by the reporting tool.
pub const SOROBAN_SDK_DEPENDENCY: &str = "soroban-sdk";

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that the crate is correctly configured as an rlib.
    #[test]
    fn is_rlib() {
        assert!(IS_RLIB, "helper-only must be an rlib");
    }

    /// Verifies that the dependency name matches what `cargo-budget-report`
    /// expects when scanning for `soroban-sdk` dependencies.
    #[test]
    fn dependency_name_matches() {
        assert_eq!(
            SOROBAN_SDK_DEPENDENCY,
            "soroban-sdk",
            "dependency name must match the expected `soroban-sdk` package name"
        );
    }
}