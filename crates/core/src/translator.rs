use crate::{
    analysis::{ElfFile, Routine},
    constants::INSTRUCTION_SIZE,
    module::{BinaryOp, Expression, Module, UnaryOp},
};
use arm_decoder::{
    instructions::Instruction,
    structures::{Condition, Register, SizeVariant},
};
use binaryen::ffi as by;
use elf::ElfBytes;

pub const GENERAL_PURPOSE_REGISTER_COUNT: u32 = 32;
pub const PARAMETER_REGISTER_COUNT: u32 = 8;

pub const FIRST_FLAG_LOCAL_INDEX: u32 = 0;
pub const SP_REGISTER_LOCAL_INDEX: u32 = 4;
pub const FIRST_GP_REGISTER_LOCAL_INDEX: u32 = 5;
pub const INTERNAL_CALL_RETURN_LOCAL_INDEX: u32 = FIRST_GP_REGISTER_LOCAL_INDEX + 5;
pub const SVC_CALL_RETURN_LOCAL_INDEX: u32 = INTERNAL_CALL_RETURN_LOCAL_INDEX + 1;

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
pub struct RoutineContext {
    param_count: u32,
    module: Module,
}

impl RoutineContext {
    fn read_flag(&self, flag: Flag) -> Expression {
        self.module.local_get(
            self.param_count + FIRST_FLAG_LOCAL_INDEX + flag.index(),
            self.module.i32(),
        )
    }

    fn get_register_local_index(&self, register: Register) -> u32 {
        use Register::*;

        self.param_count
            + match register {
                SP => SP_REGISTER_LOCAL_INDEX,

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

                X31 => unreachable!(),
                XZR => unreachable!(),
            }
    }

    fn read_register(&self, register: Register, variant: SizeVariant) -> Expression {
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

    fn write_register(
        &mut self,
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

impl RoutineContext {
    pub fn translate_instruction(
        &self,
        instruction: Instruction,
        block_exprs: &mut Vec<Expression>,
    ) {
        match instruction {
            Instruction::Nop => {
                block_exprs.push(self.module.nop());
            }
            _ => todo!(),
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
struct GlobalContext {
    module: Module,
    svc_function_name: String,
    svc_return_type: by::BinaryenType,
}

impl GlobalContext {
    pub fn translate_elf(bytes: &[u8]) -> Result<Module, Box<dyn std::error::Error>> {
        // TODO: Avoid redundancy
        let elf_file = ElfFile::minimal_parse(bytes)?;
        let analysis = crate::analysis::analyze(bytes, elf_file)?;

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
            svc_return_type,
        );

        let memory_info = set_up_memory("memory", bytes, elf_file, &module);

        let context = Self {
            module,
            svc_function_name: svc_function_name.to_string(),
            svc_return_type,
        };

        for (routine_index, routine) in analysis.routines.iter().enumerate() {
            let function_name = &function_names[routine_index];
            context.translate_routine(routine, function_name);
        }

        let module = context.module;

        if let Some(entry_routine_index) = analysis.entry_routine_index {
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

        // let ok = module.validate();

        // if ok {
        //     if optimize {
        //         module.optimize();
        //     }

        //     module.print();
        //     module.save(&mut File::create("output.wasm")?)?;
        // }

        Ok(module)
    }

    pub fn translate_routine(&self, routine: &Routine, function_name: &str) {
        let module = &self.module;

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
            local_types.push(self.svc_return_type);

            local_types
        };

        let mut relooper = module.relooper();

        let context = RoutineContext {
            param_count,
            module: module.clone(),
        };

        let mut routine_exprs = Vec::new();

        for (param_index, param_register) in param_registers.iter().enumerate() {
            routine_exprs.push(module.local_set(
                param_count + get_reg_local_index(*param_register),
                module.local_get(param_index as u32, module.i64()),
            ));
        }

        let mut relooper_blocks = Vec::new();

        for (block_index, block) in routine.blocks.iter().enumerate() {
            let mut block_exprs = Vec::new();

            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let current_address =
                    block.start_address + (instruction_index as u64) * INSTRUCTION_SIZE;

                context.translate_instruction(*instruction, &mut block_exprs);
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
                        Condition::EQ => module.binary(
                            context.read_flag(Flag::Zero),
                            module.const_(0i32),
                            BinaryOp::EqInt32,
                        ),
                        Condition::NE => context.read_flag(Flag::Zero),
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
                            _ => unreachable!(),
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
