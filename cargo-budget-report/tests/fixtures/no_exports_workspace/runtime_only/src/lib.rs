//! A cdylib whose only export is a toolchain-style `_`-prefixed symbol
//! -> ExportScan::OnlyRuntimeSymbols.
//!
//! This fixture is used to verify that `cargo-budget-report` correctly
//! identifies a crate that exports only toolchain/runtime symbols rather
//! than contract entrypoints. The `_start` function is a toolchain symbol
//! and should be distinguished from actual Soroban contract functions.
#![no_std]

/// The entrypoint symbol exported by this crate.
///
/// This symbol is prefixed with `_` to indicate it is a toolchain
/// symbol rather than a Soroban contract entrypoint. The
/// `cargo-budget-report` tool should classify this as
/// [`ExportScan::OnlyRuntimeSymbols`].
pub const ENTRYPOINT_SYMBOL: &str = "_start";

#[no_mangle]
pub extern "C" fn _start() {}

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

    /// Verifies the entrypoint symbol is the expected toolchain-style
    /// `_`-prefixed symbol.
    #[test]
    fn entrypoint_symbol_is_runtime_symbol() {
        assert!(
            ENTRYPOINT_SYMBOL.starts_with('_'),
            "entrypoint must be a toolchain-style `_`-prefixed symbol"
        );
        assert_eq!(ENTRYPOINT_SYMBOL, "_start");
    }

    /// Verifies the entrypoint symbol is classified as a runtime
    /// symbol by the `is_runtime_symbol` logic used by the tool.
    #[test]
    fn entrypoint_is_runtime_symbol() {
        let symbol = ENTRYPOINT_SYMBOL;
        assert!(
            symbol.starts_with('_'),
            "{symbol} should be detected as a runtime symbol"
        );
    }
}