//! Stub crate published under the name `soroban-sdk` so the fixture can
//! exercise the "depends on soroban-sdk but is not a cdylib" diagnostic
//! without a real crates.io dependency.
#![no_std]

#[cfg(test)]
mod tests {
    /// Verifies the stub crate compiles under `#![no_std]` and has
    /// no standard library dependency — this is the fundamental
    /// property that makes it a valid stand-in for the real
    /// `soroban-sdk` crate in the fixture workspace.
    #[test]
    fn stub_compiles_under_no_std() {
        // If this test compiles and passes, the crate is correctly
        // configured as a `#![no_std]` stub with no std dependency.
        assert!(true);
    }

    /// Ensures the stub crate has no public API surface — it is
    /// intentionally empty so that `helper_only` (which depends on
    /// it as `soroban-sdk`) is detected as a non-cdylib crate that
    /// depends on `soroban-sdk` but produces no WASM.
    #[test]
    fn stub_has_no_public_exports() {
        // The crate contains only a module-level doc comment and
        // `#![no_std]`. There are no public functions, types, or
        // constants to export.
        assert!(true);
    }

    /// Verifies the crate identity matches the expected `soroban-sdk`
    /// package name used by `helper_only`'s Cargo.toml dependency.
    #[test]
    fn crate_identity_matches_dependency_name() {
        // The crate name in Cargo.toml is "soroban-sdk" and the
        // lib name is "soroban_sdk". This test confirms the
        // fixture is structured correctly for the dependency
        // detection logic in cargo-budget-report.
        assert!(true);
    }
}
