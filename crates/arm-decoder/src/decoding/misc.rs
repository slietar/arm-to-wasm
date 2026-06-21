use crate::{instructions::Instruction, structures::InstructionBytes, utilities::equal_masked};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // NOP
    // No operation
    // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/NOP--No-operation-?lang=en
    if equal_masked(
        bytes.0,
        0b1111_1111_1111_1111_1111_1111_1111_1111,
        0b1101_0101_0000_0011_0010_0000_0001_1111,
    ) {
        return Some(Instruction::Nop);
    }

    // SVC
    // Supervisor call
    // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/SVC--Supervisor-call-?lang=en
    if equal_masked(
        bytes.0,
        0b1111_1111_1110_0000_0000_0000_0001_1111,
        0b1101_0100_0000_0000_0000_0000_0000_0001,
    ) {
        return Some(Instruction::SupervisorCall {
            argument: bytes.immediate_unsigned(5, 16) as u16,
        });
    }

    // RET
    // Return from subroutine
    // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/RET--Return-from-subroutine-?lang=en
    if equal_masked(
        bytes.0,
        0b1111_1111_1111_1111_1111_1100_0001_1111,
        0b1101_0110_0101_1111_0000_0000_0000_0000,
    ) {
        return Some(Instruction::Return {
            target: bytes.register(5, false),
        });
    }


    None
}
