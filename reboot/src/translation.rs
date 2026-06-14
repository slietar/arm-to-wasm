use std::{ffi::CString, fs::File};

use binaryen::ffi as by;
use elf::ElfBytes;

use crate::{
    constants::{INSTRUCTION_SIZE, PAGE_SIZE},
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
pub const INTERNAL_CALL_RETURN_LOCAL_INDEX: u32 = 5 + GENERAL_PURPOSE_REGISTER_COUNT;
pub const SVC_CALL_RETURN_LOCAL_INDEX: u32 = INTERNAL_CALL_RETURN_LOCAL_INDEX + 1;

pub const SVC_PARAM_REGISTERS: [Register; 7] = [
    Register::X8,
    Register::X0,
    Register::X1,
    Register::X2,
    Register::X3,
    Register::X4,
    Register::X5,
];

pub const SVC_RETURN_REGISTERS: [Register; 2] = [Register::X0, Register::X1];

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

    let function_names = analysis
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

    let svc_function_name = "supervisor_call";
    let svc_return_type = module.tuple_type(
        &SVC_RETURN_REGISTERS
            .iter()
            .map(|_| module.i64())
            .collect::<Vec<_>>(),
    );

    module.import_function(
        svc_function_name,
        "ref",
        "supervisor_call",
        &(std::iter::once(module.i32())
            .chain(SVC_PARAM_REGISTERS.iter().map(|_| module.i64()))
            .collect::<Vec<_>>()),
        svc_return_type,
    );

    let return_type = module.tuple_type(&param_types);

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

        // Additional locals for temporary values
        local_types.push(return_type);
        local_types.push(svc_return_type);

        local_types
    };

    let elf_file = ElfBytes::<elf::endian::AnyEndian>::minimal_parse(elf_bytes)?;
    let memory_info = set_up_memory("memory", elf_bytes, elf_file, &module);

    // eprintln!(
    //     "{:#?}",
    //     memory_info
    // );

    for (routine_index, routine) in analysis.routines.iter().enumerate() {
        let routine_name = &function_names[routine_index];

        // if routine.name.as_deref().unwrap() != "_start" {
        //     continue;
        // }

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

            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let current_address =
                    block.start_address + (instruction_index as u64) * INSTRUCTION_SIZE;

                match instruction {
                    Instruction::Nop => {
                        block_exprs.push(module.nop());
                    }
                    Instruction::AddImmediate {
                        destination,
                        operand,
                        source,
                        variant,
                    } => {
                        let read_expr = get_reg_expr(&module, *source, *variant, param_count);
                        let write_expr = match variant {
                            SizeVariant::Reg32 => module.unary(
                                module.binary(
                                    read_expr,
                                    module.const_(*operand as i32),
                                    BinaryOp::Add,
                                ),
                                UnaryOp::ExtendUInt32,
                            ),
                            SizeVariant::Reg64 => {
                                module.binary(read_expr, module.const_(*operand), BinaryOp::Add)
                            }
                        };

                        block_exprs.push(module.local_set(
                            param_count + get_reg_local_index(*destination),
                            write_expr,
                        ));
                    }
                    // TODO: Implement Reg32
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
                            module.binary(
                                get_reg_expr(
                                    &module,
                                    address.base,
                                    SizeVariant::Reg64,
                                    param_count,
                                ),
                                module.const_(address.mode.access_offset() as i64),
                                BinaryOp::Add,
                            ),
                            0,
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
                    Instruction::BranchWithLink { target } => {
                        let target_address =
                            ((current_address as i64) + *target * (INSTRUCTION_SIZE as i64)) as u64;
                        let target_routine_index = analysis
                            .routines
                            .iter()
                            .position(|r| r.address == target_address)
                            .unwrap();
                        let target_function_name = &function_names[target_routine_index];

                        let arg_exprs = param_registers
                            .iter()
                            .map(|reg| get_reg_expr(&module, *reg, SizeVariant::Reg64, param_count))
                            .collect::<Vec<_>>();

                        block_exprs.push(module.local_set(
                            param_count + INTERNAL_CALL_RETURN_LOCAL_INDEX,
                            module.call(target_function_name, &arg_exprs, return_type),
                        ));

                        for (param_index, param_register) in param_registers.iter().enumerate() {
                            block_exprs.push(module.local_set(
                                param_count + get_reg_local_index(*param_register),
                                module.tuple_extract(
                                    module.local_get(
                                        param_count + INTERNAL_CALL_RETURN_LOCAL_INDEX,
                                        return_type,
                                    ),
                                    param_index as u32,
                                ),
                            ));
                        }
                    }
                    Instruction::Return { target } => {
                        assert!(*target == Register::X30);

                        let return_exprs = param_registers
                            .iter()
                            .map(|reg| get_reg_expr(&module, *reg, SizeVariant::Reg64, param_count))
                            .collect::<Vec<_>>();

                        block_exprs.push(module.return_(module.tuple(&return_exprs)));
                    }
                    Instruction::SupervisorCall { argument } => {
                        let arg_exprs = std::iter::once(module.const_(*argument as u32))
                            .chain(SVC_PARAM_REGISTERS.iter().map(|reg| {
                                get_reg_expr(&module, *reg, SizeVariant::Reg64, param_count)
                            }))
                            .collect::<Vec<_>>();

                        block_exprs.push(module.local_set(
                            param_count + SVC_CALL_RETURN_LOCAL_INDEX,
                            module.call(svc_function_name, &arg_exprs, svc_return_type),
                        ));

                        for (return_index, return_register) in
                            SVC_RETURN_REGISTERS.iter().enumerate()
                        {
                            block_exprs.push(module.local_set(
                                param_count + get_reg_local_index(*return_register),
                                module.tuple_extract(
                                    module.local_get(
                                        param_count + SVC_CALL_RETURN_LOCAL_INDEX,
                                        svc_return_type,
                                    ),
                                    return_index as u32,
                                ),
                            ));
                        }
                    }
                    Instruction::FormPCRelativeAddress { destination, value } => {
                        let address = ((current_address as i64) + *value) as u64;

                        block_exprs.push(module.local_set(
                            param_count + get_reg_local_index(*destination),
                            module.const_(address),
                        ));
                    }
                    Instruction::FormPCRelativeAddressToPage { destination, value } => {
                        let address = ((current_address & !0xfff) as i64 + *value) as u64;

                        block_exprs.push(module.local_set(
                            param_count + get_reg_local_index(*destination),
                            module.const_(address),
                        ));
                    }
                    _ => {
                        eprintln!(
                            "Unimplemented instruction at {:#x}: {:?}",
                            current_address, instruction
                        );
                    }
                }
            }

            if block.fallthrough_block_index.is_none() && block.jump_block_index.is_none() {
                block_exprs.push(module.unreachable());
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

    if let Some(entry_routine_index) = analysis.entry_routine_index {
        let arg_exprs = param_registers
            .iter()
            .map(|reg| match *reg {
                Register::SP => module.const_(memory_info.stack_internal_address as i64),
                _ => module.const_(0i64),
            })
            .collect::<Vec<_>>();

        let entry_function_name = "entry";
        let entry_function = module.function(
            entry_function_name,
            &[],
            module.none(),
            &[],
            module.block(
                module.none(),
                &[
                    module.drop(module.call(
                        &function_names[entry_routine_index],
                        &arg_exprs,
                        module.tuple_type(&param_types),
                    )),
                    module.unreachable(),
                ],
            ),
        );

        module.export_function(entry_function_name, "_entry");
    }

    module.print();
    module.validate();
    module.optimize();
    module.print();
    module.save(&mut File::create("output.wasm")?)?;

    Ok(())
}

#[derive(Debug)]
struct MappedSegment<'a> {
    address: u64,
    data: &'a [u8],
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
    let mut mapped_segments = Vec::new();

    for segment in elf_file.segments().unwrap() {
        // eprintln!("Segment: {:?}", segment);

        if segment.p_type == elf::abi::PT_LOAD {
            mapped_segments.push(MappedSegment {
                address: segment.p_vaddr,
                data: &elf_bytes
                    [(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize],
                size: segment.p_filesz,
                writable: (segment.p_flags & elf::abi::PF_W) != 0,
            });
        }
    }

    let total_mapped_size = mapped_segments
        .iter()
        .map(|seg| seg.address + seg.size)
        .max()
        .unwrap_or(0)
        .div_ceil(PAGE_SIZE as u64)
        * PAGE_SIZE as u64;

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
        .map(|seg| unsafe { module.const_(seg.address).extract() })
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
