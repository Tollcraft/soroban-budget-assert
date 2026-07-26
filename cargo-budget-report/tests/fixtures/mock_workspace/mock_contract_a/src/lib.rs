//! Minimal `no_std` WASM export used by `cargo-budget-report`'s integration
//! tests. It carries no Soroban SDK or other dependency so that the mock
//! workspace builds near-instantly; `cargo-budget-report` only needs a valid
//! `cdylib` WASM binary with a named export, it does not require the export
//! to be a real Soroban contract function.
#![no_std]

#[no_mangle]
pub extern "C" fn ping() -> i64 {
    1
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
