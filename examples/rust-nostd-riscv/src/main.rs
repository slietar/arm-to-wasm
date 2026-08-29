#![no_std]
#![no_main]

use core::arch::asm;

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    // compare(13, 13);
    exit(3);
}

#[inline(never)]
fn exit(code: i32) /* -> ! */ {
    unsafe {
        asm!(
            "li a7, 93",
            "mv a0, {code}",
            "ecall",
            code = in(reg) code,
            out("a0") _,
            out("a7") _,
        );
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
