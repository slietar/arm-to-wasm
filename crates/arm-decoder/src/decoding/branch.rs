use crate::{
    instructions::{BranchTarget, Instruction, LogicalOp},
    structures::{Condition, InstructionBytes, Shift, SizeVariant},
    utilities::equal_masked,
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // B, BL
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Branches--Exception-Generating-and-System-instructions?lang=en#branch_imm
    if equal_masked(
        bytes.0,
        0b0111_1100_0000_0000_0000_0000_0000_0000,
        0b0001_0100_0000_0000_0000_0000_0000_0000,
    ) {
        return Some(Instruction::UnconditionalBranch {
            link: bytes.bool(31),
            target: BranchTarget::RelativeInstructionOffset(bytes.immediate(0, 26, true) as i64),
        });
    }

    // BR, BLR
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Branches--Exception-Generating-and-System-instructions?lang=en#branch_reg
    if equal_masked(
        bytes.0,
        0b1111_1111_1101_1111_1111_1100_0001_1111,
        0b1101_0110_0001_1111_0000_0000_0000_0000,
    ) {
        return Some(Instruction::UnconditionalBranch {
            link: bytes.bool(21),
            target: BranchTarget::Register(bytes.register(5, false)),
        });
    }

    // B.cond
    // Branch conditionally
    if equal_masked(
        bytes.0,
        0b1111_1111_0000_0000_0000_0000_0001_0000,
        0b0101_0100_0000_0000_0000_0000_0000_0000,
    ) {
        return Some(Instruction::BranchConditionally {
            condition: Condition::decode(bytes.immediate_unsigned(0, 4)),
            target: bytes.immediate(5, 19, true) as i64,
        });
    }

    // TBZ, TBNZ
    if equal_masked(
        bytes.0,
        0b0111_1110_0000_0000_0000_0000_0001_0000,
        0b0011_0110_0000_0000_0000_0000_0000_0000,
    ) {
        let variant = bytes.variant();
        let test_bit = bytes.immediate_unsigned(19, 5);

        if matches!(variant, SizeVariant::Reg32) && test_bit > 31 {
            panic!();
        }

        return Some(Instruction::TestBitAndBranch {
            branch_if_zero: !bytes.bool(24),
            register: bytes.register(0, false),
            target: bytes.immediate(5, 14, true) as i64,
            test_bit,
            variant,
        });
    }

    None
}
