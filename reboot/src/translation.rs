use std::{ffi::CString, fs::File};

use binaryen::ffi as by;
use elf::ElfBytes;

use crate::{
    constants::PAGE_SIZE,
    instructions::{AddressingMode, Condition, Instruction, Register, SizeVariant},
    module::{BinaryOp, Expression, Module, StoreVariant, UnaryOp},
};

pub const GENERAL_PURPOSE_REGISTER_COUNT: u32 = 31;
pub const PARAMETER_REGISTER_COUNT: u32 = 8;

pub const SP_LOCAL_INDEX: u32 = 0;
pub const CARRY_FLAG_LOCAL_INDEX: u32 = 1;
pub const NEGATIVE_FLAG_LOCAL_INDEX: u32 = 2;
pub const ZERO_FLAG_LOCAL_INDEX: u32 = 3;
pub const OVERFLOW_FLAG_LOCAL_INDEX: u32 = 4;

pub const FIRST_GP_REGISTER_LOCAL_INDEX: u32 = 5;

fn get_reg_local_index(register: Register) -> u32 {
    use Register::*;

    match register {
        SP => SP_LOCAL_INDEX,

        X0 => FIRST_GP_REGISTER_LOCAL_INDEX + 0,
        X1 => FIRST_GP_REGISTER_LOCAL_INDEX + 1,
        X2 => FIRST_GP_REGISTER_LOCAL_INDEX + 2,
        X3 => FIRST_GP_REGISTER_LOCAL_INDEX + 3,
        X4 => FIRST_GP_REGISTER_LOCAL_INDEX + 4,
        X5 => FIRST_GP_REGISTER_LOCAL_INDEX + 5,
        X6 => FIRST_GP_REGISTER_LOCAL_INDEX + 6,
        X7 => FIRST_GP_REGISTER_LOCAL_INDEX + 7,
        X8 => FIRST_GP_REGISTER_LOCAL_INDEX + 8,
        X9 => FIRST_GP_REGISTER_LOCAL_INDEX + 9,
        X10 => FIRST_GP_REGISTER_LOCAL_INDEX + 10,
        X11 => FIRST_GP_REGISTER_LOCAL_INDEX + 11,
        X12 => FIRST_GP_REGISTER_LOCAL_INDEX + 12,
        X13 => FIRST_GP_REGISTER_LOCAL_INDEX + 13,
        X14 => FIRST_GP_REGISTER_LOCAL_INDEX + 14,
        X15 => FIRST_GP_REGISTER_LOCAL_INDEX + 15,
        X16 => FIRST_GP_REGISTER_LOCAL_INDEX + 16,
        X17 => FIRST_GP_REGISTER_LOCAL_INDEX + 17,
        X18 => FIRST_GP_REGISTER_LOCAL_INDEX + 18,
        X19 => FIRST_GP_REGISTER_LOCAL_INDEX + 19,
        X20 => FIRST_GP_REGISTER_LOCAL_INDEX + 20,
        X21 => FIRST_GP_REGISTER_LOCAL_INDEX + 21,
        X22 => FIRST_GP_REGISTER_LOCAL_INDEX + 22,
        X23 => FIRST_GP_REGISTER_LOCAL_INDEX + 23,
        X24 => FIRST_GP_REGISTER_LOCAL_INDEX + 24,
        X25 => FIRST_GP_REGISTER_LOCAL_INDEX + 25,
        X26 => FIRST_GP_REGISTER_LOCAL_INDEX + 26,
        X27 => FIRST_GP_REGISTER_LOCAL_INDEX + 27,
        X28 => FIRST_GP_REGISTER_LOCAL_INDEX + 28,
        X29 => FIRST_GP_REGISTER_LOCAL_INDEX + 29,
        X30 => FIRST_GP_REGISTER_LOCAL_INDEX + 30,

        XZR => unreachable!(),
    }
}

fn get_reg_expr(
    module: &Module,
    register: Register,
    variant: SizeVariant,
    param_count: u32,
) -> Expression {
    match register {
        Register::XZR => match variant {
            SizeVariant::Reg32 => module.const_(0i32),
            SizeVariant::Reg64 => module.const_(0i64),
        },
        _ => {
            let local_index = get_reg_local_index(register);
            let expr = module.local_get(param_count + local_index, module.i64());

            match variant {
                SizeVariant::Reg32 => module.unary(
                    module.binary(
                        expr,
                        module.const_(0x00_00_00_00_ff_ff_ff_ffu64),
                        BinaryOp::And,
                    ),
                    UnaryOp::WrapInt64,
                ),
                SizeVariant::Reg64 => expr,
            }
        }
    }
}

pub fn translate(elf_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = crate::analysis::analyze(elf_bytes)?;

    // eprintln!("Analysis result: {:#?}", analysis);
    let mut module = Module::new();

    let routine_names = analysis
        .routines
        .iter()
        .enumerate()
        .map(|(routine_index, routine)| {
            format!(
                "routine_{}_{}",
                routine_index,
                routine.name.as_deref().unwrap_or("none")
            )
        })
        .collect::<Vec<_>>();

    let local_types = {
        let mut local_types = Vec::new();

        // SP
        local_types.push(module.i64());

        // Flags
        local_types.push(module.i32());
        local_types.push(module.i32());
        local_types.push(module.i32());
        local_types.push(module.i32());

        // GP registers
        local_types.extend((0..GENERAL_PURPOSE_REGISTER_COUNT).map(|_| module.i64()));

        local_types
    };
    let elf_file = ElfBytes::<elf::endian::AnyEndian>::minimal_parse(elf_bytes)?;
    let memory_info = set_up_memory("memory", elf_bytes, elf_file, &module);

    let param_registers = [
        Register::X0,
        Register::X1,
        Register::X2,
        Register::X3,
        Register::X4,
        Register::X5,
        Register::X6,
        Register::X7,
        Register::SP,
    ];

    let param_count = param_registers.len() as u32;

    let param_types = param_registers
        .iter()
        .map(|_| module.i64())
        .collect::<Vec<_>>();

    let return_type = module.tuple(&param_types);

    for (routine_index, routine) in analysis.routines.iter().enumerate() {
        let routine_name = &routine_names[routine_index];

        if routine.name.as_deref().unwrap() != "_start" {
            continue;
        }

        let mut relooper = module.relooper();

        let mut routine_exprs = Vec::new();

        for (param_index, param_register) in param_registers.iter().enumerate() {
            routine_exprs.push(module.local_set(
                param_count + get_reg_local_index(*param_register),
                module.local_get(param_index as u32, module.i64()),
            ));
        }

        let mut relooper_blocks = Vec::new();

        for block in &routine.blocks {
            let mut block_exprs = Vec::new();

            for instruction in &block.instructions {
                match instruction {
                    Instruction::Nop => {
                        block_exprs.push(module.nop());
                    }
                    Instruction::SubImmediate {
                        destination,
                        operand,
                        source,
                        variant,
                    } => {
                        block_exprs.push(module.local_set(
                            param_count + get_reg_local_index(*destination),
                            module.binary(
                                get_reg_expr(&module, *source, *variant, param_count),
                                module.const_(*operand),
                                BinaryOp::Sub,
                            ),
                        ));
                    }
                    Instruction::StoreRegisterImmediate {
                        address,
                        value,
                        variant,
                    } => {
                        block_exprs.push(module.store(
                            match variant {
                                SizeVariant::Reg32 => StoreVariant::I64L32,
                                SizeVariant::Reg64 => StoreVariant::I64,
                            },
                            get_reg_expr(&module, *value, *variant, param_count),
                            get_reg_expr(&module, address.base, SizeVariant::Reg64, param_count),
                            (address.mode.access_offset()
                                + (match address.base {
                                    Register::SP => memory_info.stack_internal_address as i32,
                                    _ => 0,
                                })) as u32,
                            8,
                            &memory_info.name,
                        ));

                        if let Some(writeback_offset) = address.mode.writeback_offset() {
                            block_exprs.push(module.local_set(
                                param_count + get_reg_local_index(address.base),
                                module.binary(
                                    get_reg_expr(
                                        &module,
                                        address.base,
                                        SizeVariant::Reg64,
                                        param_count,
                                    ),
                                    module.const_(writeback_offset as i64),
                                    BinaryOp::Add,
                                ),
                            ));
                        }
                    }
                    Instruction::MoveWideWithZero {
                        destination,
                        value,
                        variant,
                    } => {
                        block_exprs.push(module.local_set(
                            param_count + get_reg_local_index(*destination),
                            module.const_(*value),
                        ));
                    }
                    // Instruction::Return { target } => {
                    //     assert!(*target == Register::X30);

                    //     block_exprs.push(module.return_(
                    //         module.local_get(param_count + get_reg_local_index(*target), module.i64()),
                    //     ));
                    // }
                    _ => {}
                }
            }

            let by_block = module.block(module.none(), &block_exprs);
            let relooper_block = relooper.add_block(by_block);

            relooper_blocks.push(relooper_block);
        }

        for (block, relooper_block) in routine.blocks.iter().zip(relooper_blocks.iter()) {
            if let Some(fallthrough_block_index) = block.fallthrough_block_index {
                relooper.branch(
                    relooper_block,
                    &relooper_blocks[fallthrough_block_index],
                    None,
                );
            }

            if let Some(jump_block_index) = block.jump_block_index {
                let condition = match &block.instructions.last().unwrap() {
                    Instruction::BranchConditionally { target, condition } => {
                        Some(match condition {
                            Condition::EQ => module.binary(
                                module.local_get(param_count + ZERO_FLAG_LOCAL_INDEX, module.i32()),
                                module.const_(1i32),
                                BinaryOp::Eq,
                            ),
                            _ => todo!(),
                        })
                    }
                    _ => None,
                };

                relooper.branch(
                    relooper_block,
                    &relooper_blocks[jump_block_index],
                    condition,
                );
            }
        }

        routine_exprs.push(relooper.finish(&relooper_blocks[0]));

        let func_block = module.block(module.none(), &routine_exprs);
        let func = module.function(
            routine_name,
            &param_types,
            return_type,
            &local_types,
            func_block,
        );
    }

    module.validate();
    module.print();
    module.save(&mut File::create("output.wasm")?)?;

    Ok(())
}

#[derive(Debug)]
struct MappedSegment<'a> {
    address: u64,
    data: &'a [u8],
    memory_offset: u64,
    size: u64,
    writable: bool,
}

#[derive(Debug)]
pub struct MemoryInfo {
    name: CString,
    stack_internal_address: u64,
    stack_size: u64,
}

pub fn set_up_memory(
    memory_name: &str,
    elf_bytes: &[u8],
    elf_file: ElfBytes<elf::endian::AnyEndian>,
    module: &Module,
) -> MemoryInfo {
    let mut current_offset = 0;
    let mut mapped_segments = Vec::new();

    for segment in elf_file.segments().unwrap() {
        // eprintln!("Segment: {:?}", segment);

        if segment.p_type == elf::abi::PT_LOAD {
            // let is_executable = (segment.p_flags & elf::abi::PF_X) != 0;
            // if is_executable {
            //     eprintln!("Found executable segment at 0x{:x} with {} bytes", segment.p_offset, segment.p_filesz);
            // }

            // eprintln!("{} {}", segment.p_filesz, file_data[(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize].len());

            mapped_segments.push(MappedSegment {
                address: segment.p_vaddr,
                data: &elf_bytes
                    [(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize],
                memory_offset: current_offset,
                size: segment.p_filesz,
                writable: (segment.p_flags & elf::abi::PF_W) != 0,
            });

            // eprintln!("{:?}", mapped_segments.last().unwrap().data);

            current_offset += segment.p_filesz;
        }
    }

    let total_mapped_size = current_offset;

    let segment_names = mapped_segments
        .iter()
        .enumerate()
        .map(|(i, _)| CString::new(format!("segment_{}", i)).unwrap())
        .collect::<Vec<_>>();

    let mut segment_name_ptrs = segment_names.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();

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
                by::BinaryenLiteralInt64(seg.memory_offset as i64),
            )
        })
        .collect::<Vec<_>>();

    let mut segment_sizes = mapped_segments
        .iter()
        .map(|seg| seg.size as u32)
        .collect::<Vec<_>>();

    let mapped_memory_page_count = total_mapped_size.div_ceil(PAGE_SIZE);
    let mapped_memory_size = mapped_memory_page_count * PAGE_SIZE;
    let stack_memory_page_count = 2;
    let stack_memory_internal_address = mapped_memory_size;

    let memory_name = CString::new(memory_name).unwrap();

    unsafe {
        by::BinaryenSetMemory(
            module.by_module,
            (mapped_memory_page_count + stack_memory_page_count) as u32,
            i32::cast_unsigned(-1),
            memory_name.as_ptr(),
            segment_name_ptrs.as_mut_ptr() as *mut *const i8,
            segment_datas.as_mut_ptr() as *mut *const i8,
            segment_passives.as_mut_ptr(),
            segment_offsets.as_mut_ptr(),
            segment_sizes.as_mut_ptr(),
            mapped_segments.len() as u32,
            false,
            true,
            memory_name.as_ptr(),
        );
    }

    MemoryInfo {
        name: memory_name,
        stack_internal_address: stack_memory_internal_address,
        stack_size: stack_memory_page_count * PAGE_SIZE,
    }
}
