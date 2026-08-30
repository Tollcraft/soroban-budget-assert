//! A cdylib with no function exports at all -> ExportScan::NoFunctionExports.
#![no_std]

fn _internal() -> i64 { 1 }

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
