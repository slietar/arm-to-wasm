use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use elf::ElfBytes;

use crate::architecture::{Architecture, BranchKind, Instr};

pub type ElfFile<'a> = ElfBytes<'a, elf::endian::AnyEndian>;

#[derive(Debug)]
pub struct Routine<A: Architecture> {
    pub address: u64,
    pub blocks: Vec<Block<A>>,
    pub name: Option<String>,
    pub stack_size: Option<u64>,
    pub variables: HashMap<i32, u32>,
}

#[derive(Debug)]
pub struct Analysis<A: Architecture> {
    pub entry_routine_index: Option<usize>,
    pub routines: Vec<Routine<A>>,
}

#[derive(Debug)]
struct ExecutableSegment<A: Architecture> {
    pub address: u64,
    pub instructions: Vec<A::InstrType>,
    pub source_offset: u64,
    pub size: u64,
}

#[derive(Debug)]
pub struct Block<A: Architecture> {
    pub fallthrough_block_index: Option<usize>,
    pub instructions: Vec<A::InstrType>,
    pub jump_block_index: Option<usize>,
    pub start_address: u64,

    // Implies fallthrough_block_index and jump_block_index are None
    pub tail_call_routine_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct Jump {
    conditional: bool,
    next_address: u64,
    source_address: u64,
    tail_call_routine_index: Option<usize>,
    target_address: u64,
}

#[derive(Debug)]
struct StackAccess {
    address: u64,
    offset: i32,
    size_bytes: u32,
    read: bool,
    write: bool,
}

pub fn analyze<A: Architecture>(
    elf_bytes: &[u8],
    elf_file: &ElfFile<'_>,
    architecture: &A,
) -> Result<Analysis<A>, Box<dyn std::error::Error>> {
    let (section_headers_opt, section_name_table_opt) = elf_file.section_headers_with_strtab()?;
    let section_headers = section_headers_opt
        .ok_or_else(|| "ELF has no section headers".to_string())?
        .iter()
        .collect::<Vec<_>>();
    let section_name_table =
        section_name_table_opt.ok_or_else(|| "ELF has no section name string table".to_string())?;

    let mut routines_names_by_address = HashMap::<u64, Option<String>>::new();

    let entry_address = elf_file.ehdr.e_entry;

    if entry_address != 0 {
        routines_names_by_address.insert(entry_address, None);
    }

    if let Some((symbol_table, symbol_string_table)) = elf_file.symbol_table()? {
        for symbol in symbol_table.iter() {
            if symbol.st_symtype() == elf::abi::STT_FUNC {
                routines_names_by_address.insert(
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
        let _section_name = section_name_table.get(section.sh_name as usize);

        if !is_executable {
            continue;
        }

        let (section_data, _) = elf_file.section_data(&section)?;
        let _instructions = architecture.decode_instructions(section_data);
    }

    let executable_segments: Vec<ExecutableSegment<A>> = elf_file
        .segments()
        .unwrap()
        .iter()
        .filter(|seg| (seg.p_type == elf::abi::PT_LOAD) && ((seg.p_flags & elf::abi::PF_X) != 0))
        .map(|seg| ExecutableSegment {
            address: seg.p_vaddr,
            // TODO: Move to section
            instructions: architecture.decode_instructions(
                &elf_bytes[(seg.p_offset as usize)..(seg.p_offset + seg.p_filesz) as usize],
            ),
            source_offset: seg.p_offset,
            size: seg.p_filesz,
        })
        .collect();

    let mut routines = Vec::new();

    let mut routine_addresses_and_names_sorted = routines_names_by_address
        .iter()
        .map(|(&addr, name)| (addr, (*name).as_deref()))
        .collect::<Vec<_>>();

    routine_addresses_and_names_sorted.sort_by_key(|(addr, _)| *addr);

    for (routine_address, routine_name) in routine_addresses_and_names_sorted.iter().copied() {
        let segment = executable_segments
            .iter()
            .find(|segment| {
                routine_address >= segment.address
                    && routine_address < segment.address + segment.size
            })
            .unwrap();

        let segment_data = &elf_bytes
            [(segment.source_offset as usize)..((segment.source_offset + segment.size) as usize)];

        let routine_data_offset = segment.source_offset + (routine_address - segment.address);

        let next_routine_address = routines_names_by_address
            .keys()
            .filter(|&&addr| addr > routine_address)
            .min()
            .copied();

        let routine_end_address = next_routine_address
            .unwrap_or(u64::MAX)
            .min(segment.address + segment.size);

        let mut jumps = Vec::new();
        let mut stack_entry_size = None;

        // Prologue = no branching instruction yet
        let mut is_prologue = true;

        let mut stack_accesses = Vec::<StackAccess>::new();
        let mut explicit_returned_addresses = HashSet::<u64>::new();

        let mut current_address = routine_address;

        // TODO: Improve instruction index lookup
        let routine_first_instruction_index = {
            let mut p = segment.address;
            segment.instructions
                .iter()
                .position(|instruction| {
                    let instruction_size = instruction.size();
                    let instruction_end = p + instruction_size;

                    let is_first_instruction = current_address >= p && current_address < instruction_end;

                    p = instruction_end;

                    is_first_instruction
                })
                .unwrap()
        };

        for instruction in &segment.instructions[routine_first_instruction_index..] {
            // Catches branches [to a function that makes no calls or] to a function that never returns
            if (current_address < routine_address) || (current_address >= routine_end_address) {
                eprintln!("Unexpected stopping at address {:#x}", current_address,);
                break;
            }

            let next_address = current_address + instruction.size();

            let branch_kind = instruction.branch_kind(current_address);
            let (allocate, accesses) = instruction.stack_frame_effect();

            // eprintln!("{:#x}: {:?}", current_address, instruction);
            // eprintln!("  Branch kind: {:x?}", branch_kind);

            if let Some(size) = allocate
                && stack_entry_size.is_none()
                && is_prologue
            {
                stack_entry_size = Some(size);
            }

            if !accesses.is_empty() {
                for access in accesses {
                    stack_accesses.push(StackAccess {
                        address: current_address,
                        offset: access.offset,
                        size_bytes: access.size_bytes,
                        read: access.read,
                        write: access.write,
                    });
                }

                is_prologue = false;
            }

            match branch_kind {
                BranchKind::Jump {
                    conditional,
                    target_address
                } => {
                    let external_jump = target_address < routine_address
                        || target_address >= routine_end_address;
                    let external_called_routine_index = external_jump
                        .then(|| {
                            routine_addresses_and_names_sorted
                                .iter()
                                .position(|(addr, _)| *addr == target_address)
                        })
                        .flatten();

                    if external_jump && external_called_routine_index.is_none() {
                        eprintln!("Current address: {:#x}", current_address);
                        eprintln!("Target address: {:#x}", target_address);
                        eprintln!("Routine address: {:#x}", routine_address);
                        eprintln!("Routine end address: {:#x}", routine_end_address);
                        panic!();
                    }

                    jumps.push(Jump {
                        conditional,
                        next_address,
                        tail_call_routine_index: external_called_routine_index,
                        target_address,
                        source_address: current_address,
                    });

                    is_prologue = false;

                    if !conditional {
                        break;
                    }
                }
                BranchKind::Call => {
                    is_prologue = false;
                }
                BranchKind::Return => {
                    is_prologue = false;
                    explicit_returned_addresses.insert(next_address);
                    break;
                }
                BranchKind::None => {}
            }

            current_address = next_address;
        }

        let _ = is_prologue;

        // eprintln!("Jumps: {:#x?}", jumps);

        // Block analysis

        let mut block_start_addresses = jumps
            .iter()
            .filter(|jump| jump.tail_call_routine_index.is_none())
            .map(|jump| jump.target_address)
            .chain(std::iter::once(routine_address))
            .chain(
                jumps
                    .iter()
                    .filter(|jump| jump.conditional)
                    .map(|jump| jump.next_address),
            )
            .collect::<Vec<_>>();

        block_start_addresses.sort();
        block_start_addresses.dedup();

        #[derive(Debug, Clone)]
        enum BlockEndKind {
            ExplicitReturn,
            JumpTarget,
            JumpSource(Jump),
            RoutineEnd,
            TailCall { routine_index: usize },
        }

        let mut block_ends = std::iter::empty()
            .chain(
                jumps
                    .iter()
                    .filter(|jump| jump.tail_call_routine_index.is_none())
                    .flat_map(|jump| {
                        [
                            // The block ends after a branch instruction
                            (
                                jump.next_address,
                                BlockEndKind::JumpSource(jump.clone()),
                            ),
                            // The block ends just before a branch instruction
                            (jump.target_address, BlockEndKind::JumpTarget),
                        ]
                    }),
            )
            .chain(
                // The block ends after an explicit return instruction
                explicit_returned_addresses
                    .iter()
                    .map(|&address| (address, BlockEndKind::ExplicitReturn)),
            )
            .chain(
                // The block ends after a tail call
                jumps.iter().filter_map(|jump| {
                    jump.tail_call_routine_index.map(|routine_index| {
                        (
                            jump.source_address,
                            BlockEndKind::TailCall { routine_index },
                        )
                    })
                }),
            )
            .chain(
                // The block ends after reaching the end of the routine, which
                // is not a problem because there likely was an earlier call
                // that trapped
                std::iter::once((routine_end_address, BlockEndKind::RoutineEnd)),
            )
            .collect::<Vec<_>>();

        block_ends.sort_by_key(|(address, jump)| {
            (
                *address,
                match jump {
                    // The lowest value is kept
                    BlockEndKind::ExplicitReturn => 1,
                    BlockEndKind::JumpTarget => 2,
                    BlockEndKind::JumpSource(_) => 0,
                    BlockEndKind::RoutineEnd => 3,
                    BlockEndKind::TailCall { .. } => 4,
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
                    BlockEndKind::JumpTarget
                        | BlockEndKind::JumpSource(Jump {
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
                    BlockEndKind::JumpSource(jump) => Some(
                        block_start_addresses
                            .iter()
                            .position(|&start_addr| start_addr == jump.target_address)
                            .unwrap(),
                    ),
                    _ => None,
                };

                // If a conditional jump targets the next instruction, ignore
                // the jump and treat it as a fallthrough
                if fallthrough_block_index == jump_block_index {
                    jump_block_index = None;
                }

                let tail_call_routine_index =
                    if let BlockEndKind::TailCall { routine_index } = end_kind {
                        Some(*routine_index)
                    } else {
                        None
                    };

                Block {
                    start_address: addr,
                    instructions: architecture.decode_instructions(
                        &segment_data[((addr - segment.address) as usize)
                            ..((end_addr - segment.address) as usize)],
                    ),
                    fallthrough_block_index,
                    jump_block_index,
                    tail_call_routine_index,
                }
            })
            .collect::<Vec<_>>();

        // Block entrance check

        let mut blocks_entered = vec![false; blocks.len()];
        blocks_entered[0] = true;

        for block in blocks.iter() {
            if let Some(fallthrough_block_index) = block.fallthrough_block_index {
                blocks_entered[fallthrough_block_index] = true;
            }

            if let Some(jump_block_index) = block.jump_block_index {
                blocks_entered[jump_block_index] = true;
            }
        }

        if !blocks_entered.iter().all(|&entered| entered) {
            panic!("Not all blocks are entered: {:#?}", blocks_entered);
        }

        // Register read analysis

        let segment_instructions = architecture.decode_instructions(segment_data);

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

            for instruction in &block.instructions {
                walker.process::<A>(instruction);
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

        let _ = Walker::merge_all(&exit_walkers);

        // Variable analysis

        let mut variables = HashMap::<i32, u32>::new();

        for stack_access in stack_accesses {
            let offset = stack_access.offset;

            if let Some(existing_size) = variables.get_mut(&offset) {
                *existing_size = (*existing_size).max(stack_access.size_bytes);
            } else {
                variables.insert(offset, stack_access.size_bytes);
            }
        }

        routines.push(Routine {
            address: routine_address,
            name: routine_name.map(|s| s.to_string()),
            stack_size: stack_entry_size,
            variables: variables.clone(),
            blocks,
        });
    }

    routines.sort_by_key(|routine| -(routine.address as i64));

    Ok(Analysis {
        entry_routine_index: routines
            .iter()
            .position(|routine| routine.address == entry_address),
        routines,
    })
}

pub fn main_analyze<A: Architecture>(
    elf_bytes: &[u8],
    architecture: &A,
) -> Result<(), Box<dyn std::error::Error>> {
    let elf_file = ElfFile::minimal_parse(elf_bytes)?;
    let analysis = analyze(elf_bytes, &elf_file, architecture)?;

    eprintln!("Analysis: {:#?}", analysis);

    Ok(())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RegisterReadWalker {
    registers_read: HashSet<u32>,
    registers_written: HashSet<u32>,
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

    fn process<A: Architecture>(&mut self, instruction: &A::InstrType) {
        self.registers_read.extend(
            instruction
                .registers_read()
                .into_iter()
                .filter(|reg| !self.registers_written.contains(reg)),
        );

        self.registers_written
            .extend(instruction.registers_written());
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
