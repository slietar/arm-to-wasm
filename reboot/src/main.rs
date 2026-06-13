#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

mod analysis;
mod decoding;
mod instructions;
mod module;
mod translator;

use std::path::PathBuf;

use crate::{module::Module, translator::translate};

const INSTRUCTION_SIZE: u64 = 4;
const PAGE_SIZE: u32 = 65_536;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = Module::new();

    let path = PathBuf::from("../example/target/aarch64-unknown-none/debug/example");
    // let path = PathBuf::from("/Users/simon/Developer/arm-to-wasm/runtime/target/debug/runtime");
    let elf_bytes = std::fs::read(path).expect("Could not read file.");

    // let translator = translate(&elf_bytes)?;

    analysis::main_analyze(&elf_bytes)?;
    // instructions::decode_file(&elf_bytes)?;

    Ok(())
}
