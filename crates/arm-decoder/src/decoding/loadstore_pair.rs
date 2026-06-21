use crate::{
    instructions::{Instruction, LoadStoreOffset, LoadStoreOp},
    structures::{Extension, InstructionBytes, SizeVariant, SliceSize, Transform, WritebackOffset},
    utilities::{equal_masked, sign_extend},
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // Enforcing VR = 0

    if equal_masked(
        bytes.0,
        0b0011_1110_0000_0000_0000_0000_0000_0000,
        0b0010_1000_0000_0000_0000_0000_0000_0000,
    ) {
        let (op, size) = match (
            bytes.immediate_unsigned(30, 2),
            bytes.immediate_unsigned(22, 1),
        ) {
            (0b00, 0b0) => (LoadStoreOp::Store, SizeVariant::Reg32),
            (0b00, 0b1) => (LoadStoreOp::LoadZeroExtend, SizeVariant::Reg32),
            (0b01, 0b1) => (
                LoadStoreOp::LoadSignExtend {
                    variant: SizeVariant::Reg32,
                },
                SizeVariant::Reg32,
            ),
            (0b10, 0b0) => (LoadStoreOp::Store, SizeVariant::Reg64),
            (0b10, 0b1) => (LoadStoreOp::LoadZeroExtend, SizeVariant::Reg64),
            _ => unreachable!(),
        };

        let offset = bytes.immediate(15, 7, true) * (size.byte_count() as i32);

        return Some(Instruction::LoadStorePairOfRegisters {
            address: bytes.register(5, true),
            offset: match bytes.immediate_unsigned(23, 2) {
                0b00 => unimplemented!(),

                // Signed offset
                0b10 => WritebackOffset {
                    access: offset,
                    writeback: None,
                },

                // Post index with writeback
                0b01 => WritebackOffset {
                    access: 0,
                    writeback: Some(offset),
                },

                // Pre index with writeback
                0b11 => WritebackOffset {
                    access: offset,
                    writeback: Some(offset),
                },
                _ => unreachable!(),
            },
            op,
            size,
            value1: bytes.register(0, false),
            value2: bytes.register(10, false),
            variant: bytes.variant(),
        });
    }

    None
}
