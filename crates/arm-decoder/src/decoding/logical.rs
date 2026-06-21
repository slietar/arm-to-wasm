use crate::{
    instructions::{Instruction, LogicalOp},
    structures::{InstructionBytes, Shift, SizeVariant},
    utilities::equal_masked,
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // Immediate
    if equal_masked(
        bytes.0,
        0b0001_1111_1000_0000_0000_0000_0000_0000,
        0b0001_0010_0000_0000_0000_0000_0000_0000,
    ) {
        let variant = bytes.variant();
        let is_pattern_double = bytes.bool(22);

        if matches!(variant, SizeVariant::Reg32) && is_pattern_double {
            unreachable!();
        }

        return Some(Instruction::LogicalImmediate {
            destination: bytes.register(0, true),
            op: LogicalOp::decode(bytes.immediate_unsigned(29, 2)),
            operand1: bytes.register(5, false),
            operand2: decode_bitmask(
                is_pattern_double,
                bytes.immediate_unsigned(10, 6),
                bytes.immediate_unsigned(16, 6),
                variant,
            ),
            variant,
        });
    }

    // Shifted register
    if equal_masked(
        bytes.0,
        0b0001_1111_0000_0000_0000_0000_0000_0000,
        0b0000_1010_0000_0000_0000_0000_0000_0000,
    ) {
        let shift_amount = bytes.immediate_unsigned(10, 6) as u64;
        let variant = bytes.variant();

        if matches!(variant, SizeVariant::Reg32) && shift_amount > 31 {
            unreachable!();
        }

        return Some(Instruction::LogicalShiftedRegister {
            destination: bytes.register(0, false),
            inverse_operand2: bytes.bool(21),
            op: LogicalOp::decode(bytes.immediate_unsigned(29, 2)),
            operand1: bytes.register(5, false),
            operand2: bytes.register(16, false),
            shift_type: Shift::decode(bytes.immediate_unsigned(22, 2), true),
            shift_amount: shift_amount,
            variant,
        });
    }

    None
}

pub fn decode_bitmask(n: bool, imms: u32, immr: u32, size: SizeVariant) -> u64 {
    let leading_one_count = (imms | 0b1111_1111_1111_1111_1111_1111_1100_0000).leading_ones() - 26;
    let pattern_length = (1u64 << (5 - leading_one_count));
    let one_count = (imms & ((1 << (5 - leading_one_count)) - 1)) + 1;

    // eprintln!("pattern_length: {pattern_length}, one_count: {one_count}");

    let pattern = (1u64 << one_count) - 1;
    let repeat_count = 64 / pattern_length;

    let mut result = 0;

    for index in 0..repeat_count {
        result |= (pattern << (index * pattern_length));
    }

    result = result.rotate_right(immr);

    result
}
