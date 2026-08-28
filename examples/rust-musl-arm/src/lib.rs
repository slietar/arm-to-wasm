// #![no_std]

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

// unsafe extern "C" {
//     fn add(x: i32, y: i32) -> i32;
// }

#[unsafe(no_mangle)]
pub extern "C" fn increment(x: i32) -> i32 {
    // (x + 1) * 2
    println!("Incrementing {} to {}", x, x + 1);
    decrement(x + 2)
}

// #[unsafe(no_mangle)]
pub extern "C" fn decrement(x: i32) -> i32 {
    println!("Decrementing {} to {}", x, x - 1);
    x - 1
}

// #[panic_handler]
// fn panic(_info: &core::panic::PanicInfo) -> ! {
//     loop {}
// }
