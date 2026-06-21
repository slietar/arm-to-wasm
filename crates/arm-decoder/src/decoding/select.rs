use crate::{
    instructions::{BranchTarget, ConditionalSelectMode, Instruction, LogicalOp},
    structures::{Condition, InstructionBytes, Shift, SizeVariant},
    utilities::equal_masked,
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // Enforcing S = 0 and op2 = 0x
    if equal_masked(
        bytes.0,
        0b0011_1111_1110_0000_0000_1000_0000_0000,
        0b0001_1010_1000_0000_0000_0000_0000_0000,
    ) {
        return Some(Instruction::ConditionalSelect {
            condition: Condition::decode(bytes.immediate_unsigned(12, 4) ^ 0b0001, false),
            destination: bytes.register(0, false),
            op: match (bytes.bool(30), bytes.bool(10)) {
                (false, false) => ConditionalSelectMode::Identity,
                (false, true) => ConditionalSelectMode::Increment,
                (true, false) => ConditionalSelectMode::Invert,
                (true, true) => ConditionalSelectMode::Negate,
                _ => unreachable!(),
            },
            operand1: bytes.register(5, false),
            operand2: bytes.register(16, false),
            variant: bytes.variant(),
        });
    }

    None
}
