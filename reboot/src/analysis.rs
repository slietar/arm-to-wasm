use std::collections::{HashMap, HashSet};

use capstone::prelude::*;
use elf::section;

use crate::translator::{ExecutableSegment, bit_mask, decode_bl_target, get_arm_operand, sign_extend};

const INSTRUCTION_SIZE: u64 = 4;


pub fn decode_target_address(instruction: &capstone::Insn, current_address: u64, length: u32, shift: u32) -> u64 {
    let encoded = u32::from_le_bytes(instruction.bytes().try_into().unwrap());
    let imm = ((encoded >> shift) as u64) & bit_mask(length);
    let imm = u64::cast_signed(sign_extend(imm as u64, length)) << 2;
    ((current_address as i64) + imm) as u64
}

pub fn analyze(elf_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let disassembler = Capstone::new()
        .arm64()
        .mode(capstone::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .unwrap();

    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&elf_bytes)?;
    let (section_headers_opt, section_name_table_opt) = elf_file.section_headers_with_strtab()?;
    let section_headers = section_headers_opt
        .ok_or_else(|| "ELF has no section headers".to_string())?
        .iter()
        .collect::<Vec<_>>();
    let section_name_table =
        section_name_table_opt.ok_or_else(|| "ELF has no section name string table".to_string())?;

    #[derive(Debug)]
    struct Routine {
        name: Option<String>,
    }

    let mut routines = HashMap::<u64, Routine>::new();

    let entry_address = elf_file.ehdr.e_entry;

    if entry_address != 0 {
        routines.insert(entry_address, Routine { name: None });
    }

    if let Some((symbol_table, symbol_string_table)) = elf_file.symbol_table()? {
        for symbol in symbol_table.iter() {
            if symbol.st_symtype() == elf::abi::STT_FUNC {
                // let section = section_headers[symbol.st_shndx as usize];

                // assert!(section.sh_type == elf::abi::SHT_PROGBITS);
                // assert!((section.sh_flags & elf::abi::SHF_EXECINSTR as u64) != 0);

                // let section_address = section.sh_addr;

                routines.insert(
                    // section_address + symbol.st_value,
                    symbol.st_value,
                    Routine {
                        name: Some(
                            symbol_string_table
                                .get(symbol.st_name as usize)?
                                .to_string(),
                        ),
                    },
                );
            }
        }
    }

    for section in section_headers.iter() {
        let is_executable = (section.sh_flags & elf::abi::SHF_EXECINSTR as u64) != 0;
        let section_name = section_name_table.get(section.sh_name as usize);

        // eprintln!("Section: {:?}, Executable: {}, Type: {}", section_name, is_executable, section.sh_type);

        if !is_executable {
            continue;
        }

        let (data, _) = elf_file.section_data(&section)?;

        let instructions = disassembler.disasm_all(data, 0x1000).unwrap();

        // eprintln!("Found {} instructions", instructions.len());

        let section_start_address = section.sh_addr;

        for (instruction_index, instruction) in instructions.as_ref().iter().enumerate() {
            let detail: InsnDetail = disassembler.insn_detail(&instruction).unwrap();
            let arch_detail = detail.arch_detail();
            let mut ops = arch_detail.arm64().unwrap().operands();

            let current_address =
                section_start_address + (instruction_index as u64) * INSTRUCTION_SIZE;

            if instruction.mnemonic().unwrap() == "bl" {
                if let capstone::arch::arm64::Arm64OperandType::Imm(imm) =
                    ops.next().unwrap().op_type
                {
                    let target_address = decode_bl_target(&instruction, current_address);
                    routines
                        .entry(target_address)
                        .or_insert_with(|| Routine { name: None });
                }
            }
        }
    }

    let executable_segments: Vec<_> = elf_file
        .segments()
        .unwrap()
        .iter()
        .filter(|seg| (seg.p_type == elf::abi::PT_LOAD) && ((seg.p_flags & elf::abi::PF_X) != 0))
        .map(|seg| ExecutableSegment {
            address: seg.p_vaddr,
            source_offset: seg.p_offset,
            size: seg.p_filesz,
        })
        .collect();

    for (&routine_address, routine) in &routines {
        let segment = executable_segments
            .iter()
            .find(|segment| {
                routine_address >= segment.address
                    && routine_address < segment.address + segment.size
            })
            .unwrap();

        eprintln!(
            "\nRoutine at {:#x} ({})",
            routine_address,
            routine.name.as_deref().unwrap_or("<unknown>"),
        );

        let segment_data = &elf_bytes
            [(segment.source_offset as usize)..((segment.source_offset + segment.size) as usize)];

        let mut handled_addresses = HashSet::new();
        let mut queue = vec![routine_address];
        let mut jump_addresses = Vec::new();

        while !queue.is_empty() {
            let branch_address = queue.pop().unwrap();

            if handled_addresses.contains(&branch_address) {
                continue;
            }

            // eprintln!("Handling address {:#x}", current_address);

            // let instruction_index = (current_address - segment.address) / INSTRUCTION_SIZE;
            let instructions = disassembler
                .disasm_all(
                    &segment_data[((branch_address - segment.address) as usize)..],
                    0x1000,
                )
                .unwrap();

            for (instruction_index, instruction) in instructions.iter().enumerate() {
                let current_address =
                    branch_address + (instruction_index as u64) * INSTRUCTION_SIZE;

                if handled_addresses.contains(&current_address) {
                    break;
                }

                handled_addresses.insert(current_address);

                eprintln!("Handling address {:#x}", current_address);

                let detail: InsnDetail = disassembler.insn_detail(&instruction).unwrap();
                let arch_detail = detail.arch_detail();
                // let mut ops = arch_detail.arm64().unwrap().operands();

                match instruction.mnemonic().unwrap() {
                    "b" => {
                        let target_address = decode_target_address(&instruction, current_address, 26, 0);
                        // eprintln!("Branch target address: {:#x}", target_address);

                        queue.push(target_address);
                        jump_addresses.push(target_address);
                        break;
                    },
                    "b.lt" => {
                        let target_address = decode_target_address(&instruction, current_address, 19, 5);
                        // eprintln!("Cond Branch target address: {:#x}", target_address);

                        queue.push(target_address);
                        jump_addresses.push(target_address);
                    },
                    "tbnz" | "tbz" => {
                        let target_address = decode_target_address(&instruction, current_address, 14, 5);

                        queue.push(target_address);
                        jump_addresses.push(target_address);
                    }
                    "bl" => {
                        break;
                    },
                    _ => {
                        // eprintln!("Skipping instruction: {} {}", instruction.mnemonic().unwrap(), instruction.op_str().unwrap());
                    },
                }
            }

            // let instructions = instructions.iter().collect::<Vec<_>>();
            // let instruction = instructions[instruction_index as usize];

            // if instruction.mnemonic().unwrap() == "bl" {
            //     if let capstone::arch::arm64::Arm64OperandType::Imm(imm) =
            //         ops.next().unwrap().op_type
            //     {
            //         let target_address = decode_bl_target(&instruction, current_address);
            //         queue.push(target_address);
            //     }
            // }

            // handled_addresses.insert(current_address);
        }

        eprintln!("Jump addresses: {:#x?}", jump_addresses);
    }

    // eprintln!("Found {} unique function addresses", routines.len());
    // eprintln!("Function addresses: {:#x?}", routines.iter().filter(|(_, routine)| routine.name.is_none()).map(|(addr, _)| addr).collect::<Vec<_>>());

    Ok(())
}
