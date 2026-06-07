#![allow(dead_code)]
#![allow(unused_variables)]

mod module;
mod translator;
use binaryen::ffi as by;

use std::{ffi::CString, fs::File};

use capstone::prelude::*;
use elf::{ElfBytes, abi::SHF_EXECINSTR, endian::AnyEndian};

use crate::{
    module::Module,
    translator::{Translator, get_arm_operand, translate},
};

const INSTRUCTION_SIZE: u64 = 4;
const PAGE_SIZE: u32 = 65_536;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = Module::new();

    let path = std::path::PathBuf::from("../example/target/debug/example");
    let elf_bytes = std::fs::read(path).expect("Could not read file.");

    let translator = translate(&elf_bytes)?;

    Ok(())
}
