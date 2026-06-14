#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

mod analysis;
mod constants;
mod decoding;
mod instructions;
mod module;
mod translation;
mod instruction_helper;
// mod translator;

use std::path::PathBuf;

const INSTRUCTION_SIZE: u64 = 4;
const PAGE_SIZE: u32 = 65_536;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from("../example/target/aarch64-unknown-none/debug/example");
    // let path = PathBuf::from("/Users/simon/Developer/arm-to-wasm/runtime/target/debug/runtime");
    let elf_bytes = std::fs::read(path).expect("Could not read file.");

    // let translator = translate(&elf_bytes)?;

    // analysis::main_analyze(&elf_bytes)?;
    // instructions::decode_file(&elf_bytes)?;
    translation::translate(&elf_bytes)?;

    Ok(())
}
