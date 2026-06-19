use crate::{
    instructions::{AddSubtractOp, AddSubtractRightOperand, Instruction},
    structures::{Extension, InstructionBytes, Shift, SizeVariant, SliceSize},
    utilities::equal_masked,
};

fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // Immediate
    if equal_masked(
        bytes.0,
        0b0001_1111_1000_0000_0000_0000_0000_0000,
        0b0001_0001_0000_0000_0000_0000_0000_0000,
    ) {
        return Some(Instruction::AddSubtract {
            destination: bytes.register(0, true),
            op: AddSubtractOp::from_bool(bytes.bool(30)),
            operand1: bytes.register(0, true),
            operand2: AddSubtractRightOperand::Immediate(
                (bytes.immediate_unsigned(10, 12) as u64) << (if bytes.bool(22) { 12 } else { 0 }),
            ),
            set_flags: bytes.bool(29),
            variant: bytes.variant(),
        });
    }

    // Shifted register
    if equal_masked(
        bytes.0,
        0b0001_1111_0010_0000_0000_0000_0000_0000,
        0b0000_1011_0000_0000_0000_0000_0000_0000,
    ) {
        let variant = bytes.variant();
        let shift_amount = bytes.immediate_unsigned(10, 6) as u64;

        if matches!(variant, SizeVariant::Reg32) && shift_amount > 31 {
            unreachable!();
        }

        return Some(Instruction::AddSubtract {
            destination: bytes.register(0, false),
            op: AddSubtractOp::from_bool(bytes.bool(30)),
            operand1: bytes.register(5, false),
            operand2: AddSubtractRightOperand::ShiftedRegister {
                register: bytes.register(16, false),
                shift_amount,
                shift_type: Shift::decode(bytes.immediate_unsigned(22, 2), false),
            },
            set_flags: bytes.bool(29),
            variant: bytes.variant(),
        });
    }

    // Extended register
    if equal_masked(
        bytes.0,
        0b0001_1111_0010_0000_0000_0000_0000_0000,
        0b0000_1011_0010_0000_0000_0000_0000_0000,
    ) {
        let left_shift_amount = bytes.immediate_unsigned(10, 3) as u64;

        if left_shift_amount > 4 {
            unreachable!();
        }

        return Some(Instruction::AddSubtract {
            destination: bytes.register(0, true),
            op: AddSubtractOp::from_bool(bytes.bool(30)),
            operand1: bytes.register(5, true),
            operand2: AddSubtractRightOperand::ExtendedRegister {
                extension: Extension {
                    size: match bytes.immediate_unsigned(13, 2) {
                        0b00 => SliceSize::Byte,
                        0b01 => SliceSize::Halfword,
                        0b10 => SliceSize::Word,
                        0b11 => SliceSize::Doubleword,
                        _ => unreachable!(),
                    },
                    signed: bytes.bool(15),
                },
                left_shift_amount,
                register: bytes.register(16, true),
            },
            set_flags: bytes.bool(29),
            variant: bytes.variant(),
        });
    }

    None
}
