use crate::instructions::{Address, AddressingMode, Instruction, Register};

pub trait InstructionInfo {
    fn registers_read(&self) -> Vec<Register>;
    fn registers_written(&self) -> Vec<Register>;
}

impl InstructionInfo for Instruction {
    fn registers_read(&self) -> Vec<Register> {
        use Instruction::*;

        match self {
            AddImmediate { source, .. } => vec![*source],
            BitwiseOrShiftedRegister {
                operand1, operand2, ..
            } => vec![*operand1, *operand2],
            Branch { .. } => vec![],
            BranchConditionally { .. } => vec![],
            BranchWithLink { .. } => vec![],
            FormPCRelativeAddress { .. } => vec![],
            LoadRegisterImmediate { address, .. } => vec![address.base],
            MoveWideWithZero { .. } => vec![],
            Nop => vec![],
            Return { target } => vec![*target],
            StoreRegisterImmediate { address, value, .. } => vec![address.base, *value],
            StoreRegisterHalfwordImmediate { address, value } => vec![address.base, *value],
            StorePairOfRegisters {
                address,
                value1,
                value2,
                ..
            } => vec![address.base, *value1, *value2],
            StoreRegisterRegister {
                base_address,
                offset,
                value,
                ..
            } => vec![*base_address, *offset, *value],
            SubImmediate { source, .. } => vec![*source],
            SubShiftedRegister {
                operand1, operand2, ..
            } => vec![*operand1, *operand2],
            SupervisorCall { .. } => vec![],
            TestBitAndBranchIfNonzero { value, .. } => vec![*value],
            TestBitAndBranchIfZero { value, .. } => vec![*value],
            Unknown => vec![],
        }
    }

    fn registers_written(&self) -> Vec<Register> {
        use Instruction::*;

        match self {
            AddImmediate { destination, .. } => vec![*destination],
            BitwiseOrShiftedRegister { destination, .. } => vec![*destination],
            Branch { .. } => vec![],
            BranchConditionally { .. } => vec![],
            BranchWithLink { .. } => vec![],
            FormPCRelativeAddress { .. } => vec![],
            LoadRegisterImmediate { destination, .. } => vec![*destination],
            MoveWideWithZero { destination, .. } => vec![*destination],
            Nop => vec![],
            Return { .. } => vec![],
            StoreRegisterImmediate {
                address:
                    Address {
                        base,
                        mode:
                            AddressingMode::PreIndexWithWriteback { .. }
                            | AddressingMode::PostIndexWithWriteback { .. },
                    },
                ..
            } => vec![*base],
            // TODO: Add other writeback cases
            StoreRegisterImmediate { .. } => vec![],
            StoreRegisterHalfwordImmediate { .. } => vec![],
            StorePairOfRegisters { .. } => vec![],
            StoreRegisterRegister { .. } => vec![],
            SubImmediate { destination, .. } => vec![*destination],
            SubShiftedRegister { destination, .. } => vec![*destination],
            SupervisorCall { .. } => vec![],
            TestBitAndBranchIfNonzero { .. } => vec![],
            TestBitAndBranchIfZero { .. } => vec![],
            Unknown => vec![],
        }
    }
}
