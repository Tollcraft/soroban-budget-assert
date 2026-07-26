//! See `mock_contract_a/src/lib.rs` for why this crate is a bare `no_std`
//! export rather than a real Soroban contract.
#![no_std]

#[no_mangle]
pub extern "C" fn pong() -> i64 {
    2
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
