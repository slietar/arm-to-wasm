#![allow(dead_code)]
#![allow(unused_variables)]

mod module;
mod translator;
use binaryen::ffi as by;

use std::{ffi::CString, fs::File};

use capstone::prelude::*;
use elf::{ElfBytes, abi::SHF_EXECINSTR, endian::AnyEndian};

use crate::{module::Module, translator::{REGISTER_COUNT, Translator}};

fn extract_instructions(file_data: &[u8]) -> Result<Vec<u8>, String> {
    let file = ElfBytes::<AnyEndian>::minimal_parse(file_data)
        .map_err(|err| format!("Failed to parse ELF file: {err}"))?;

    if file.ehdr.osabi != elf::abi::ELFOSABI_NONE && file.ehdr.osabi != elf::abi::ELFOSABI_LINUX {
        return Err(format!(
            "Unsupported OS ABI: expected Linux (ELFOSABI_LINUX), found {}",
            file.ehdr.osabi
        ));
    }

    if file.ehdr.e_machine != elf::abi::EM_AARCH64 {
        return Err(format!(
            "Unsupported architecture: expected AArch64 (EM_AARCH64), found {}",
            file.ehdr.e_machine
        ));
    }

    // let mut instructions = Vec::new();

    // for segment in file.segments().unwrap() {
    //     eprintln!("Segment: {:?}", segment);

    //     if segment.p_type == elf::abi::PT_LOAD && (segment.p_flags & elf::abi::PF_X) != 0 {
    //         instructions.extend_from_slice(&file_data[(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize]);
    //         eprintln!("Found executable segment with {} bytes of instructions", segment.p_filesz);
    //     }
    // }

    let (section_headers_opt, section_name_table_opt) = file
        .section_headers_with_strtab()
        .map_err(|err| format!("Failed to read section headers: {err}"))?;

    let section_headers =
        section_headers_opt.ok_or_else(|| "ELF has no section headers".to_string())?;
    let section_name_table =
        section_name_table_opt.ok_or_else(|| "ELF has no section name string table".to_string())?;

    let mut instructions = Vec::new();

    for header in section_headers.iter() {
        let is_executable = (header.sh_flags & SHF_EXECINSTR as u64) != 0;
        if !is_executable {
            continue;
        }

        let section_name = section_name_table
            .get(header.sh_name as usize)
            .unwrap_or("<invalid-section-name>");

        let (data, _) = file
            .section_data(&header)
            .map_err(|err| format!("Failed to read section '{section_name}': {err}"))?;

        instructions.extend_from_slice(data);
    }

    if instructions.is_empty() {
        return Err("No executable sections found in ELF".to_string());
    }

    Ok(instructions)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = Module::new();

    let path = std::path::PathBuf::from("../example/target/debug/example");
    let file_data = std::fs::read(path).expect("Could not read file.");

    let instruction_bytes = extract_instructions(file_data.as_slice())?;

    println!("Extracted {} instruction bytes", instruction_bytes.len());

    // Loop through instructions

    let cs = Capstone::new()
        .arm64()
        .mode(capstone::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .unwrap();

    let translator = Translator {
        module: module.by_module,
    };

    let instructions = cs.disasm_all(&instruction_bytes, 0x1000).unwrap();
    let mut exprs = Vec::new();

    for instruction in instructions.as_ref() {
        let x = translator.translate(&cs, &instruction);
        exprs.push(x);
    }

    let block = unsafe {
        by::BinaryenBlock(
            module.by_module,
            "block".as_ptr() as *const i8,
            exprs.as_mut_ptr(),
            exprs.len() as u32,
            by::BinaryenTypeNone(),
        )
    };

    // Run the relooper

    let relooper = unsafe { by::RelooperCreate(module.by_module) };

    let relooped_block = unsafe {
        by::RelooperAddBlock(
            relooper,
            block,
        )
    };

    let expr = unsafe { by::RelooperRenderAndDispose(relooper, relooped_block, 0) };

    let mut var_types = unsafe { vec![by::BinaryenTypeInt64()] };
    var_types.extend((0..REGISTER_COUNT).map(|_| unsafe { by::BinaryenTypeInt64() }));

    let main_func_name = CString::new("main").unwrap();

    let main_func = unsafe {
        by::BinaryenAddFunction(
            module.by_module,
            main_func_name.as_ptr(),
            by::BinaryenTypeNone(),
            by::BinaryenTypeNone(),
            var_types.as_mut_ptr(),
            var_types.len() as u32,
            expr,
        )
    };

    // Create entry function

    let entry_func_body = unsafe {
        by::BinaryenCall(
            module.by_module,
            main_func_name.as_ptr(),
            [].as_mut_ptr(),
            0,
            by::BinaryenTypeNone(),
        )
    };

    let entry_func_name = CString::new("_entry").unwrap();

    let _entry_func = unsafe {
        by::BinaryenAddFunction(
            module.by_module,
            entry_func_name.as_ptr(),
            by::BinaryenTypeNone(),
            by::BinaryenTypeNone(),
            [].as_mut_ptr(),
            0,
            entry_func_body,
        )
    };

    let _export = unsafe {
        by::BinaryenAddExport(
            module.by_module,
            entry_func_name.as_ptr(),
            entry_func_name.as_ptr(),
        )
    };

    let mut output_file = File::create("output.wasm")?;

    // module.optimize();
    module.print();
    module.save(&mut output_file)?;

    Ok(())
}
