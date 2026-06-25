use std::{collections::HashMap, ffi::CString};

use crate::{
    analysis::{Analysis, ElfFile, Routine},
    constants::{INSTRUCTION_SIZE, PAGE_SIZE},
};
use arm_decoder::{
    instructions::Instruction,
    structures::{Condition, Register, SizeVariant},
};
use binaryen::ffi as by;
use binaryen_module::{BinaryOp, Expression, Module, Type, UnaryOp};
use elf::ElfBytes;

pub const GP_REGISTER_COUNT: u32 = 31;
pub const GP_FLAG_COUNT: u32 = 4;

pub const FIRST_FLAG_LOCAL_INDEX: u32 = 0;
pub const SP_REGISTER_LOCAL_INDEX: u32 = 4;
pub const FIRST_GP_REGISTER_LOCAL_INDEX: u32 = 5;
pub const INTERNAL_CALL_RETURN_LOCAL_INDEX: u32 = FIRST_GP_REGISTER_LOCAL_INDEX + 5;
pub const SVC_CALL_RETURN_LOCAL_INDEX: u32 = INTERNAL_CALL_RETURN_LOCAL_INDEX + 1;

pub const DEFAULT_PARAM_REGISTERS: [Register; 9] = [
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Carry,
    Negative,
    Zero,
    Overflow,
}

impl Flag {
    pub fn index(&self) -> u32 {
        match self {
            Flag::Carry => 0,
            Flag::Negative => 1,
            Flag::Zero => 2,
            Flag::Overflow => 3,
        }
    }
}

#[derive(Debug)]
pub struct RoutineContext<'a> {
    pub global: &'a GlobalContext,

    first_flag_local_index: u32,
    local_index_by_register: HashMap<Register, u32>,
    pub func_return_scratch_local_index: u32,
    pub module: Module,
    pub return_registers: Vec<Register>,
    pub svc_return_scratch_local_index: u32,
}

impl RoutineContext<'_> {
    pub fn read_flag(&self, flag: Flag) -> Expression {
        self.module.local_get(
            self.first_flag_local_index + flag.index(),
            self.module.i32(),
        )
    }

    pub fn get_register_local_index(&self, register: Register) -> u32 {
        use Register::*;

        self.local_index_by_register
            .get(&register)
            .copied()
            .unwrap()
    }

    pub fn read_register(&self, register: Register, variant: SizeVariant) -> Expression {
        match register {
            Register::XZR => match variant {
                SizeVariant::Reg32 => self.module.const_(0i32),
                SizeVariant::Reg64 => self.module.const_(0i64),
            },
            _ => {
                let expr = self
                    .module
                    .local_get(self.get_register_local_index(register), self.module.i64());

                match variant {
                    SizeVariant::Reg32 => self.module.unary(expr, UnaryOp::WrapInt64),
                    SizeVariant::Reg64 => expr,
                }
            }
        }
    }

    pub fn write_flag(&self, flag: Flag, value: Expression) -> Expression {
        self.module
            .local_set(self.first_flag_local_index + flag.index(), value)
    }

    pub fn write_register(
        &self,
        register: Register,
        variant: SizeVariant,
        value: Expression,
    ) -> Expression {
        match register {
            Register::XZR => self.module.nop(),
            _ => {
                let value = match variant {
                    SizeVariant::Reg32 => self.module.unary(value, UnaryOp::ExtendUInt32),
                    SizeVariant::Reg64 => value,
                };

                self.module
                    .local_set(self.get_register_local_index(register), value)
            }
        }
    }
}

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

#[derive(Debug)]
pub struct GlobalContext {
    module: Module,

    pub analysis: Analysis,
    pub function_names: Vec<String>,
    pub memory_name: CString,
    pub svc_function_name: String,
    pub svc_return_type: Type,
}

impl GlobalContext {
    pub fn translate_elf(bytes: &[u8]) -> Result<Module, Box<dyn std::error::Error>> {
        // TODO: Avoid redundancy
        let elf_file = ElfFile::minimal_parse(bytes)?;
        let analysis = crate::analysis::analyze(bytes, &elf_file)?;

        let module = Module::new();

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

        let svc_return_type = module.tuple_type(
            &SVC_RETURN_REGISTERS
                .iter()
                .map(|_| module.i64())
                .collect::<Vec<_>>(),
        );

        let svc_function_name = "svc";

        module.import_function(
            svc_function_name,
            "ref",
            "supervisor_call",
            &(std::iter::once(module.i32())
                .chain(SVC_PARAM_REGISTERS.iter().map(|_| module.i64()))
                .collect::<Vec<_>>()),
            svc_return_type.clone(),
        );

        let memory_info = set_up_memory("memory", bytes, &elf_file, &module);

        let context = Self {
            analysis,
            function_names,
            memory_name: memory_info.name,
            module: module.clone(),
            svc_function_name: svc_function_name.to_string(),
            svc_return_type,
        };

        for (routine_index, routine) in context.analysis.routines.iter().enumerate() {
            let function_name = &context.function_names[routine_index];
            context.translate_routine(routine, function_name);
        }

        if let Some(entry_routine_index) = context.analysis.entry_routine_index {
            // TODO: Avoid redundancy
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

            let param_types = param_registers
                .iter()
                .map(|_| module.i64())
                .collect::<Vec<_>>();

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
                            &context.function_names[entry_routine_index],
                            &arg_exprs,
                            module.tuple_type(&param_types),
                        )),
                        module.unreachable(),
                    ],
                ),
            );

            module.export_function(entry_function_name, "_entry");
        }

        Ok(module)
    }

    pub fn translate_routine(&self, routine: &Routine, function_name: &str) {
        let module = &self.module;

        // Allocate parameters and locals

        let param_registers = DEFAULT_PARAM_REGISTERS;
        let param_count = param_registers.len() as u32;

        let param_types = param_registers
            .iter()
            .map(|_| module.i64())
            .collect::<Vec<_>>();

        let return_type = module.tuple_type(&param_types);

        let mut next_local_index = 0;

        let mut local_index_by_register = param_registers
            .iter()
            .enumerate()
            .map(|(param_index, register)| {
                let local_index = param_index as u32;
                next_local_index = next_local_index.max(local_index + 1);
                (*register, local_index)
            })
            .collect::<HashMap<_, _>>();

        let mut local_types = Vec::new();

        for reg_index in 0..GP_REGISTER_COUNT {
            let reg = Register::decode(reg_index, false, true);

            if !local_index_by_register.contains_key(&reg) {
                let local_index = next_local_index;
                next_local_index += 1;

                local_index_by_register.insert(reg, local_index);
                local_types.push(module.i64());
            }
        }

        let first_flag_local_index = next_local_index;

        for _ in 0..GP_FLAG_COUNT {
            let local_index = next_local_index;
            next_local_index += 1;

            local_types.push(module.i32());
        }

        // Additional locals for temporary values
        let func_return_scratch_local_index = next_local_index;
        local_types.push(return_type.clone());
        next_local_index += 1;

        let svc_return_scratch_local_index = next_local_index;
        local_types.push(self.svc_return_type.clone());
        next_local_index += 1;

        _ = next_local_index;

        // Run translation

        let relooper = module.relooper();

        let context = RoutineContext {
            global: self,

            first_flag_local_index,
            func_return_scratch_local_index,
            local_index_by_register,
            module: module.clone(),
            return_registers: param_registers.to_vec(),
            svc_return_scratch_local_index,
        };

        let mut routine_exprs = Vec::new();

        for (param_index, param_register) in param_registers.iter().enumerate() {
            // routine_exprs.push(module.local_set(
            //     param_count + get_reg_local_index(*param_register),
            //     module.local_get(param_index as u32, module.i64()),
            // ));
            // TODO
        }

        let mut relooper_blocks = Vec::new();

        for (block_index, block) in routine.blocks.iter().enumerate() {
            let mut block_exprs = Vec::new();

            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let current_address =
                    block.start_address + (instruction_index as u64) * INSTRUCTION_SIZE;

                context.translate_instruction(current_address, instruction, &mut block_exprs);
            }

            if block.fallthrough_block_index.is_none() && block.jump_block_index.is_none() {
                block_exprs.push(module.unreachable());
                // eprintln!("Unreachable block at index {}", block_index);
            }

            let by_block = module.block(module.none(), &block_exprs);
            let relooper_block = relooper.add_block(by_block);

            relooper_blocks.push(relooper_block);
        }

        // eprintln!("Blocks: {:#?}", routine.blocks);

        for (block_index, (block, relooper_block)) in routine
            .blocks
            .iter()
            .zip(relooper_blocks.iter())
            .enumerate()
        {
            if let Some(fallthrough_block_index) = block.fallthrough_block_index {
                relooper.branch(
                    relooper_block,
                    &relooper_blocks[fallthrough_block_index],
                    None,
                );

                // eprintln!("Branch {} -> {}", block_index, fallthrough_block_index);
            }

            if let Some(jump_block_index) = block.jump_block_index {
                let last_instruction = block.instructions.last().unwrap();
                let condition_expr = match last_instruction {
                    Instruction::UnconditionalBranch { .. } => None,
                    Instruction::BranchConditionally { condition, .. } => Some(match condition {
                        Condition::EQ => context.read_flag(Flag::Zero),
                        Condition::NE => {
                            module.unary(context.read_flag(Flag::Zero), UnaryOp::EqZInt32)
                        }

                        // N != V
                        Condition::LT => module.binary(
                            context.read_flag(Flag::Negative),
                            context.read_flag(Flag::Overflow),
                            BinaryOp::NeInt32,
                        ),

                        // !Z && (N == V)
                        Condition::GT => module.binary(
                            module.unary(context.read_flag(Flag::Negative), UnaryOp::EqZInt32),
                            module.binary(
                                context.read_flag(Flag::Zero),
                                context.read_flag(Flag::Overflow),
                                BinaryOp::EqInt32,
                            ),
                            BinaryOp::AndInt32,
                        ),
                        _ => todo!(),
                    }),
                    Instruction::CompareAndBranch {
                        branch_if_zero,
                        target,
                        register,
                        variant,
                    } => Some(module.binary(
                        context.read_register(*register, *variant),
                        match variant {
                            SizeVariant::Reg32 => module.const_(0i32),
                            SizeVariant::Reg64 => module.const_(0i64),
                        },
                        match (branch_if_zero, variant) {
                            (false, SizeVariant::Reg32) => BinaryOp::NeInt32,
                            (false, SizeVariant::Reg64) => BinaryOp::NeInt64,
                            (true, SizeVariant::Reg32) => BinaryOp::EqInt32,
                            (true, SizeVariant::Reg64) => BinaryOp::EqInt64,
                        },
                    )),
                    _ => todo!(),
                };

                relooper.branch(
                    relooper_block,
                    &relooper_blocks[jump_block_index],
                    condition_expr,
                );

                // if condition_expr.is_some() {
                //     eprintln!(
                //         "Branch {} -> {} with condition",
                //         block_index, jump_block_index
                //     );
                // } else {
                //     eprintln!("Branch {} -> {}", block_index, jump_block_index);
                // }
            }
        }

        routine_exprs.push(relooper.finish(&relooper_blocks[0]));

        let func_block = module.block(module.none(), &routine_exprs);
        let func = module.function(
            function_name,
            &param_types,
            return_type,
            &local_types,
            func_block,
        );
    }
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
    pub name: CString,
    pub stack_internal_address: u64,
    pub stack_size: u64,
}

pub fn set_up_memory(
    memory_name: &str,
    elf_bytes: &[u8],
    elf_file: &ElfFile,
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
        .map(|seg| unsafe { module.const_(seg.address).unsafe_ptr() })
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
            module.unsafe_ptr(),
            (mapped_memory_page_count + stack_memory_page_count) as u32,
            u32::MAX,
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
