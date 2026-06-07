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
    translator::{Translator, get_arm_operand},
};

const PAGE_SIZE: u32 = 65_536;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = Module::new();

    let path = std::path::PathBuf::from("../example/target/debug/example");
    let file_data = std::fs::read(path).expect("Could not read file.");

    let file = ElfBytes::<AnyEndian>::minimal_parse(&file_data)
        .map_err(|err| format!("Failed to parse ELF file: {err}"))?;

    if file.ehdr.osabi != elf::abi::ELFOSABI_NONE && file.ehdr.osabi != elf::abi::ELFOSABI_LINUX {
        return Err(format!(
            "Unsupported OS ABI: expected Linux (ELFOSABI_LINUX), found {}",
            file.ehdr.osabi
        )
        .into());
    }

    if file.ehdr.e_machine != elf::abi::EM_AARCH64 {
        return Err(format!(
            "Unsupported architecture: expected AArch64 (EM_AARCH64), found {}",
            file.ehdr.e_machine
        )
        .into());
    }

    // eprintln!("Entry address: 0x{:x}", file.ehdr.e_entry);

    // let mut instructions = Vec::new();

    // Initialize memory

    #[derive(Debug)]
    struct MappedSegment<'a> {
        address: u64,
        data: &'a [u8],
        offset: u64,
        size: u64,
    }

    let mut current_offset = 0;
    let mut mapped_segments = Vec::new();

    for segment in file.segments().unwrap() {
        // eprintln!("Segment: {:?}", segment);

        if segment.p_type == elf::abi::PT_LOAD {
            // let is_executable = (segment.p_flags & elf::abi::PF_X) != 0;
            // if is_executable {
            //     eprintln!("Found executable segment at 0x{:x} with {} bytes", segment.p_offset, segment.p_filesz);
            // }

            // eprintln!("{} {}", segment.p_filesz, file_data[(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize].len());

            mapped_segments.push(MappedSegment {
                address: segment.p_vaddr,
                data: &file_data[(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize],
                offset: current_offset,
                size: segment.p_filesz,
            });

            // eprintln!("{:?}", mapped_segments.last().unwrap().data);

            current_offset += segment.p_filesz;
        }
    }

    // mapped_segments.clear();
    // eprintln!("Mapped segments: {:#?}", mapped_segments);

    let total_mapped_size = current_offset as u32;

    let loaded_memory_name = CString::new("emul_mem").unwrap();

    let segment_names = mapped_segments
        .iter()
        .enumerate()
        .map(|(i, _)| CString::new(format!("segment_{}", i)).unwrap())
        .collect::<Vec<_>>();

    let mut segment_name_ptrs = segment_names
        .iter()
        .map(|s| s.as_ptr())
        .collect::<Vec<_>>();

    let mut segment_datas = mapped_segments
        .iter()
        .map(|seg| seg.data.as_ptr())
        .collect::<Vec<_>>();

    let mut segment_passives = vec![false; mapped_segments.len()];

    let mut segment_offsets = mapped_segments
        .iter()
        .map(|seg| unsafe {
            by::BinaryenConst(
                module.by_module,
                by::BinaryenLiteralInt64(seg.offset as i64),
            )
        })
        .collect::<Vec<_>>();

    let mut segment_sizes = mapped_segments
        .iter()
        .map(|seg| seg.size as u32)
        .collect::<Vec<_>>();

    unsafe {
        by::BinaryenSetMemory(
            module.by_module,
            total_mapped_size.div_ceil(PAGE_SIZE) as u32,
            i32::cast_unsigned(-1),
            loaded_memory_name.as_ptr(),
            segment_name_ptrs.as_mut_ptr() as *mut *const i8,
            segment_datas.as_mut_ptr() as *mut *const i8,
            segment_passives.as_mut_ptr(),
            segment_offsets.as_mut_ptr(),
            segment_sizes.as_mut_ptr(),
            mapped_segments.len() as u32,
            false,
            true,
            loaded_memory_name.as_ptr(),
        );
    }

    eprintln!("Total mapped size: {} bytes", total_mapped_size);

    // Find instructions

    let (section_headers_opt, section_name_table_opt) = file
        .section_headers_with_strtab()
        .map_err(|err| format!("Failed to read section headers: {err}"))?;

    let section_headers =
        section_headers_opt.ok_or_else(|| "ELF has no section headers".to_string())?;
    let section_name_table =
        section_name_table_opt.ok_or_else(|| "ELF has no section name string table".to_string())?;

    let mut instruction_bytes = Vec::new();

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

        instruction_bytes.extend_from_slice(data);
    }

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
    eprintln!("Found {} instructions", instructions.len());

    let mut jump_targets = Vec::new();

    for (instruction_index, instruction) in instructions.as_ref().iter().enumerate() {
        let detail: InsnDetail = cs.insn_detail(&instruction).unwrap();
        let arch_detail = detail.arch_detail();
        let ops = arch_detail.operands();

        if instruction.mnemonic().unwrap() == "bl" {
            if let capstone::arch::arm64::Arm64OperandType::Imm(imm) =
                get_arm_operand(&ops[0]).op_type
            {
                jump_targets.push(instruction_index + (imm as usize));
            }
        }
    }

    eprintln!("Identified jump targets: {:#x?}", jump_targets);

    exprs.push(translator.setup());

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

    let relooped_block = unsafe { by::RelooperAddBlock(relooper, block) };

    let expr = unsafe { by::RelooperRenderAndDispose(relooper, relooped_block, 0) };

    let mut var_types = translator.var_types();

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

    let mut syscall_handler_params = unsafe {
        [
            by::BinaryenTypeInt32(),
            by::BinaryenTypeInt64(),
            by::BinaryenTypeInt64(),
            by::BinaryenTypeInt64(),
            by::BinaryenTypeInt64(),
            by::BinaryenTypeInt64(),
            by::BinaryenTypeInt64(),
            by::BinaryenTypeInt64(),
        ]
    };

    let external_module_name = CString::new("env").unwrap();
    let syscall_handler_name = CString::new("syscall_handler").unwrap();

    let _import = unsafe {
        by::BinaryenAddFunctionImport(
            module.by_module,
            syscall_handler_name.as_ptr(),
            external_module_name.as_ptr(),
            syscall_handler_name.as_ptr(),
            by::BinaryenTypeCreate(
                syscall_handler_params.as_mut_ptr(),
                syscall_handler_params.len() as u32,
            ),
            by::BinaryenTypeInt64(),
        )
    };

    let mut output_file = File::create("output.wasm")?;

    // module.optimize();
    module.print();
    module.save(&mut output_file)?;

    Ok(())
}
