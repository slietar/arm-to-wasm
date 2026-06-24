use arm_decoder::{
    instructions::{
        AddSubtractRightOperand, BranchTarget, Instruction, LoadStoreOffset, LoadStoreOp,
    },
    structures::{Register, SizeVariant, SliceSize},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizedRegister {
    pub register: Register,
    pub variant: SizeVariant,
}

pub trait InstructionInfo {
    fn registers_read(&self) -> Vec<SizedRegister>;
    fn registers_written(&self) -> Vec<SizedRegister>;
}

fn variant_for_slice_size(size: SliceSize) -> SizeVariant {
    match size {
        SliceSize::Doubleword => SizeVariant::Reg64,
        SliceSize::Byte | SliceSize::Halfword | SliceSize::Word => SizeVariant::Reg32,
    }
}

impl InstructionInfo for Instruction {
    fn registers_read(&self) -> Vec<SizedRegister> {
        use Instruction::*;

        match self {
            AddSubtract {
                operand1,
                operand2,
                variant,
                ..
            } => {
                let mut regs = vec![SizedRegister {
                    register: *operand1,
                    variant: *variant,
                }];

                match operand2 {
                    AddSubtractRightOperand::Immediate(_) => {}
                    AddSubtractRightOperand::ShiftedRegister { register, .. }
                    | AddSubtractRightOperand::ExtendedRegister { register, .. } => {
                        regs.push(SizedRegister {
                            register: *register,
                            variant: *variant,
                        });
                    }
                }

                regs
            }
            LogicalImmediate {
                operand1, variant, ..
            } => vec![SizedRegister {
                register: *operand1,
                variant: *variant,
            }],
            LogicalShiftedRegister {
                operand1,
                operand2,
                variant,
                ..
            } => vec![
                SizedRegister {
                    register: *operand1,
                    variant: *variant,
                },
                SizedRegister {
                    register: *operand2,
                    variant: *variant,
                },
            ],
            FormPCRelativeAddress { .. } => Vec::new(),
            LoadStoreRegister {
                address,
                offset,
                op,
                size,
                value,
            } => {
                let mut regs = vec![SizedRegister {
                    register: *address,
                    variant: SizeVariant::Reg64,
                }];

                if let LoadStoreOffset::Register { register, .. } = offset {
                    regs.push(SizedRegister {
                        register: *register,
                        variant: SizeVariant::Reg64,
                    });
                }

                if matches!(op, LoadStoreOp::Store) {
                    regs.push(SizedRegister {
                        register: *value,
                        variant: variant_for_slice_size(*size),
                    });
                }

                regs
            }
            LoadStorePairOfRegisters {
                address,
                op,
                value1,
                value2,
                variant,
                ..
            } => {
                let mut regs = vec![SizedRegister {
                    register: *address,
                    variant: SizeVariant::Reg64,
                }];

                if matches!(op, LoadStoreOp::Store) {
                    regs.push(SizedRegister {
                        register: *value1,
                        variant: *variant,
                    });
                    regs.push(SizedRegister {
                        register: *value2,
                        variant: *variant,
                    });
                }

                regs
            }
            MoveWide {
                source, variant, ..
            } => vec![SizedRegister {
                register: *source,
                variant: *variant,
            }],
            Nop => Vec::new(),
            Return { target } => vec![SizedRegister {
                register: *target,
                variant: SizeVariant::Reg64,
            }],
            UnconditionalBranch {
                target: BranchTarget::Register(register),
                ..
            } => vec![SizedRegister {
                register: *register,
                variant: SizeVariant::Reg64,
            }],
            BranchConditionally { .. } => Vec::new(),
            CompareAndBranch {
                register, variant, ..
            }
            | TestBitAndBranch {
                register, variant, ..
            } => vec![SizedRegister {
                register: *register,
                variant: *variant,
            }],
            ConditionalSelect {
                operand1,
                operand2,
                variant,
                ..
            } => vec![
                SizedRegister {
                    register: *operand1,
                    variant: *variant,
                },
                SizedRegister {
                    register: *operand2,
                    variant: *variant,
                },
            ],
            BitfieldMove {
                source, variant, ..
            } => vec![SizedRegister {
                register: *source,
                variant: *variant,
            }],
            ConvertFPInteger {
                operand,
                integer_size,
                ..
            }
            | FPMove {
                operand,
                integer_size,
                ..
            } => vec![SizedRegister {
                register: *operand,
                variant: *integer_size,
            }],
            FPProcessing {
                operand1, operand2, ..
            }
            | SIMDTriple {
                operand1, operand2, ..
            } => Vec::new(),
            Breakpoint { .. }
            | PrefetchMemory
            | PermanentlyUndefined { .. }
            | SupervisorCall { .. }
            | LoadLiteral { .. }
            | UnconditionalBranch { .. }
            | Unknown => Vec::new(),
        }
    }

    fn registers_written(&self) -> Vec<SizedRegister> {
        use Instruction::*;

        match self {
            AddSubtract {
                destination,
                variant,
                ..
            }
            | LogicalImmediate {
                destination,
                variant,
                ..
            }
            | LogicalShiftedRegister {
                destination,
                variant,
                ..
            }
            | ConditionalSelect {
                destination,
                variant,
                ..
            }
            | BitfieldMove {
                destination,
                variant,
                ..
            }
            | MoveWide {
                destination,
                variant,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *variant,
            }],
            FormPCRelativeAddress { destination, .. } => vec![SizedRegister {
                register: *destination,
                variant: SizeVariant::Reg64,
            }],
            LoadLiteral {
                destination, size, ..
            } => vec![SizedRegister {
                register: *destination,
                variant: variant_for_slice_size(*size),
            }],
            LoadStoreRegister {
                address,
                offset,
                op,
                size,
                value,
            } => {
                let mut regs = Vec::new();

                if let LoadStoreOffset::Immediate { offset } = offset
                    && offset.writeback.is_some()
                {
                    regs.push(SizedRegister {
                        register: *address,
                        variant: SizeVariant::Reg64,
                    });
                }

                if !matches!(op, LoadStoreOp::Store) {
                    regs.push(SizedRegister {
                        register: *value,
                        variant: variant_for_slice_size(*size),
                    });
                }

                regs
            }
            LoadStorePairOfRegisters {
                address,
                offset,
                op,
                value1,
                value2,
                variant,
                ..
            } => {
                let mut regs = Vec::new();

                if offset.writeback.is_some() {
                    regs.push(SizedRegister {
                        register: *address,
                        variant: SizeVariant::Reg64,
                    });
                }

                if !matches!(op, LoadStoreOp::Store) {
                    regs.push(SizedRegister {
                        register: *value1,
                        variant: *variant,
                    });
                    regs.push(SizedRegister {
                        register: *value2,
                        variant: *variant,
                    });
                }

                regs
            }
            ConvertFPInteger {
                destination,
                integer_size,
                ..
            }
            | FPMove {
                destination,
                integer_size,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *integer_size,
            }],
            FPProcessing { destination, .. } | SIMDTriple { destination, .. } => Vec::new(),
            Nop => Vec::new(),
            Return { .. }
            | SupervisorCall { .. }
            | Breakpoint { .. }
            | PrefetchMemory
            | PermanentlyUndefined { .. }
            | UnconditionalBranch { .. }
            | BranchConditionally { .. }
            | CompareAndBranch { .. }
            | TestBitAndBranch { .. }
            | Unknown => Vec::new(),
        }
    }
}
