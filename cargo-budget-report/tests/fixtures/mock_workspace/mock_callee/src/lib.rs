//! Minimal `no_std` WASM export used by `cargo-budget-report`'s integration
//! tests. This contract represents a callee that another contract calls.
//! Its exported function accepts an "other" address parameter (simulating
//! a cross-contract call pattern in the arg structure) so that the mock
//! workspace can test `{contract:...}` placeholder resolution.
#![no_std]

#[no_mangle]
pub extern "C" fn do_cross_contract_work(_other: i64, _n: i64) -> i64 {
    42
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
