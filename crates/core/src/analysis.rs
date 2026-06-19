use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use elf::section;

use crate::{
    constants::INSTRUCTION_SIZE,
    instruction_helper::InstructionInfo as _,
    instructions::{Address, AddressingMode, Instruction, Register, SizeVariant, SizedRegister},
};

#[derive(Debug, Clone)]
pub struct Routine {
    pub address: u64,
    pub blocks: Vec<Block>,
    pub name: Option<String>,
    pub stack_size: Option<u64>,
    pub variables: HashMap<i32, SizeVariant>,
}

#[derive(Debug, Clone)]
pub struct Analysis {
    pub entry_routine_index: Option<usize>,
    pub routines: Vec<Routine>,
}

#[derive(Debug)]
struct ExecutableSegment {
    pub address: u64,
    pub instructions: Vec<Instruction>,
    pub source_offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub fallthrough_block_index: Option<usize>,
    pub instruction_count: u64,
    pub instructions: Vec<Instruction>,
    pub jump_block_index: Option<usize>,
    pub start_address: u64,
}

#[derive(Debug, Clone)]
struct Jump {
    conditional: bool,
    source_address: u64,
    target_address: u64,
}

#[derive(Debug)]
struct StackAccess {
    address: u64,
    offset: i32,
    size: SizeVariant,
    read: bool,
    write: bool,
}

pub fn analyze(elf_bytes: &[u8]) -> Result<Analysis, Box<dyn std::error::Error>> {
    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&elf_bytes)?;
    let (section_headers_opt, section_name_table_opt) = elf_file.section_headers_with_strtab()?;
    let section_headers = section_headers_opt
        .ok_or_else(|| "ELF has no section headers".to_string())?
        .iter()
        .collect::<Vec<_>>();
    let section_name_table =
        section_name_table_opt.ok_or_else(|| "ELF has no section name string table".to_string())?;

    let mut routines_names = HashMap::<u64, Option<String>>::new();

    let entry_address = elf_file.ehdr.e_entry;

    if entry_address != 0 {
        routines_names.insert(entry_address, None);
    }

    if let Some((symbol_table, symbol_string_table)) = elf_file.symbol_table()? {
        for symbol in symbol_table.iter() {
            if symbol.st_symtype() == elf::abi::STT_FUNC {
                // let section = section_headers[symbol.st_shndx as usize];

                // assert!(section.sh_type == elf::abi::SHT_PROGBITS);
                // assert!((section.sh_flags & elf::abi::SHF_EXECINSTR as u64) != 0);

                // let section_address = section.sh_addr;

                routines_names.insert(
                    // section_address + symbol.st_value,
                    symbol.st_value,
                    Some(
                        symbol_string_table
                            .get(symbol.st_name as usize)?
                            .to_string(),
                    ),
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

                routines_names.entry(target_address).or_insert_with(|| None);
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
            // TODO: Move to section
            instructions: Instruction::decode_bytes(
                &elf_bytes[(seg.p_offset as usize)..(seg.p_offset + seg.p_filesz) as usize],
            ),
            source_offset: seg.p_offset,
            size: seg.p_filesz,
        })
        .collect();

    let mut routines = Vec::new();

    for (&routine_address, routine_name) in &routines_names {
        let segment = executable_segments
            .iter()
            .find(|segment| {
                routine_address >= segment.address
                    && routine_address < segment.address + segment.size
            })
            .unwrap();

        // eprintln!(
        //     "\nRoutine at {:#x} ({})",
        //     routine_address,
        //     routine_name.as_deref().unwrap_or("<unknown>"),
        // );

        let segment_data = &elf_bytes
            [(segment.source_offset as usize)..((segment.source_offset + segment.size) as usize)];

        let routine_data_offset = segment.source_offset + (routine_address - segment.address);

        let next_routine_address = routines_names
            .keys()
            .filter(|&&addr| addr > routine_address)
            .min()
            .copied();

        let routine_max_address = next_routine_address
            .unwrap_or(segment.address + segment.size)
            .min(segment.address + segment.size);

        // eprintln!("Routine address: {:#x}", routine_address);
        // eprintln!("Routine max address: {:#x}", routine_max_address);

        let mut handled_addresses = HashSet::new();
        let mut queue = vec![routine_address];
        let mut jumps = Vec::new();

        let mut stack_entry_size = None;

        // Prologue = no branching instruction yet
        let mut is_prologue = true;

        let mut stack_accesses = Vec::<StackAccess>::new();
        let mut block_exit_addresses = HashSet::<u64>::new();

        while !queue.is_empty() {
            let branch_address = queue.pop().unwrap();

            if handled_addresses.contains(&branch_address) {
                continue;
            }

            // eprintln!("Handling address {:#x}", current_address);

            // let instruction_index = (current_address - segment.address) / INSTRUCTION_SIZE;
            let branch_instructions = &segment.instructions
                [(((branch_address - segment.address) / INSTRUCTION_SIZE) as usize)..];

            for (instruction_index, instruction) in branch_instructions.iter().enumerate() {
                let current_address =
                    branch_address + (instruction_index as u64) * INSTRUCTION_SIZE;

                if handled_addresses.contains(&current_address) {
                    break;
                }

                // Catches branches [to a function that makes no calls or] to a function that never returns
                if (current_address < routine_address) || (current_address >= routine_max_address) {
                    eprintln!("Stopping at address {:#x}", current_address,);
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
                        jumps.push(Jump {
                            conditional: false,
                            target_address,
                            source_address: current_address,
                        });
                        is_prologue = false;
                        break;
                    }
                    Instruction::BranchConditionally { target, .. }
                    | Instruction::CompareAndBranchOnNonzero { target, .. }
                    | Instruction::CompareAndBranchOnZero { target, .. }
                    | Instruction::TestBitAndBranchIfNonzero { target, .. }
                    | Instruction::TestBitAndBranchIfZero { target, .. } => {
                        let target_address = ((current_address as i64)
                            + (*target as i64) * (INSTRUCTION_SIZE as i64))
                            as u64;
                        // eprintln!("Cond Branch target address: {:#x}", target_address);

                        queue.push(target_address);
                        jumps.push(Jump {
                            conditional: true,
                            source_address: current_address,
                            target_address,
                        });
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
                        block_exit_addresses.insert(current_address);
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

        // Block analysis

        // eprintln!("Block start addresses: {:#x?}", jump_source_addresses);
        // eprintln!("Jump destination addresses: {:#x?}", block_start_addresses);

        // let last_address = *handled_addresses.iter().max().unwrap();

        // eprintln!("Jumps: {:#x?}", jumps);

        let mut block_start_addresses = jumps
            .iter()
            .map(|jump| jump.target_address)
            .chain(std::iter::once(routine_address))
            .chain(
                jumps
                    .iter()
                    .filter(|jump| jump.conditional)
                    .map(|jump| jump.source_address + INSTRUCTION_SIZE),
            )
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        block_start_addresses.sort();

        // Problem: RET is not detected

        #[derive(Debug, Clone)]
        enum BlockEndKind {
            Exit,
            Fallthrough,
            Jump(Jump),
        }

        let mut block_ends = jumps
            .iter()
            .flat_map(|jump| {
                [
                    (
                        jump.source_address + INSTRUCTION_SIZE,
                        BlockEndKind::Jump(jump.clone()),
                    ),
                    (jump.target_address, BlockEndKind::Fallthrough),
                ]
            })
            .chain(
                block_exit_addresses
                    .iter()
                    .map(|&address| (address + INSTRUCTION_SIZE, BlockEndKind::Exit)),
            )
            .collect::<Vec<_>>();

        block_ends.push((routine_max_address, BlockEndKind::Exit));

        block_ends.sort_by_key(|(address, jump)| {
            (
                *address,
                match jump {
                    BlockEndKind::Exit => 1,
                    BlockEndKind::Fallthrough => 2,
                    BlockEndKind::Jump(_) => 0,
                },
            )
        });
        block_ends.dedup_by_key(|(address, _)| *address);

        let blocks = block_start_addresses
            .iter()
            .map(|&addr| {
                let (end_addr, end_kind) = block_ends
                    .iter()
                    .find(|(end_addr, _)| *end_addr > addr)
                    .unwrap();

                let fallthrough_block_index = matches!(
                    end_kind,
                    BlockEndKind::Fallthrough
                        | BlockEndKind::Jump(Jump {
                            conditional: true,
                            ..
                        })
                )
                .then(|| {
                    block_start_addresses
                        .iter()
                        .position(|&start_addr| start_addr == *end_addr)
                        .unwrap()
                });

                let mut jump_block_index = match end_kind {
                    BlockEndKind::Jump(jump) => Some(
                        block_start_addresses
                            .iter()
                            .position(|&start_addr| start_addr == jump.target_address)
                            .unwrap(),
                    ),
                    _ => None,
                };

                if fallthrough_block_index == jump_block_index {
                    jump_block_index = None;
                }

                Block {
                    start_address: addr,
                    instruction_count: ((end_addr - addr) / INSTRUCTION_SIZE),
                    instructions: segment.instructions[(((addr - segment.address)
                        / INSTRUCTION_SIZE)
                        as usize)
                        ..(((end_addr - segment.address) / INSTRUCTION_SIZE) as usize)]
                        .to_vec(),
                    fallthrough_block_index,
                    jump_block_index,
                }
            })
            .collect::<Vec<_>>();

        // eprintln!("Block start addresses: {:#x?}", block_start_addresses);
        // eprintln!("Block ends: {:#x?}", block_ends);
        // eprintln!("Blocks: {:#x?}", blocks);

        // eprintln!("Stack entry size: {:?}", stack_entry_size);

        // eprintln!("Handled addresses: {:#x?}", handled_addresses);
        // eprintln!("Jump addresses: {:#x?}", jump_addresses);
        // eprintln!("Stack accesses: {:#?}", stack_accesses);

        // if routine.name.as_deref() == Some("_RNvNtCsbcsGJ9IzBgZ_4core9panicking9panic_fmt") {
        //     eprintln!("Stack accesses: {:#?}", stack_accesses);
        // }

        // Register read analysis

        let segment_instructions = Instruction::decode_bytes(&segment_data);

        type Walker = RegisterReadWalker;
        let walker = Walker::default();

        let mut queue = vec![(0, walker)];
        let mut exit_walkers = Vec::new();
        let mut cache = HashSet::new();

        while let Some(key) = queue.pop() {
            if cache.contains(&key) {
                exit_walkers.push(key.1);
                continue;
            }

            cache.insert(key.clone());

            let (block_index, mut walker) = key;
            let block = &blocks[block_index];

            let block_instructions =
                &segment_instructions[(((block.start_address - segment.address) / INSTRUCTION_SIZE)
                    as usize)
                    ..(((block.start_address - segment.address) / INSTRUCTION_SIZE
                        + block.instruction_count) as usize)];

            for instruction in block_instructions {
                walker.process(instruction);
            }

            let mut inserted = false;

            if let Some(fallthrough_block_index) = block.fallthrough_block_index {
                queue.push((fallthrough_block_index, walker.clone()));
                inserted = true;
            }

            if let Some(jump_block_index) = block.jump_block_index {
                queue.push((jump_block_index, walker.clone()));
                inserted = true;
            }

            if !inserted {
                exit_walkers.push(walker);
            }
        }

        // eprintln!("Exit walker: {:#?}", Walker::merge_all(&exit_walkers));

        // Variable analysis

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

        routines.push(Routine {
            address: routine_address,
            name: routine_name.clone(),
            stack_size: stack_entry_size,
            variables: variables.clone(),
            blocks: blocks.clone(),
        });

        let mut variables = variables.into_iter().collect::<Vec<_>>();
        variables.sort_by_key(|(offset, size)| *offset);

        for (offset, size) in variables {
            // eprintln!("Variable at SP{:+#}: {:?}", offset, size);
        }
    }

    // eprintln!("Found {} unique function addresses", routines.len());
    // eprintln!("Function addresses: {:#x?}", routines.iter().filter(|(_, routine)| routine.name.is_none()).map(|(addr, _)| addr).collect::<Vec<_>>());

    routines.sort_by_key(|routine| -(routine.address as i64));

    Ok(Analysis {
        entry_routine_index: routines
            .iter()
            .position(|routine| routine.address == entry_address),
        routines,
    })
}

pub fn main_analyze(elf_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = analyze(elf_bytes)?;

    eprintln!("Analysis result: {:#?}", analysis);

    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RegisterReadWalker {
    registers_read: HashSet<SizedRegister>,
    registers_written: HashSet<Register>,
}

impl RegisterReadWalker {
    fn merge(&mut self, other: &RegisterReadWalker) -> RegisterReadWalker {
        RegisterReadWalker {
            registers_read: self
                .registers_read
                .union(&other.registers_read)
                .copied()
                .collect(),
            registers_written: self
                .registers_written
                .union(&other.registers_written)
                .copied()
                .collect(),
        }
    }

    fn merge_all(walkers: &[RegisterReadWalker]) -> RegisterReadWalker {
        walkers
            .iter()
            .fold(RegisterReadWalker::default(), |mut acc, walker| {
                acc.merge(walker)
            })
    }

    fn process(&mut self, instruction: &Instruction) {
        self.registers_read.extend(
            instruction
                .registers_read()
                .iter()
                .filter(|&&reg| !self.registers_written.contains(&reg.register)),
        );

        self.registers_written.extend(
            instruction
                .registers_written()
                .iter()
                .map(|reg| reg.register),
        );
    }
}

impl Hash for RegisterReadWalker {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        hash_set(state, &self.registers_read);
        hash_set(state, &self.registers_written);
    }
}

fn hash_set<H: std::hash::Hasher, T: Hash + Eq>(state: &mut H, set: &HashSet<T>) {
    let hash = set
        .iter()
        .map(|t| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            t.hash(&mut hasher);
            std::hash::Hasher::finish(&hasher)
        })
        .fold(0, u64::wrapping_add);

    state.write_usize(set.len());
    state.write_u64(hash);
}
