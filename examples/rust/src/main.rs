#![no_std]
#![no_main]

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

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    let a = 5;

    if sum(a, 3) == 15 {
        exit(0);
    }

    // loop {
    //     x[i] = 234;

    //     if i == 15 {
    //         break;
    //     }

    //     i += 1;
    // }
}

fn sum(a: i32, b: i32) -> i32 {
    a
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

    // loop {}
}

// fn b() -> i32 {
//     0
// }

// fn c() -> i32 {
//     b()
// }

// #[inline(always)]
// fn a() {
//     // let x = b"Hello, world!\n";
//     let msg = b"Hello, world!\n";
//     // let p = 3u8;
//     // let q = 4u8;

//     let mut x = c();

//     while x < 3 {
//         x += 1;
//     }

//     // for i in 0..6 {
//     //     x += i;
//     // }

//     // let b: &dyn A = &B;
//     // b.run();

//     unsafe {
//         // write(1, msg, len) — Linux AArch64 syscall 64
//         asm!(
//             "mov x8, #64",
//             "mov x0, #1",
//             "mov x1, {buf}",
//             "mov x2, {len}",
//             "svc #0",
//             buf = in(reg) msg.as_ptr(),
//             len = in(reg) msg.len(),
//             out("x0") _,
//             out("x8") _,
//         );

//         // exit(0) — Linux AArch64 syscall 93
//         // asm!(
//         //     "mov x8, #93",
//         //     "mov x0, #0",
//         //     "svc #0",
//         //     out("x0") _,
//         //     out("x8") _,
//         // );

//         // let x = 5;
//         exit(x);
//     }
// }

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
