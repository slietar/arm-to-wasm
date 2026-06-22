#![no_std]

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

#[unsafe(no_mangle)]
pub extern "C" fn increment(x: i32) -> i32 {
    (x + 1) * 2
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
