mod module;

use std::fs::File;

use capstone::prelude::*;
use elf::{ElfBytes, abi::SHF_EXECINSTR, endian::AnyEndian};

use crate::module::Module;

fn extract_instructions(file_data: &[u8]) -> Result<Vec<u8>, String> {
    let file = ElfBytes::<AnyEndian>::minimal_parse(file_data)
        .map_err(|err| format!("Failed to parse ELF file: {err}"))?;

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

const INSTRUCTION_SIZE: usize = 4;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module = Module::new();

    let mut output_file = File::create("output.wasm")?;

    module.print();
    module.save(&mut output_file)?;

    return Ok(());


    let path = std::path::PathBuf::from("../example/target/debug/example");
    let file_data = std::fs::read(path).expect("Could not read file.");

    let instructions = extract_instructions(file_data.as_slice())
        .expect("Failed to extract instructions from ELF");

    println!("Extracted {} instruction bytes", instructions.len());

    let cs = Capstone::new()
        .arm64()
        .mode(capstone::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .unwrap();

    let insns = cs.disasm_all(&instructions, 0x1000).unwrap();

    for insn in insns.as_ref() {
        // println!();
        // println!("{}", insn);

        let detail: InsnDetail = cs.insn_detail(&insn).expect("Failed to get insn detail");
        let arch_detail: ArchDetail = detail.arch_detail();
        let ops = arch_detail.operands();

        let output: &[(&str, String)] = &[
            ("insn id:", format!("{:?}", insn.id().0)),
            ("bytes:", format!("{:?}", insn.bytes())),
            // ("read regs:", cs.reg_name(detail.regs_read())),
            // ("write regs:", reg_names(&cs, detail.regs_write())),
            // ("insn groups:", group_names(&cs, detail.groups())),
        ];

        // for &(ref name, ref message) in output.iter() {
        //     println!("{:4}{:12} {}", "", name, message);
        // }

        // println!("{:4}operands: {}", "", ops.len());

        // detail.regs_write()

        match insn.mnemonic().unwrap() {
            "sub" => {
                println!("Found a sub instruction!");
                let op0 = &ops[0];
                let op1 = &ops[1];
                let op2 = &ops[2];

                eprintln!("op0: {op0:?}");
                eprintln!("op1: {op1:?}");
                eprintln!("op2: {op2:?}");
            }
            _ => {}
        }

        eprintln!("{:?}", insn.mnemonic());

        break;
    }

    // let instruction_count = instructions.len() / INSTRUCTION_SIZE;

    // for instr_index in 0..instruction_count {
    //     let instruction_addr = (instr_index * INSTRUCTION_SIZE) as u64;

    //     let offset = (instr_index * INSTRUCTION_SIZE) as usize;
    //     let instruction_encoded = u32::from_le_bytes(instructions[offset..(offset + INSTRUCTION_SIZE)].try_into().unwrap());
    //     let instruction = match disarm64::decoder::decode(instruction_encoded) {
    //         Some(instruction) => instruction,
    //         None => {
    //             break;
    //         },
    //     };

    //     // match instruction.operation {
    //     //     Instruction => {},
    //     // }

    //     eprintln!("0x{instruction_addr:08x}: {instruction}");
    //     eprintln!("{:<?}", instruction.operation);
    // }

    Ok(())
}
