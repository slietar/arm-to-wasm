#![allow(dead_code)]
#![allow(unused_variables)]

mod module;
mod translator;
use binaryen::ffi as by;

use std::{ffi::CString, fs::File};

use capstone::prelude::*;
use elf::{ElfBytes, abi::SHF_EXECINSTR, endian::AnyEndian};

use crate::module::Module;

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

fn get_arm_operand(operand: &arch::ArchOperand) -> &arch::arm64::Arm64Operand {
    if let arch::ArchOperand::Arm64Operand(arm_operand) = operand {
        arm_operand
    } else {
        unreachable!()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = Module::new();

    let path = std::path::PathBuf::from("../example/target/debug/example");
    let file_data = std::fs::read(path).expect("Could not read file.");

    let instruction_bytes = extract_instructions(file_data.as_slice())?;

    println!("Extracted {} instruction bytes", instruction_bytes.len());

    // Setup registers

    fn get_reg_local_index(reg_index: u16) -> u32 {
        (reg_index as u32) + 1
    }

    unsafe fn access_reg32(
        module: *mut by::BinaryenModule,
        reg_index: u32,
        use_zero_reg: bool,
    ) -> by::BinaryenExpressionRef {
        if use_zero_reg && reg_index == 31 {
            return unsafe { by::BinaryenConst(module, by::BinaryenLiteralInt32(0)) };
        }

        unsafe {
            by::BinaryenUnary(
                module,
                by::BinaryenWrapInt64(),
                by::BinaryenLocalGet(module, get_reg_local_index(reg_index), by::BinaryenInt64()),
            )
        }
    }

    unsafe fn access_reg64(
        module: *mut by::BinaryenModule,
        reg_index: u32,
        use_zero_reg: bool,
    ) -> by::BinaryenExpressionRef {
        if use_zero_reg && reg_index == 31 {
            return unsafe { by::BinaryenConst(module, by::BinaryenLiteralInt64(0)) };
        }

        unsafe { by::BinaryenLocalGet(module, get_reg_local_index(reg_index), by::BinaryenInt64()) }
    }

    let return_pc_addr = 0;

    // Loop through instructions

    // let mut operations = Vec::new();

    let cs = Capstone::new()
        .arm64()
        .mode(capstone::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .unwrap();

    let instructions = cs.disasm_all(&instruction_bytes, 0x1000).unwrap();

    for instruction in instructions.as_ref() {
        // println!();
        // println!("{}", insn);

        let detail: InsnDetail = cs.insn_detail(&instruction)?;
        let arch_detail: ArchDetail = detail.arch_detail();
        let ops = arch_detail.operands();

        let output: &[(&str, String)] = &[
            ("insn id:", format!("{:?}", instruction.id().0)),
            ("bytes:", format!("{:?}", instruction.bytes())),
            // ("read regs:", cs.reg_name(detail.regs_read())),
            // ("write regs:", reg_names(&cs, detail.regs_write())),
            // ("insn groups:", group_names(&cs, detail.groups())),
        ];

        // for &(ref name, ref message) in output.iter() {
        //     println!("{:4}{:12} {}", "", name, message);
        // }

        // println!("{:4}operands: {}", "", ops.len());

        // detail.regs_write()

        match instruction.mnemonic().unwrap() {
            "sub" => {
                println!("Found a sub instruction!");
                let op0 = get_arm_operand(&ops[0]);
                let op1 = get_arm_operand(&ops[1]);
                let op2 = get_arm_operand(&ops[2]);

                eprintln!("op0: {op0:?}");
                eprintln!("op1: {op1:?}");
                eprintln!("op2: {op2:?}");

                let reg0_id = if let arch::arm64::Arm64OperandType::Reg(reg_id) = op0.op_type {
                    reg_id.0
                } else {
                    unreachable!()
                };

                let reg0 = get_reg_local_index(reg0_id);
            }
            _ => {}
        }

        eprintln!("{:?}", instruction.mnemonic());

        break;
    }

    // Run the relooper

    let relooper = unsafe { by::RelooperCreate(module.by_module) };

    let block = unsafe {
        by::RelooperAddBlock(
            relooper,
            by::BinaryenDrop(
                module.by_module,
                by::BinaryenBinary(
                    module.by_module,
                    by::BinaryenAddInt64(),
                    by::BinaryenConst(module.by_module, by::BinaryenLiteralInt64(3)),
                    by::BinaryenConst(module.by_module, by::BinaryenLiteralInt64(4)),
                ),
            ),
        )
    };

    let expr = unsafe { by::RelooperRenderAndDispose(relooper, block, 0) };

    let main_func_name = CString::new("main").unwrap();

    let main_func = unsafe {
        by::BinaryenAddFunction(
            module.by_module,
            main_func_name.as_ptr(),
            by::BinaryenTypeNone(),
            by::BinaryenTypeNone(),
            [].as_mut_ptr(),
            0,
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
