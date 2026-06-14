use crate::instructions::{
    Address, AddressingMode, Instruction, Register, SizeVariant, SizedRegister,
};

pub trait InstructionInfo {
    fn registers_read(&self) -> Vec<SizedRegister>;
    fn registers_written(&self) -> Vec<SizedRegister>;
}

impl InstructionInfo for Instruction {
    fn registers_read(&self) -> Vec<SizedRegister> {
        use Instruction::*;

        match self {
            AddImmediate {
                source, variant, ..
            } => vec![SizedRegister {
                register: *source,
                variant: *variant,
            }],
            BitwiseOrShiftedRegister {
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
            Branch { .. } => Vec::new(),
            BranchConditionally { .. } => Vec::new(),
            BranchWithLink { .. } => Vec::new(),
            FormPCRelativeAddress { .. } => Vec::new(),
            FormPCRelativeAddressToPage { destination, value } => Vec::new(),
            LoadRegisterImmediate {
                address, variant, ..
            } => vec![SizedRegister {
                register: address.base,
                variant: SizeVariant::Reg64,
            }],
            MoveWideWithZero { .. } => Vec::new(),
            Nop => Vec::new(),
            Return { target } => vec![SizedRegister {
                register: *target,
                variant: SizeVariant::Reg64,
            }],
            StoreRegisterImmediate {
                address,
                value,
                variant,
                ..
            } => vec![
                SizedRegister {
                    register: address.base,
                    variant: SizeVariant::Reg64,
                },
                SizedRegister {
                    register: *value,
                    variant: *variant,
                },
            ],
            StoreRegisterHalfwordImmediate { address, value } => vec![
                SizedRegister {
                    register: address.base,
                    variant: SizeVariant::Reg64,
                },
                SizedRegister {
                    register: *value,
                    variant: SizeVariant::Reg64, // TODO: Set to halfword
                },
            ],
            StorePairOfRegisters {
                address,
                value1,
                value2,
                ..
            } => vec![
                SizedRegister {
                    register: address.base,
                    variant: SizeVariant::Reg64,
                },
                SizedRegister {
                    register: *value1,
                    variant: SizeVariant::Reg64,
                },
                SizedRegister {
                    register: *value2,
                    variant: SizeVariant::Reg64,
                },
            ],
            StoreRegisterRegister {
                base_address,
                offset,
                value,
                ..
            } => vec![
                SizedRegister {
                    register: *base_address,
                    variant: SizeVariant::Reg64,
                },
                SizedRegister {
                    register: *offset,
                    variant: SizeVariant::Reg64,
                },
                SizedRegister {
                    register: *value,
                    variant: SizeVariant::Reg64,
                },
            ],
            SubImmediate { source, .. } | SubsImmediate { source, .. } => vec![SizedRegister {
                register: *source,
                variant: SizeVariant::Reg64,
            }],
            SubShiftedRegister {
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
            SupervisorCall { .. } => Vec::new(),
            TestBitAndBranchIfNonzero { value, variant, .. } => vec![SizedRegister {
                register: *value,
                variant: *variant,
            }],
            TestBitAndBranchIfZero { value, variant, .. } => vec![SizedRegister {
                register: *value,
                variant: *variant,
            }],
            Unknown => Vec::new(),
        }
    }

    fn registers_written(&self) -> Vec<SizedRegister> {
        use Instruction::*;

        match self {
            AddImmediate {
                destination,
                variant,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *variant,
            }],
            BitwiseOrShiftedRegister {
                destination,
                variant,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *variant,
            }],
            Branch { .. } => Vec::new(),
            BranchConditionally { .. } => Vec::new(),
            BranchWithLink { .. } => Vec::new(),
            FormPCRelativeAddress { destination, .. } => vec![SizedRegister {
                register: *destination,
                variant: SizeVariant::Reg64,
            }],
            FormPCRelativeAddressToPage { destination, value } => vec![SizedRegister {
                register: *destination,
                variant: SizeVariant::Reg64,
            }],
            LoadRegisterImmediate {
                destination,
                variant,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *variant,
            }],
            MoveWideWithZero {
                destination,
                variant,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *variant,
            }],
            Nop => Vec::new(),
            Return { .. } => Vec::new(),
            StoreRegisterImmediate {
                address:
                    Address {
                        base,
                        mode:
                            AddressingMode::PreIndexWithWriteback { .. }
                            | AddressingMode::PostIndexWithWriteback { .. },
                    },
                ..
            } => vec![SizedRegister {
                register: *base,
                variant: SizeVariant::Reg64,
            }],
            // TODO: Add other writeback cases
            StoreRegisterImmediate { .. } => Vec::new(),
            StoreRegisterHalfwordImmediate { .. } => Vec::new(),
            StorePairOfRegisters { .. } => Vec::new(),
            StoreRegisterRegister { .. } => Vec::new(),
            SubImmediate {
                destination,
                variant,
                ..
            }
            | SubsImmediate {
                destination,
                variant,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *variant,
            }],
            SubShiftedRegister {
                destination,
                variant,
                ..
            } => vec![SizedRegister {
                register: *destination,
                variant: *variant,
            }],
            SupervisorCall { .. } => Vec::new(),
            TestBitAndBranchIfNonzero { .. } => Vec::new(),
            TestBitAndBranchIfZero { .. } => Vec::new(),
            Unknown => Vec::new(),
        }
    }
}
