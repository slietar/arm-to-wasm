use arm_decoder::{
    instructions::{AddSubtractOp, AddSubtractRightOperand, BranchTarget, Instruction, LogicalOp},
    structures::{Register, Shift, SizeVariant, SliceSize},
    utilities::INSTRUCTION_SIZE,
};
use binaryen_module::{BinaryOp, Expression, UnaryOp};

use crate::translator::{
    DEFAULT_PARAM_REGISTERS, Flag, RoutineContext, SVC_PARAM_REGISTERS, SVC_RETURN_REGISTERS,
};

impl RoutineContext<'_> {
    pub fn translate_instruction(
        &self,
        address: u64,
        instruction: &Instruction,
        block_exprs: &mut Vec<Expression>,
    ) {
        match instruction {
            Instruction::AddSubtract {
                destination,
                op,
                set_flags,
                operand1,
                operand2,
                variant,
            } => {
                let get_op1 = || self.read_register(*operand1, *variant);

                let get_op2 = || -> Expression {
                    match operand2 {
                        AddSubtractRightOperand::Immediate(imm) => match variant {
                            SizeVariant::Reg32 => self.module.const_(*imm as i32),
                            SizeVariant::Reg64 => self.module.const_(*imm as i64),
                        },
                        AddSubtractRightOperand::ShiftedRegister {
                            register,
                            shift_amount,
                            shift_type,
                        } => {
                            let reg = self.read_register(*register, *variant);
                            if *shift_amount == 0 {
                                reg
                            } else {
                                let (amount_expr, shift_op) = match (shift_type, variant) {
                                    (Shift::LSL, SizeVariant::Reg32) => (
                                        self.module.const_(*shift_amount as i32),
                                        BinaryOp::ShlInt32,
                                    ),
                                    (Shift::LSL, SizeVariant::Reg64) => (
                                        self.module.const_(*shift_amount as i64),
                                        BinaryOp::ShlInt64,
                                    ),
                                    (Shift::LSR, SizeVariant::Reg32) => (
                                        self.module.const_(*shift_amount as i32),
                                        BinaryOp::ShrUInt32,
                                    ),
                                    (Shift::LSR, SizeVariant::Reg64) => (
                                        self.module.const_(*shift_amount as i64),
                                        BinaryOp::ShrUInt64,
                                    ),
                                    (Shift::ASR, SizeVariant::Reg32) => (
                                        self.module.const_(*shift_amount as i32),
                                        BinaryOp::ShrSInt32,
                                    ),
                                    (Shift::ASR, SizeVariant::Reg64) => (
                                        self.module.const_(*shift_amount as i64),
                                        BinaryOp::ShrSInt64,
                                    ),
                                    (Shift::ROR, SizeVariant::Reg32) => (
                                        self.module.const_(*shift_amount as i32),
                                        BinaryOp::RotRInt32,
                                    ),
                                    (Shift::ROR, SizeVariant::Reg64) => (
                                        self.module.const_(*shift_amount as i64),
                                        BinaryOp::RotRInt64,
                                    ),
                                };
                                self.module.binary(reg, amount_expr, shift_op)
                            }
                        }
                        AddSubtractRightOperand::ExtendedRegister {
                            extension,
                            left_shift_amount,
                            register,
                        } => {
                            let reg64 = self.read_register(*register, SizeVariant::Reg64);

                            // Apply zero/sign extension to i64
                            let extended = match (extension.size, extension.signed) {
                                (SliceSize::Byte, false) => self.module.binary(
                                    reg64,
                                    self.module.const_(0xFFi64),
                                    BinaryOp::AndInt64,
                                ),
                                (SliceSize::Byte, true) => {
                                    let masked = self.module.binary(
                                        reg64,
                                        self.module.const_(0xFFi64),
                                        BinaryOp::AndInt64,
                                    );
                                    let shl = self.module.binary(
                                        masked,
                                        self.module.const_(56i64),
                                        BinaryOp::ShlInt64,
                                    );
                                    self.module.binary(
                                        shl,
                                        self.module.const_(56i64),
                                        BinaryOp::ShrSInt64,
                                    )
                                }
                                (SliceSize::Halfword, false) => self.module.binary(
                                    reg64,
                                    self.module.const_(0xFFFFi64),
                                    BinaryOp::AndInt64,
                                ),
                                (SliceSize::Halfword, true) => {
                                    let masked = self.module.binary(
                                        reg64,
                                        self.module.const_(0xFFFFi64),
                                        BinaryOp::AndInt64,
                                    );
                                    let shl = self.module.binary(
                                        masked,
                                        self.module.const_(48i64),
                                        BinaryOp::ShlInt64,
                                    );
                                    self.module.binary(
                                        shl,
                                        self.module.const_(48i64),
                                        BinaryOp::ShrSInt64,
                                    )
                                }
                                (SliceSize::Word, false) => self.module.binary(
                                    reg64,
                                    self.module.const_(0xFFFF_FFFFi64),
                                    BinaryOp::AndInt64,
                                ),
                                (SliceSize::Word, true) => {
                                    let wrapped = self.module.unary(reg64, UnaryOp::WrapInt64);
                                    self.module.unary(wrapped, UnaryOp::ExtendSInt32)
                                }
                                (SliceSize::Doubleword, _) => reg64,
                            };

                            // Apply left shift
                            let shifted = if *left_shift_amount == 0 {
                                extended
                            } else {
                                self.module.binary(
                                    extended,
                                    self.module.const_(*left_shift_amount as i64),
                                    BinaryOp::ShlInt64,
                                )
                            };

                            // Truncate to variant width
                            match variant {
                                SizeVariant::Reg32 => {
                                    self.module.unary(shifted, UnaryOp::WrapInt64)
                                }
                                SizeVariant::Reg64 => shifted,
                            }
                        }
                    }
                };

                // Arithmetic result in variant type (i32 or i64)
                let get_arith = || {
                    self.module.binary(
                        get_op1(),
                        get_op2(),
                        match (op, variant) {
                            (AddSubtractOp::Add, SizeVariant::Reg32) => BinaryOp::AddInt32,
                            (AddSubtractOp::Add, SizeVariant::Reg64) => BinaryOp::AddInt64,
                            (AddSubtractOp::Subtract, SizeVariant::Reg32) => BinaryOp::SubInt32,
                            (AddSubtractOp::Subtract, SizeVariant::Reg64) => BinaryOp::SubInt64,
                        },
                    )
                };

                // Zero-extend i32 result to i64 before writing to register
                let write_value = match variant {
                    SizeVariant::Reg32 => self.module.unary(get_arith(), UnaryOp::ExtendUInt32),
                    SizeVariant::Reg64 => get_arith(),
                };
                block_exprs.push(self.write_register(
                    *destination,
                    SizeVariant::Reg64,
                    write_value,
                ));

                if *set_flags {
                    // Sign bit helper: returns i32 value 0 or 1
                    let sign = |expr: Expression| match variant {
                        SizeVariant::Reg32 => {
                            self.module
                                .binary(expr, self.module.const_(0i32), BinaryOp::LtSInt32)
                        }
                        SizeVariant::Reg64 => {
                            self.module
                                .binary(expr, self.module.const_(0i64), BinaryOp::LtSInt64)
                        }
                    };

                    // N flag: result < 0 (signed)
                    block_exprs.push(self.write_flag(Flag::Negative, sign(get_arith())));

                    // Z flag: result == 0
                    let zero_expr = match variant {
                        SizeVariant::Reg32 => self.module.binary(
                            get_arith(),
                            self.module.const_(0i32),
                            BinaryOp::EqInt32,
                        ),
                        SizeVariant::Reg64 => self.module.binary(
                            get_arith(),
                            self.module.const_(0i64),
                            BinaryOp::EqInt64,
                        ),
                    };
                    block_exprs.push(self.write_flag(Flag::Zero, zero_expr));

                    // C flag
                    let carry_expr = match op {
                        // Add: unsigned overflow, i.e. result <u op1
                        AddSubtractOp::Add => match variant {
                            SizeVariant::Reg32 => {
                                self.module
                                    .binary(get_arith(), get_op1(), BinaryOp::LtUInt32)
                            }
                            SizeVariant::Reg64 => {
                                self.module
                                    .binary(get_arith(), get_op1(), BinaryOp::LtUInt64)
                            }
                        },
                        // Sub: no borrow, i.e. op1 >=u op2
                        AddSubtractOp::Subtract => match variant {
                            SizeVariant::Reg32 => {
                                self.module.binary(get_op1(), get_op2(), BinaryOp::GeUInt32)
                            }
                            SizeVariant::Reg64 => {
                                self.module.binary(get_op1(), get_op2(), BinaryOp::GeUInt64)
                            }
                        },
                    };
                    block_exprs.push(self.write_flag(Flag::Carry, carry_expr));

                    // V flag (signed overflow)
                    let overflow_expr = match op {
                        // Add: V = (op1_sign == op2_sign) AND (result_sign != op1_sign)
                        AddSubtractOp::Add => self.module.binary(
                            self.module.unary(
                                self.module.binary(
                                    sign(get_op1()),
                                    sign(get_op2()),
                                    BinaryOp::XorInt32,
                                ),
                                UnaryOp::EqZInt32,
                            ),
                            self.module.binary(
                                sign(get_op1()),
                                sign(get_arith()),
                                BinaryOp::XorInt32,
                            ),
                            BinaryOp::AndInt32,
                        ),
                        // Sub: V = (op1_sign != op2_sign) AND (result_sign != op1_sign)
                        AddSubtractOp::Subtract => self.module.binary(
                            self.module.binary(
                                sign(get_op1()),
                                sign(get_op2()),
                                BinaryOp::XorInt32,
                            ),
                            self.module.binary(
                                sign(get_op1()),
                                sign(get_arith()),
                                BinaryOp::XorInt32,
                            ),
                            BinaryOp::AndInt32,
                        ),
                    };

                    block_exprs.push(self.write_flag(Flag::Overflow, overflow_expr));
                }
            }

            Instruction::LogicalImmediate {
                destination,
                op,
                operand1,
                operand2,
                variant,
            } => {
                let op1 = self.read_register(*operand1, *variant);
                let op2 = match variant {
                    SizeVariant::Reg32 => self.module.const_(*operand2 as i32),
                    SizeVariant::Reg64 => self.module.const_(*operand2 as i64),
                };

                let (binary_op, set_flags) = match op {
                    LogicalOp::And { set_flags } => (
                        match variant {
                            SizeVariant::Reg32 => BinaryOp::AndInt32,
                            SizeVariant::Reg64 => BinaryOp::AndInt64,
                        },
                        *set_flags,
                    ),
                    LogicalOp::Or => (
                        match variant {
                            SizeVariant::Reg32 => BinaryOp::OrInt32,
                            SizeVariant::Reg64 => BinaryOp::OrInt64,
                        },
                        false,
                    ),
                    LogicalOp::Xor => (
                        match variant {
                            SizeVariant::Reg32 => BinaryOp::XorInt32,
                            SizeVariant::Reg64 => BinaryOp::XorInt64,
                        },
                        false,
                    ),
                };

                let get_arith = || {
                    let op1 = self.read_register(*operand1, *variant);
                    let op2 = match variant {
                        SizeVariant::Reg32 => self.module.const_(*operand2 as i32),
                        SizeVariant::Reg64 => self.module.const_(*operand2 as i64),
                    };

                    self.module.binary(op1, op2, binary_op)
                };

                block_exprs.push(self.write_register(
                    *destination,
                    *variant,
                    self.module.binary(op1, op2, binary_op),
                ));

                if set_flags {
                    // N flag: result < 0 (signed)
                    let sign_expr = match variant {
                        SizeVariant::Reg32 => self.module.binary(
                            get_arith(),
                            self.module.const_(0i32),
                            BinaryOp::LtSInt32,
                        ),
                        SizeVariant::Reg64 => self.module.binary(
                            get_arith(),
                            self.module.const_(0i64),
                            BinaryOp::LtSInt64,
                        ),
                    };
                    block_exprs.push(self.write_flag(Flag::Negative, sign_expr));

                    // Z flag: result == 0
                    let zero_expr = match variant {
                        SizeVariant::Reg32 => self.module.binary(
                            get_arith(),
                            self.module.const_(0i32),
                            BinaryOp::EqInt32,
                        ),
                        SizeVariant::Reg64 => self.module.binary(
                            get_arith(),
                            self.module.const_(0i64),
                            BinaryOp::EqInt64,
                        ),
                    };
                    block_exprs.push(self.write_flag(Flag::Zero, zero_expr));

                    // C and V flags are always cleared by logical operations
                    block_exprs.push(self.write_flag(Flag::Carry, self.module.const_(0i32)));
                    block_exprs.push(self.write_flag(Flag::Overflow, self.module.const_(0i32)));
                }
            }

            Instruction::UnconditionalBranch {
                link: true,
                target: BranchTarget::RelativeInstructionOffset(target_offset),
            } => {
                let target_address =
                    ((address as i64) + *target_offset * (INSTRUCTION_SIZE as i64)) as u64;

                let target_routine_index = self
                    .global
                    .analysis
                    .routines
                    .iter()
                    .position(|r| r.address == target_address)
                    .unwrap();
                let target_function_name = &self.global.function_names[target_routine_index];

                let return_registers = DEFAULT_PARAM_REGISTERS.to_vec();
                let return_registers = self.return_registers.clone();

                let arg_exprs = return_registers
                    .iter()
                    .map(|reg| self.read_register(*reg, SizeVariant::Reg64))
                    .collect::<Vec<_>>();

                let return_type = self.module.tuple_type(
                    &(return_registers
                        .iter()
                        .map(|_| self.module.i64())
                        .collect::<Vec<_>>()),
                );

                block_exprs.push(
                    self.module.local_set(
                        self.func_return_scratch_local_index,
                        self.module
                            .call(target_function_name, &arg_exprs, return_type.clone()),
                    ),
                );

                for (return_index, return_register) in return_registers.iter().enumerate() {
                    block_exprs.push(self.write_register(
                        *return_register,
                        SizeVariant::Reg64,
                        self.module.tuple_extract(
                            self.module.local_get(
                                self.func_return_scratch_local_index,
                                return_type.clone(),
                            ),
                            return_index as u32,
                        ),
                    ));
                }
            }

            Instruction::SupervisorCall { argument } => {
                let arg_exprs = std::iter::once(self.module.const_(*argument as u32))
                    .chain(
                        SVC_PARAM_REGISTERS
                            .iter()
                            .map(|reg| self.read_register(*reg, SizeVariant::Reg64)),
                    )
                    .collect::<Vec<_>>();

                block_exprs.push(self.module.local_set(
                    self.svc_return_scratch_local_index,
                    self.module.call(
                        &self.global.svc_function_name,
                        &arg_exprs,
                        self.global.svc_return_type.clone(),
                    ),
                ));

                for (return_index, return_register) in SVC_RETURN_REGISTERS.iter().enumerate() {
                    block_exprs.push(self.write_register(
                        *return_register,
                        SizeVariant::Reg64,
                        self.module.tuple_extract(
                            self.module.local_get(
                                self.svc_return_scratch_local_index,
                                self.global.svc_return_type.clone(),
                            ),
                            return_index as u32,
                        ),
                    ));
                }
            }

            Instruction::Nop => {
                block_exprs.push(self.module.nop());
            }
            Instruction::Return { target } => {
                assert!(*target == Register::X30);

                let return_exprs = self
                    .return_registers
                    .iter()
                    .map(|reg| self.read_register(*reg, SizeVariant::Reg64))
                    .collect::<Vec<_>>();

                block_exprs.push(self.module.return_(self.module.tuple(&return_exprs)));
            }
            _ => {
                block_exprs.push(self.module.nop());
            }
        }
    }
}
