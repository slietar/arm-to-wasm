#![allow(unused)]

mod decoding;
mod instructions;
mod structures;
mod utilities;

use crate::decoding::logical::decode_bitmask;

fn main() {
    eprintln!("{:064b}", decode_bitmask(true, 0b111_000, 0b000_000, structures::SizeVariant::Reg64));
}
