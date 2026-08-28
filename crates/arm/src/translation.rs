use arm_decoder::{
    instructions::{
        AddSubtractOp, AddSubtractRightOperand, BivalentObject, BranchTarget, GPLoadMode,
        IndexMode, Instruction, LoadStoreIndex, LoadStoreOp, LogicalImmediateOperand, LogicalOp,
    },
    structures::{Condition, Register, Shift, SizeVariant, Sized, SliceSize, WritebackOffset},
    utilities::INSTRUCTION_SIZE,
};
use bnyr::{BinaryOp, Expression, Module, LoadVariant, StoreVariant, UnaryOp};

use aw_core::architecture::{BranchKind, Instr, StackAccess, Width};
use aw_core::translator::RoutineContext;

use crate::instruction_helper::InstructionInfo;
use crate::registers::{self, Flag};

fn width(variant: SizeVariant) -> Width {
    match variant {
        SizeVariant::Reg32 => Width::W32,
        SizeVariant::Reg64 => Width::W64,
    }
}

/// Wraps `arm_decoder::instructions::Instruction` so `aw_core::architecture::Instr`
/// can be implemented for it (both are foreign to this crate, so the orphan rule
/// requires a local newtype).
#[derive(Debug)]
pub struct ArmInstruction(pub Instruction);

impl Instr for ArmInstruction {
    fn translate(&self, ctx: &RoutineContext, current_address: u64, block_exprs: &mut Vec<Expression>) {
        match &self.0 {
            Instruction::AddSubtract {
                destination,
                op,
                set_flags,
                operand1,
                operand2,
                variant,
            } => {
                let get_op1 = || ctx.read_register(registers::id(*operand1), width(*variant));

                let get_op2 = || -> Expression {
                    match operand2 {
                        AddSubtractRightOperand::Immediate(imm) => match variant {
                            SizeVariant::Reg32 => ctx.module.const_(*imm as i32),
                            SizeVariant::Reg64 => ctx.module.const_(*imm as i64),
                        },
                        AddSubtractRightOperand::ShiftedRegister {
                            register,
                            shift_amount,
                            shift_type,
                        } => {
                            let register_expr = ctx.read_register(registers::id(*register), width(*variant));

                            if *shift_amount == 0 {
                                register_expr
                            } else {
                                let (amount_expr, shift_op) = match (shift_type, variant) {
                                    (Shift::LSL, SizeVariant::Reg32) => (
                                        ctx.module.const_(*shift_amount as i32),
                                        BinaryOp::ShlInt32,
                                    ),
                                    (Shift::LSL, SizeVariant::Reg64) => (
                                        ctx.module.const_(*shift_amount as i64),
                                        BinaryOp::ShlInt64,
                                    ),
                                    (Shift::LSR, SizeVariant::Reg32) => (
                                        ctx.module.const_(*shift_amount as i32),
                                        BinaryOp::ShrUInt32,
                                    ),
                                    (Shift::LSR, SizeVariant::Reg64) => (
                                        ctx.module.const_(*shift_amount as i64),
                                        BinaryOp::ShrUInt64,
                                    ),
                                    (Shift::ASR, SizeVariant::Reg32) => (
                                        ctx.module.const_(*shift_amount as i32),
                                        BinaryOp::ShrSInt32,
                                    ),
                                    (Shift::ASR, SizeVariant::Reg64) => (
                                        ctx.module.const_(*shift_amount as i64),
                                        BinaryOp::ShrSInt64,
                                    ),
                                    (Shift::ROR, SizeVariant::Reg32) => (
                                        ctx.module.const_(*shift_amount as i32),
                                        BinaryOp::RotRInt32,
                                    ),
                                    (Shift::ROR, SizeVariant::Reg64) => (
                                        ctx.module.const_(*shift_amount as i64),
                                        BinaryOp::RotRInt64,
                                    ),
                                };

                                ctx.module.binary(register_expr, amount_expr, shift_op)
                            }
                        }
                        AddSubtractRightOperand::ExtendedRegister {
                            extension,
                            left_shift_amount,
                            register,
                        } => {
                            let reg64 = ctx.read_register(registers::id(*register), Width::W64);

                            // Apply zero/sign extension to i64
                            let extended = match (extension.size, extension.signed) {
                                (SliceSize::Byte, false) => ctx.module.binary(
                                    reg64,
                                    ctx.module.const_(0xFFi64),
                                    BinaryOp::AndInt64,
                                ),
                                (SliceSize::Byte, true) => {
                                    let masked = ctx.module.binary(
                                        reg64,
                                        ctx.module.const_(0xFFi64),
                                        BinaryOp::AndInt64,
                                    );
                                    let shl = ctx.module.binary(
                                        masked,
                                        ctx.module.const_(56i64),
                                        BinaryOp::ShlInt64,
                                    );
                                    ctx.module.binary(
                                        shl,
                                        ctx.module.const_(56i64),
                                        BinaryOp::ShrSInt64,
                                    )
                                }
                                (SliceSize::Halfword, false) => ctx.module.binary(
                                    reg64,
                                    ctx.module.const_(0xFFFFi64),
                                    BinaryOp::AndInt64,
                                ),
                                (SliceSize::Halfword, true) => {
                                    let masked = ctx.module.binary(
                                        reg64,
                                        ctx.module.const_(0xFFFFi64),
                                        BinaryOp::AndInt64,
                                    );
                                    let shl = ctx.module.binary(
                                        masked,
                                        ctx.module.const_(48i64),
                                        BinaryOp::ShlInt64,
                                    );
                                    ctx.module.binary(
                                        shl,
                                        ctx.module.const_(48i64),
                                        BinaryOp::ShrSInt64,
                                    )
                                }
                                (SliceSize::Word, false) => ctx.module.binary(
                                    reg64,
                                    ctx.module.const_(0xFFFF_FFFFi64),
                                    BinaryOp::AndInt64,
                                ),
                                (SliceSize::Word, true) => {
                                    let wrapped = ctx.module.unary(reg64, UnaryOp::WrapInt64);
                                    ctx.module.unary(wrapped, UnaryOp::ExtendSInt32)
                                }
                                (SliceSize::Doubleword, _) => reg64,
                            };

                            // Apply left shift
                            let shifted = if *left_shift_amount == 0 {
                                extended
                            } else {
                                ctx.module.binary(
                                    extended,
                                    ctx.module.const_(*left_shift_amount as i64),
                                    BinaryOp::ShlInt64,
                                )
                            };

                            // Truncate to variant width
                            match variant {
                                SizeVariant::Reg32 => {
                                    ctx.module.unary(shifted, UnaryOp::WrapInt64)
                                }
                                SizeVariant::Reg64 => shifted,
                            }
                        }
                    }
                };

                // Arithmetic result in variant type (i32 or i64)
                let get_arith = || {
                    ctx.module.binary(
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
                    SizeVariant::Reg32 => ctx.module.unary(get_arith(), UnaryOp::ExtendUInt32),
                    SizeVariant::Reg64 => get_arith(),
                };
                block_exprs.push(ctx.write_register(
                    registers::id(*destination),
                    Width::W64,
                    write_value,
                ));

                if *set_flags {
                    // Sign bit helper: returns i32 value 0 or 1
                    let sign = |expr: Expression| match variant {
                        SizeVariant::Reg32 => {
                            ctx.module
                                .binary(expr, ctx.module.const_(0i32), BinaryOp::LtSInt32)
                        }
                        SizeVariant::Reg64 => {
                            ctx.module
                                .binary(expr, ctx.module.const_(0i64), BinaryOp::LtSInt64)
                        }
                    };

                    // N flag: result < 0 (signed)
                    block_exprs.push(ctx.write_register(Flag::Negative.id(), Width::W32, sign(get_arith())));

                    // Z flag: result == 0
                    let zero_expr = match variant {
                        SizeVariant::Reg32 => ctx.module.binary(
                            get_arith(),
                            ctx.module.const_(0i32),
                            BinaryOp::EqInt32,
                        ),
                        SizeVariant::Reg64 => ctx.module.binary(
                            get_arith(),
                            ctx.module.const_(0i64),
                            BinaryOp::EqInt64,
                        ),
                    };
                    block_exprs.push(ctx.write_register(Flag::Zero.id(), Width::W32, zero_expr));

                    // C flag
                    let carry_expr = match op {
                        // Add: unsigned overflow, i.e. result <u op1
                        AddSubtractOp::Add => match variant {
                            SizeVariant::Reg32 => {
                                ctx.module
                                    .binary(get_arith(), get_op1(), BinaryOp::LtUInt32)
                            }
                            SizeVariant::Reg64 => {
                                ctx.module
                                    .binary(get_arith(), get_op1(), BinaryOp::LtUInt64)
                            }
                        },
                        // Sub: no borrow, i.e. op1 >=u op2
                        AddSubtractOp::Subtract => match variant {
                            SizeVariant::Reg32 => {
                                ctx.module.binary(get_op1(), get_op2(), BinaryOp::GeUInt32)
                            }
                            SizeVariant::Reg64 => {
                                ctx.module.binary(get_op1(), get_op2(), BinaryOp::GeUInt64)
                            }
                        },
                    };
                    block_exprs.push(ctx.write_register(Flag::Carry.id(), Width::W32, carry_expr));

                    // V flag (signed overflow)
                    let overflow_expr = match op {
                        // Add: V = (op1_sign == op2_sign) AND (result_sign != op1_sign)
                        AddSubtractOp::Add => ctx.module.binary(
                            ctx.module.unary(
                                ctx.module.binary(
                                    sign(get_op1()),
                                    sign(get_op2()),
                                    BinaryOp::XorInt32,
                                ),
                                UnaryOp::EqZInt32,
                            ),
                            ctx.module.binary(
                                sign(get_op1()),
                                sign(get_arith()),
                                BinaryOp::XorInt32,
                            ),
                            BinaryOp::AndInt32,
                        ),
                        // Sub: V = (op1_sign != op2_sign) AND (result_sign != op1_sign)
                        AddSubtractOp::Subtract => ctx.module.binary(
                            ctx.module.binary(
                                sign(get_op1()),
                                sign(get_op2()),
                                BinaryOp::XorInt32,
                            ),
                            ctx.module.binary(
                                sign(get_op1()),
                                sign(get_arith()),
                                BinaryOp::XorInt32,
                            ),
                            BinaryOp::AndInt32,
                        ),
                    };

                    block_exprs.push(ctx.write_register(Flag::Overflow.id(), Width::W32, overflow_expr));
                }
            }

            Instruction::FormPCRelativeAddress {
                aligned_to_page,
                destination,
                value,
            } => {
                let address = if *aligned_to_page {
                    ((current_address & !0xfff) as i64 + *value) as u64
                } else {
                    ((current_address as i64) + *value) as u64
                };

                block_exprs.push(ctx.write_register(
                    registers::id(*destination),
                    Width::W64,
                    ctx.module.const_(address),
                ));
            }

            Instruction::LoadStoreRegister {
                address_base: address,
                address_index: offset,
                op,
                value,
            } => {
                let base_address_expr = ctx.read_register(registers::id(*address), Width::W64);

                let (address_expr, access_offset) = match offset {
                    LoadStoreIndex::Immediate { offset } => {
                        if offset.access < 0 {
                            (
                                ctx.module.binary(
                                    base_address_expr,
                                    ctx.module.const_(offset.access as i64),
                                    BinaryOp::AddInt64,
                                ),
                                0,
                            )
                        } else {
                            (base_address_expr, offset.access as u32)
                        }
                    }
                    LoadStoreIndex::Register {
                        mode,
                        register,
                        shift_amount,
                    } => {
                        let mut offset_expr = ctx.read_register(registers::id(*register), width(mode.variant()));

                        match *mode {
                            IndexMode::UnsignedSingle => {
                                offset_expr = ctx.module.unary(offset_expr, UnaryOp::ExtendUInt32);
                            }
                            IndexMode::SignedSingle => {
                                offset_expr = ctx.module.unary(offset_expr, UnaryOp::ExtendSInt32);
                            }
                            IndexMode::Double => {}
                        }

                        (
                            ctx.module.binary(
                                base_address_expr,
                                ctx.module.binary(
                                    offset_expr,
                                    ctx.module.const_(*shift_amount as i64),
                                    BinaryOp::ShlInt64,
                                ),
                                BinaryOp::AddInt64,
                            ),
                            0,
                        )
                    }
                };

                match op {
                    LoadStoreOp::Store(BivalentObject::GeneralPurpose(size)) => {
                        let value_expr = ctx.read_register(registers::id(*value), width(size.cover_variant()));
                        let store_variant = match size {
                            SliceSize::Byte => StoreVariant::I32L8,
                            SliceSize::Halfword => StoreVariant::I32L16,
                            SliceSize::Word => StoreVariant::I32,
                            SliceSize::Doubleword => StoreVariant::I64,
                        };

                        block_exprs.push(ctx.module.store(
                            store_variant,
                            value_expr,
                            address_expr,
                            access_offset,
                            1,
                            &ctx.global.memory_name,
                        ));
                    }
                    LoadStoreOp::Load(BivalentObject::GeneralPurpose(mode)) => {
                        let load_variant = match mode {
                            GPLoadMode::UnsignedByte => LoadVariant::I32L8 { signed: false },
                            GPLoadMode::SignedByteToSingle => LoadVariant::I32L8 { signed: true },
                            GPLoadMode::SignedByteToDouble => LoadVariant::I64L8 { signed: true },
                            GPLoadMode::UnsignedHalf => LoadVariant::I32L16 { signed: false },
                            GPLoadMode::SignedHalfToSingle => LoadVariant::I32L16 { signed: true },
                            GPLoadMode::SignedHalfToDouble => LoadVariant::I64L16 { signed: true },
                            GPLoadMode::UnsignedSingle => LoadVariant::I32,
                            GPLoadMode::SignedSingleToDouble => {
                                LoadVariant::I64L32 { signed: true }
                            }
                            GPLoadMode::Double => LoadVariant::I64,
                        };

                        let loaded_expr = ctx.module.load(
                            load_variant,
                            address_expr,
                            access_offset,
                            1,
                            &ctx.global.memory_name,
                        );

                        block_exprs.push(ctx.write_register(
                            registers::id(*value),
                            width(mode.register_size()),
                            loaded_expr,
                        ));
                    }
                    _ => todo!(),
                }

                if let LoadStoreIndex::Immediate {
                    offset:
                        WritebackOffset {
                            access: _,
                            writeback: Some(writeback),
                        },
                } = offset
                {
                    block_exprs.push(ctx.write_register(
                        registers::id(*address),
                        Width::W64,
                        ctx.module.binary(
                            ctx.read_register(registers::id(*address), Width::W64),
                            ctx.module.const_(*writeback as i64),
                            BinaryOp::AddInt64,
                        ),
                    ));
                }
            }

            Instruction::Logical {
                destination,
                op,
                operand1,
                operand2,
                variant,
            } => {
                let get_op1 = || ctx.read_register(registers::id(*operand1), width(*variant));
                let get_op2 = || match operand2 {
                    LogicalImmediateOperand::Immediate(op2) => match variant {
                        SizeVariant::Reg32 => ctx.module.const_(*op2 as i32),
                        SizeVariant::Reg64 => ctx.module.const_(*op2 as i64),
                    },
                    LogicalImmediateOperand::ShiftedRegister {
                        inverse,
                        register,
                        shift_amount,
                        shift_type,
                    } => {
                        let mut op2 = ctx.read_register(registers::id(*register), width(*variant));

                        if *shift_amount > 0 {
                            op2 = ctx.module.binary(
                                op2,
                                variant_unsigned_const(&ctx.module, *variant, *shift_amount),
                                resolve_shift_op(*shift_type, *variant),
                            )
                        };

                        if *inverse {
                            op2 = match variant {
                                SizeVariant::Reg32 => ctx.module.binary(
                                    op2,
                                    ctx.module.const_(-1i32),
                                    BinaryOp::XorInt32,
                                ),
                                SizeVariant::Reg64 => ctx.module.binary(
                                    op2,
                                    ctx.module.const_(-1i64),
                                    BinaryOp::XorInt64,
                                ),
                            }
                        }

                        op2
                    }
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

                let get_result = || ctx.module.binary(get_op1(), get_op2(), binary_op);

                block_exprs.push(ctx.write_register(registers::id(*destination), width(*variant), get_result()));

                if set_flags {
                    // N flag: result < 0 (signed)
                    let sign_expr = match variant {
                        SizeVariant::Reg32 => ctx.module.binary(
                            get_result(),
                            ctx.module.const_(0i32),
                            BinaryOp::LtSInt32,
                        ),
                        SizeVariant::Reg64 => ctx.module.binary(
                            get_result(),
                            ctx.module.const_(0i64),
                            BinaryOp::LtSInt64,
                        ),
                    };
                    block_exprs.push(ctx.write_register(Flag::Negative.id(), Width::W32, sign_expr));

                    // Z flag: result == 0
                    let zero_expr = match variant {
                        SizeVariant::Reg32 => ctx.module.binary(
                            get_result(),
                            ctx.module.const_(0i32),
                            BinaryOp::EqInt32,
                        ),
                        SizeVariant::Reg64 => ctx.module.binary(
                            get_result(),
                            ctx.module.const_(0i64),
                            BinaryOp::EqInt64,
                        ),
                    };
                    block_exprs.push(ctx.write_register(Flag::Zero.id(), Width::W32, zero_expr));

                    // C and V flags are always cleared by logical operations
                    block_exprs.push(ctx.write_register(Flag::Carry.id(), Width::W32, ctx.module.const_(0i32)));
                    block_exprs.push(ctx.write_register(Flag::Overflow.id(), Width::W32, ctx.module.const_(0i32)));
                }
            }

            Instruction::MoveWide {
                destination,
                keep_shift,
                source,
                value,
                variant,
            } => {
                let immediate_expr = match variant {
                    SizeVariant::Reg32 => ctx.module.const_(*value as u32),
                    SizeVariant::Reg64 => ctx.module.const_(*value),
                };

                let result_expr = if let Some(shift) = keep_shift {
                    let existing_expr = ctx.read_register(registers::id(*source), width(*variant));

                    let mask_expr = match variant {
                        SizeVariant::Reg32 => ctx.module.const_(!(0xffffu32 << shift)),
                        SizeVariant::Reg64 => ctx.module.const_(!(0xffffu64 << shift)),
                    };

                    ctx.module.binary(
                        immediate_expr,
                        ctx.module.binary(
                            existing_expr,
                            mask_expr,
                            match variant {
                                SizeVariant::Reg32 => BinaryOp::AndInt32,
                                SizeVariant::Reg64 => BinaryOp::AndInt64,
                            },
                        ),
                        match variant {
                            SizeVariant::Reg32 => BinaryOp::OrInt32,
                            SizeVariant::Reg64 => BinaryOp::OrInt64,
                        },
                    )
                } else {
                    immediate_expr
                };

                block_exprs.push(ctx.write_register(registers::id(*destination), width(*variant), result_expr));
            }

            Instruction::UnconditionalBranch {
                link: true,
                target: BranchTarget::RelativeInstructionOffset(target_offset),
            } => {
                let target_address =
                    ((current_address as i64) + *target_offset * (INSTRUCTION_SIZE as i64)) as u64;

                let target_routine_index = ctx
                    .global
                    .analysis
                    .routines
                    .iter()
                    .position(|r| r.address == target_address);

                // TODO: Better handle this kind of BL
                let target_routine_index = match target_routine_index {
                    Some(index) => index,
                    None => {
                        block_exprs.push(ctx.module.unreachable());
                        return;
                    }
                };

                let target_function_name = &ctx.global.function_names[target_routine_index];

                let return_registers = ctx.return_registers.clone();

                let arg_exprs = return_registers
                    .iter()
                    .map(|reg| ctx.read_register(*reg, Width::W64))
                    .collect::<Vec<_>>();

                let return_type = ctx.module.tuple_type(
                    &(return_registers
                        .iter()
                        .map(|_| ctx.module.i64())
                        .collect::<Vec<_>>()),
                );

                block_exprs.push(
                    ctx.module.local_set(
                        ctx.func_return_scratch_local_index,
                        ctx.module
                            .call(target_function_name, &arg_exprs, return_type.clone()),
                    ),
                );

                for (return_index, return_register) in return_registers.iter().enumerate() {
                    block_exprs.push(ctx.write_register(
                        *return_register,
                        Width::W64,
                        ctx.module.tuple_extract(
                            ctx.module.local_get(
                                ctx.func_return_scratch_local_index,
                                return_type.clone(),
                            ),
                            return_index as u32,
                        ),
                    ));
                }
            }

            Instruction::SupervisorCall { argument } => {
                let arg_exprs = std::iter::once(ctx.module.const_(*argument as u32))
                    .chain(
                        registers::SVC_PARAM_REGISTERS
                            .iter()
                            .map(|reg| ctx.read_register(registers::id(*reg), Width::W64)),
                    )
                    .collect::<Vec<_>>();

                block_exprs.push(ctx.module.local_set(
                    ctx.svc_return_scratch_local_index,
                    ctx.module.call(
                        &ctx.global.svc_function_name,
                        &arg_exprs,
                        ctx.global.svc_return_type.clone(),
                    ),
                ));

                for (return_index, return_register) in registers::SVC_RETURN_REGISTERS.iter().enumerate() {
                    block_exprs.push(ctx.write_register(
                        registers::id(*return_register),
                        Width::W64,
                        ctx.module.tuple_extract(
                            ctx.module.local_get(
                                ctx.svc_return_scratch_local_index,
                                ctx.global.svc_return_type.clone(),
                            ),
                            return_index as u32,
                        ),
                    ));
                }
            }

            Instruction::Nop => {
                block_exprs.push(ctx.module.nop());
            }
            Instruction::Return { target } => {
                assert!(*target == Register::X30);

                let return_exprs = ctx
                    .return_registers
                    .iter()
                    .map(|reg| ctx.read_register(*reg, Width::W64))
                    .collect::<Vec<_>>();

                block_exprs.push(ctx.module.return_(ctx.module.tuple(&return_exprs)));
            }

            Instruction::UnconditionalBranch { .. }
            | Instruction::BranchConditionally { .. }
            | Instruction::CompareAndBranch { .. } => {
                // Branches are handled by the relooper
            }

            _ => {
                eprintln!(
                    "Warning: Unhandled instruction at address {:#x}: {:?}",
                    current_address, self.0
                );
                block_exprs.push(ctx.module.nop());
            }
        }
    }

    fn branch_condition(&self, ctx: &RoutineContext) -> Option<Expression> {
        let module = &ctx.module;

        match &self.0 {
            Instruction::UnconditionalBranch { .. } => None,
            Instruction::BranchConditionally { condition, .. } => Some(match condition {
                Condition::EQ => ctx.read_register(Flag::Zero.id(), Width::W32),
                Condition::NE => {
                    module.unary(ctx.read_register(Flag::Zero.id(), Width::W32), UnaryOp::EqZInt32)
                }

                // N != V
                Condition::LT => module.binary(
                    ctx.read_register(Flag::Negative.id(), Width::W32),
                    ctx.read_register(Flag::Overflow.id(), Width::W32),
                    BinaryOp::NeInt32,
                ),

                // !Z && (N == V)
                Condition::GT => module.binary(
                    module.unary(ctx.read_register(Flag::Negative.id(), Width::W32), UnaryOp::EqZInt32),
                    module.binary(
                        ctx.read_register(Flag::Zero.id(), Width::W32),
                        ctx.read_register(Flag::Overflow.id(), Width::W32),
                        BinaryOp::EqInt32,
                    ),
                    BinaryOp::AndInt32,
                ),
                _ => {
                    eprintln!("Unsupported condition: {:?}", condition);
                    module.const_(1u32)
                }
            }),
            Instruction::CompareAndBranch {
                branch_if_zero,
                register,
                variant,
                ..
            } => Some(module.binary(
                ctx.read_register(registers::id(*register), width(*variant)),
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
            Instruction::TestBitAndBranch {
                branch_if_zero,
                register,
                test_bit,
                variant,
                ..
            } => Some(module.binary(
                module.binary(
                    ctx.read_register(registers::id(*register), width(*variant)),
                    match variant {
                        SizeVariant::Reg32 => module.const_(*test_bit),
                        SizeVariant::Reg64 => module.const_(*test_bit as u64),
                    },
                    match variant {
                        SizeVariant::Reg32 => BinaryOp::ShrUInt32,
                        SizeVariant::Reg64 => BinaryOp::ShrUInt64,
                    },
                ),
                match variant {
                    SizeVariant::Reg32 => module.const_(1i32),
                    SizeVariant::Reg64 => module.const_(1i64),
                },
                match (branch_if_zero, variant) {
                    (false, SizeVariant::Reg32) => BinaryOp::NeInt32,
                    (false, SizeVariant::Reg64) => BinaryOp::NeInt64,
                    (true, SizeVariant::Reg32) => BinaryOp::EqInt32,
                    (true, SizeVariant::Reg64) => BinaryOp::EqInt64,
                },
            )),
            _ => {
                panic!("Unsupported last instruction: {:?}", self.0);
            }
        }
    }

    fn branch_kind(&self) -> BranchKind {
        match &self.0 {
            Instruction::UnconditionalBranch {
                link: false,
                target: BranchTarget::RelativeInstructionOffset(target_offset),
            } => BranchKind::Jump {
                conditional: false,
                target_offset: *target_offset,
            },
            Instruction::BranchConditionally { target, .. }
            | Instruction::CompareAndBranch { target, .. }
            | Instruction::TestBitAndBranch { target, .. } => BranchKind::Jump {
                conditional: true,
                target_offset: *target,
            },
            Instruction::UnconditionalBranch { link: true, .. } => BranchKind::Call,
            Instruction::Return { .. } => BranchKind::Return,
            _ => BranchKind::None,
        }
    }

    fn stack_frame_effect(&self, stack_pointer: u32) -> (Option<u64>, Vec<StackAccess>) {
        match &self.0 {
            Instruction::AddSubtract {
                destination,
                op: AddSubtractOp::Subtract,
                operand1,
                operand2: AddSubtractRightOperand::Immediate(offset),
                set_flags: false,
                variant: SizeVariant::Reg64,
            } if registers::id(*destination) == stack_pointer
                && registers::id(*operand1) == stack_pointer =>
            {
                (Some(*offset as u64), Vec::new())
            }

            Instruction::LoadStoreRegister {
                address_base,
                address_index: LoadStoreIndex::Immediate { offset },
                op,
                ..
            } if registers::id(*address_base) == stack_pointer => {
                let allocate = offset
                    .writeback
                    .filter(|&writeback| writeback < 0)
                    .map(|writeback| -writeback as u64);

                let write = matches!(op, LoadStoreOp::Store(_));

                (
                    allocate,
                    vec![StackAccess {
                        offset: offset.access,
                        size_bytes: op.access_size().byte_count() as u32,
                        read: !write,
                        write,
                    }],
                )
            }

            Instruction::LoadStorePairOfRegisters {
                address_base,
                address_index: offset,
                op,
                variant,
                ..
            } if registers::id(*address_base) == stack_pointer => {
                let allocate = offset
                    .writeback
                    .filter(|&writeback| writeback < 0)
                    .map(|writeback| -writeback as u64);

                let write = matches!(op, LoadStoreOp::Store(_));
                let size_bytes = op.access_size().byte_count() as u32;

                (
                    allocate,
                    vec![
                        StackAccess {
                            offset: offset.access,
                            size_bytes,
                            read: !write,
                            write,
                        },
                        StackAccess {
                            offset: offset.access + (variant.any_size().byte_count() as i32),
                            size_bytes,
                            read: !write,
                            write,
                        },
                    ],
                )
            }

            _ => (None, Vec::new()),
        }
    }

    fn registers_read(&self) -> Vec<u32> {
        InstructionInfo::registers_read(&self.0)
            .into_iter()
            .map(|sr| registers::id(sr.register))
            .collect()
    }

    fn registers_written(&self) -> Vec<u32> {
        InstructionInfo::registers_written(&self.0)
            .into_iter()
            .map(|sr| registers::id(sr.register))
            .collect()
    }
}

fn variant_unsigned_const(module: &Module, variant: SizeVariant, value: u64) -> Expression {
    match variant {
        SizeVariant::Reg32 => module.const_(value as u32),
        SizeVariant::Reg64 => module.const_(value),
    }
}

fn resolve_shift_op(shift_type: Shift, variant: SizeVariant) -> BinaryOp {
    match (shift_type, variant) {
        (Shift::LSL, SizeVariant::Reg32) => BinaryOp::ShlInt32,
        (Shift::LSL, SizeVariant::Reg64) => BinaryOp::ShlInt64,
        (Shift::LSR, SizeVariant::Reg32) => BinaryOp::ShrUInt32,
        (Shift::LSR, SizeVariant::Reg64) => BinaryOp::ShrUInt64,
        (Shift::ASR, SizeVariant::Reg32) => BinaryOp::ShrSInt32,
        (Shift::ASR, SizeVariant::Reg64) => BinaryOp::ShrSInt64,
        (Shift::ROR, SizeVariant::Reg32) => BinaryOp::RotRInt32,
        (Shift::ROR, SizeVariant::Reg64) => BinaryOp::RotRInt64,
    }
}
