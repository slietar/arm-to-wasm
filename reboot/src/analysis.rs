use std::collections::{HashMap, HashSet};

use elf::section;

use crate::{
    INSTRUCTION_SIZE,
    instructions::{Address, AddressingMode, Instruction, Register, SizeVariant},
    translator::{
        ExecutableSegment, bit_mask, decode_bl_target, get_arm_operand, get_register_id,
        sign_extend,
    },
};

pub fn analyze(elf_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
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

        let (section_data, _) = elf_file.section_data(&section)?;
        let instructions = Instruction::decode_bytes(section_data);
        let section_start_address = section.sh_addr;

        for (instruction_index, instruction) in instructions.iter().enumerate() {
            if let Instruction::BranchWithLink { target } = instruction {
                let target_address = ((section_start_address as i64)
                    + ((instruction_index as i64) + (*target as i64)) * (INSTRUCTION_SIZE as i64))
                    as u64;

                routines
                    .entry(target_address)
                    .or_insert_with(|| Routine { name: None });
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
        let mut jump_addresses = HashSet::new();

        let mut stack_entry_size = None;

        // Prologue = no branching instruction yet
        let mut is_prologue = true;

        #[derive(Debug)]
        struct StackAccess {
            address: u64,
            offset: i32,
            size: SizeVariant,
            read: bool,
            write: bool,
        }

        let mut stack_accesses = Vec::<StackAccess>::new();

        while !queue.is_empty() {
            let branch_address = queue.pop().unwrap();

            if handled_addresses.contains(&branch_address) {
                continue;
            }

            // eprintln!("Handling address {:#x}", current_address);

            // let instruction_index = (current_address - segment.address) / INSTRUCTION_SIZE;
            let instructions = Instruction::decode_bytes(
                &segment_data[((branch_address - segment.address) as usize)..],
            );

            for (instruction_index, instruction) in instructions.iter().enumerate() {
                let current_address =
                    branch_address + (instruction_index as u64) * INSTRUCTION_SIZE;

                if handled_addresses.contains(&current_address) {
                    break;
                }

                handled_addresses.insert(current_address);

                // eprintln!("Handling address {:#x}", current_address);

                match instruction {
                    Instruction::Branch { target } => {
                        let target_address = ((current_address as i64)
                            + (*target as i64) * (INSTRUCTION_SIZE as i64))
                            as u64;
                        // eprintln!("Branch target address: {:#x}", target_address);

                        queue.push(target_address);
                        jump_addresses.insert(target_address);
                        is_prologue = false;
                        break;
                    }
                    Instruction::BranchConditionally { target, condition } => {
                        let target_address = ((current_address as i64)
                            + (*target as i64) * (INSTRUCTION_SIZE as i64))
                            as u64;
                        // eprintln!("Cond Branch target address: {:#x}", target_address);

                        queue.push(target_address);
                        jump_addresses.insert(target_address);
                        is_prologue = false;
                    }
                    Instruction::TestBitAndBranchIfNonzero {
                        bit,
                        target,
                        value,
                        variant,
                    }
                    | Instruction::TestBitAndBranchIfZero {
                        bit,
                        target,
                        value,
                        variant,
                    } => {
                        let target_address = ((current_address as i64)
                            + (*target as i64) * (INSTRUCTION_SIZE as i64))
                            as u64;

                        queue.push(target_address);
                        jump_addresses.insert(target_address);
                        is_prologue = false;
                    }
                    Instruction::BranchWithLink { target } => {
                        is_prologue = false;
                    }
                    Instruction::SubImmediate {
                        destination: Register::SP,
                        operand,
                        source: Register::SP,
                        variant: SizeVariant::Reg64,
                    } if stack_entry_size.is_none() && is_prologue => {
                        stack_entry_size = Some(*operand);
                    }
                    Instruction::StorePairOfRegisters {
                        address:
                            Address {
                                base: Register::SP,
                                mode:
                                    AddressingMode::PostIndexWithWriteback { offset }
                                    | AddressingMode::PreIndexWithWriteback { offset },
                            },
                        value1,
                        value2,
                        variant,
                    } if stack_entry_size.is_none() && is_prologue => {
                        assert!(*offset <= 0);
                        stack_entry_size = Some(-*offset as u64);

                        stack_accesses.push(StackAccess {
                            address: current_address,
                            offset: 0,
                            size: *variant,
                            read: false,
                            write: true,
                        });

                        stack_accesses.push(StackAccess {
                            address: current_address,
                            offset: if *variant == SizeVariant::Reg64 { 8 } else { 4 },
                            size: *variant,
                            read: false,
                            write: true,
                        });
                    }
                    Instruction::StoreRegisterImmediate {
                        address:
                            Address {
                                base: Register::SP,
                                mode:
                                    AddressingMode::PostIndexWithWriteback { offset }
                                    | AddressingMode::PreIndexWithWriteback { offset },
                            },
                        value,
                        variant,
                    } if stack_entry_size.is_none() && is_prologue => {
                        assert!(*offset <= 0);
                        stack_entry_size = Some(-*offset as u64);

                        stack_accesses.push(StackAccess {
                            address: current_address,
                            offset: 0,
                            size: *variant,
                            read: false,
                            write: true,
                        });
                    }
                    Instruction::Return { target } => {
                        is_prologue = false;
                        break;
                    }

                    Instruction::LoadRegisterImmediate {
                        address:
                            Address {
                                base: Register::SP,
                                mode,
                            },
                        destination,
                        variant,
                    } => {
                        stack_accesses.push(StackAccess {
                            address: current_address,
                            offset: mode.access_offset(),
                            size: *variant,
                            read: true,
                            write: false,
                        });
                    }
                    Instruction::StoreRegisterImmediate {
                        address:
                            Address {
                                base: Register::SP,
                                mode,
                            },
                        value,
                        variant,
                    } => {
                        stack_accesses.push(StackAccess {
                            address: current_address,
                            offset: mode.access_offset(),
                            size: *variant,
                            read: false,
                            write: true,
                        });
                    }
                    Instruction::StorePairOfRegisters {
                        address:
                            Address {
                                base: Register::SP,
                                mode,
                            },
                        value1,
                        value2,
                        variant,
                    } => {
                        stack_accesses.push(StackAccess {
                            address: current_address,
                            offset: mode.access_offset(),
                            size: *variant,
                            read: false,
                            write: true,
                        });

                        stack_accesses.push(StackAccess {
                            address: current_address,
                            offset: mode.access_offset()
                                + (if *variant == SizeVariant::Reg64 { 8 } else { 4 }),
                            size: *variant,
                            read: false,
                            write: true,
                        });
                    }

                    _ => {
                        // eprintln!("Skipping instruction: {} {}", instruction.mnemonic().unwrap(), instruction.op_str().unwrap());
                    }
                }
            }
        }

        eprintln!("Stack entry size: {:?}", stack_entry_size);
        // eprintln!("Jump addresses: {:#x?}", jump_addresses);
        // eprintln!("Stack accesses: {:#?}", stack_accesses);

        // if routine.name.as_deref() == Some("_RNvNtCsbcsGJ9IzBgZ_4core9panicking9panic_fmt") {
        //     eprintln!("Stack accesses: {:#?}", stack_accesses);
        // }

        let mut variables = HashMap::<i32, SizeVariant>::new();

        for stack_access in stack_accesses {
            let offset = stack_access.offset;

            if let Some(existing_size) = variables.get_mut(&offset) {
                if stack_access.size == SizeVariant::Reg64 {
                    *existing_size = SizeVariant::Reg64;
                }
            } else {
                variables.insert(offset, stack_access.size);
            }
        }

        let mut variables = variables.into_iter().collect::<Vec<_>>();
        variables.sort_by_key(|(offset, size)| *offset);

        for (offset, size) in variables {
            eprintln!("Variable at SP{:+#}: {:?}", offset, size);
        }
    }

    // eprintln!("Found {} unique function addresses", routines.len());
    // eprintln!("Function addresses: {:#x?}", routines.iter().filter(|(_, routine)| routine.name.is_none()).map(|(addr, _)| addr).collect::<Vec<_>>());

    Ok(())
}
