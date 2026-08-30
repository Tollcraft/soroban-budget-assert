//! A cdylib whose only export is a toolchain-style `_`-prefixed symbol
//! -> ExportScan::OnlyRuntimeSymbols.
#![no_std]

#[no_mangle]
pub extern "C" fn _start() {}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
