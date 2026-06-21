use crate::{
    instructions::{BitfieldMoveMode, Instruction, LogicalOp},
    structures::{InstructionBytes, Shift, SizeVariant},
    utilities::equal_masked,
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    if equal_masked(
        bytes.0,
        0b0001_1111_1000_0000_0000_0000_0000_0000,
        0b0001_0011_0000_0000_0000_0000_0000_0000,
    ) {
        if bytes.bool(31) != bytes.bool(22) {
            panic!();
        }

        return Some(Instruction::BitfieldMove {
            destination: bytes.register(0, true),
            source: bytes.register(5, false),
            leftmost_bit_number: bytes.immediate_unsigned(16, 6),
            right_rotate_amount: bytes.immediate_unsigned(10, 6),
            variant: bytes.variant(),

            mode: match bytes.immediate_unsigned(29, 2) {
                0b00 => BitfieldMoveMode::Signed,
                0b01 => BitfieldMoveMode::Default,
                0b10 => BitfieldMoveMode::Unsigned,
                _ => panic!(),
            },
        });
    }

    None
}
