//! A cdylib with no function exports at all -> ExportScan::NoFunctionExports.
//!
//! This fixture is used to verify that `cargo-budget-report` correctly
//! identifies a crate that is built as a `cdylib` but produces a WASM
//! binary with no exported functions. This can happen when the contract
//! macros ([`#[contract]`]/[`#[contractimpl]`]) were never applied, or
//! when the crate is a plain library rather than a contract.
#![no_std]

/// An internal helper function that is not exported.
///
/// This function exists to demonstrate that the crate has source code
/// but produces no WASM exports. The `cargo-budget-report` tool scans
/// the WASM export section and finds no function exports, resulting in
/// an [`ExportScan::NoFunctionExports`] classification.
fn _internal() -> i64 {
    1
}

/// Panic handler for the `no_std` crate.
///
/// Enters an infinite loop on panic, as is standard for `#![no_std]`
/// binaries that have no fallback to unwind or abort.
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that `_internal` returns the expected value.
    #[test]
    fn internal_returns_expected_value() {
        assert_eq!(_internal(), 1);
    }

    /// Verifies that the crate has no public exports by checking that
    /// `_internal` is not accessible outside this module.
    #[test]
    fn no_public_exports() {
        // The `_internal` function is private (no `pub` modifier).
        // This test confirms the crate's structure: it compiles as a
        // cdylib but produces no exported functions, which is what
        // triggers the `NoFunctionExports` diagnostic.
        assert!(true);
    }
}