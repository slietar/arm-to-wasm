#![no_std]
#![no_main]

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use core::arch::asm;

// trait A {
//     fn run(&self) -> i32;
// }

// struct B;

// impl A for B {
//     fn run(&self) -> i32 {
//         42
//     }
// }

// fn foobar(
//     a0: i32,
//     a1: i32,
//     a2: i32,
//     a3: i32,
//     a4: i32,
//     a5: i32,
//     a6: i32,
//     a7: i32,
//     a8: i32,
//     a9: i32,
//     a10: i32,
// ) {

// }

// pub fn main() {
//     foobar(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11);
// }

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    compare(13, 13);
    exit(3);
}

pub fn compare(a: i32, b: i32) {
    if a < b {
        write_text(1, "a < b\n");
    } else if a > b {
        write_text(1, "a > b\n");
    } else {
        write_text(1, "a == b\n");
    }
}

fn exit(code: i32) /* -> ! */ {
    unsafe {
        asm!(
            "mov x8, #93",
            "mov x0, {code}",
            "svc #0",
            code = in(reg) code,
            out("x0") _,
            out("x8") _,
        );
    }
}

fn write_text(fd: i32, text: &str) {
    let bytes = text.as_bytes();
    write(fd, bytes.as_ptr(), bytes.len());
}

fn write(fd: i32, buf: *const u8, len: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "mov x8, #64",
            "mov x0, {fd}",
            "mov x1, {buf}",
            "mov x2, {len}",
            "svc #0",
            fd = in(reg) fd,
            buf = in(reg) buf,
            len = in(reg) len,
            out("x0") ret,
            out("x8") _,
        );
    }
    ret
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
