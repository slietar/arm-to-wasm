use crate::{
    instructions::Instruction,
    structures::InstructionBytes,
    utilities::{equal_masked, sign_extend},
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // BRK
    // Breakpoint
    // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/BRK--Breakpoint-instruction-?lang=en
    if equal_masked(
        bytes.0,
        0b1111_1111_1110_0000_0000_0000_0001_1111,
        0b1101_0100_0010_0000_0000_0000_0000_0000,
    ) {
        return Some(Instruction::Breakpoint {
            immediate: bytes.immediate_unsigned(5, 16) as u16,
        });
    }

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

    // ADR, ADRP
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Immediate?lang=en#pcreladdr
    if equal_masked(
        bytes.0,
        0b1001_1111_0000_0000_0000_0000_0000_0000,
        0b0001_0000_0000_0000_0000_0000_0000_0000,
    ) {
        return Some(Instruction::FormPCRelativeAddress {
            aligned_to_page: bytes.bool(31),
            destination: bytes.register(0, false),
            value: sign_extend(
                (bytes.immediate_unsigned(5, 19) << 2) | bytes.immediate_unsigned(29, 2),
                21,
            ) as i64,
        });
    }

    None
}
