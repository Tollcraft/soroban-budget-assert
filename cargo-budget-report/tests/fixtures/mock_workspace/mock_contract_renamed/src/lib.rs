//! Minimal `no_std` WASM export used by `cargo-budget-report`'s integration
//! tests. This contract's `[lib] name = "custom_lib_name"` differs from its
//! package name (`mock-contract-renamed`) to verify that `cargo-budget-report`
//! discovers the WASM via the cdylib target name rather than a string transform
//! of the package name.
#![no_std]

#[no_mangle]
pub extern "C" fn greet() -> i64 {
    42
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
