use crate::{
    instructions::{Instruction, LoadStoreOffset, LoadStoreOp},
    structures::{Extension, InstructionBytes, SizeVariant, SliceSize, Transform, WritebackOffset},
    utilities::equal_masked,
};

fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // Not handling SIMD load/store so enforcing VR = 0 everywhere

    let get_size = || match bytes.immediate_unsigned(30, 2) {
        0b00 => SliceSize::Byte,
        0b01 => SliceSize::Halfword,
        0b10 => SliceSize::Word,
        0b11 => SliceSize::Doubleword,
        _ => unreachable!(),
    };

    let get_op = |size| match bytes.immediate_unsigned(22, 2) {
        0b00 => LoadStoreOp::Store,
        0b01 => LoadStoreOp::LoadZeroExtend,

        0b10 => {
            if matches!(size, SliceSize::Doubleword) {
                panic!();
            }

            LoadStoreOp::LoadSignExtend {
                variant: SizeVariant::Reg64,
            }
        }
        0b11 => {
            // TODO: Handle RPRFM and PRFM
            if matches!(size, SliceSize::Word | SliceSize::Doubleword) {
                panic!();
            }

            LoadStoreOp::LoadSignExtend {
                variant: SizeVariant::Reg32,
            }
        }
        _ => unreachable!(),
    };

    // Register offset
    if equal_masked(
        bytes.0,
        0b0011_1111_0010_0000_0000_1100_0000_0000,
        0b0011_1000_0010_0000_0000_1000_0000_0000,
    ) {
        let size = get_size();

        return Some(Instruction::LoadStoreRegister {
            address: bytes.register(5, true),
            offset: LoadStoreOffset::Register {
                register: bytes.register(16, false),
                extension: match bytes.immediate_unsigned(13, 3) {
                    0b010 => Some(Extension::UXTW),
                    0b011 => None,
                    0b110 => Some(Extension::SXTW),
                    0b111 => Some(Extension::SXTX),
                    _ => panic!(),
                },
                shift_amount: if bytes.bool(12) {
                    size.log_byte_count()
                } else {
                    0
                },
            },
            op: get_op(size),
            size,
            value: bytes.register(0, false),
        });
    }

    let is_unsigned_immediate = equal_masked(
        bytes.0,
        0b0011_1111_0000_0000_0000_0000_0000_0000,
        0b0011_1010_0000_0000_0000_0000_0000_0000,
    );

    // Immediate
    if equal_masked(
        bytes.0,
        0b0011_1111_0010_0000_0000_0000_0000_0000,
        0b0011_1000_0000_0000_0000_0000_0000_0000,
    ) || is_unsigned_immediate {
        let size = get_size();
        let offset = if is_unsigned_immediate {
            (bytes.immediate_unsigned(10, 12) << size.log_byte_count()) as i32
        } else {
            bytes.immediate(12, 9, true)
        };

        return Some(Instruction::LoadStoreRegister {
            address: bytes.register(5, true),
            offset: LoadStoreOffset::Immediate {
                offset: match bytes.immediate_unsigned(10, 2) {
                    // Unsigned immediate
                    _ if is_unsigned_immediate => WritebackOffset {
                        access: offset,
                        writeback: None,
                    },

                    // Unscaled immediate
                    0b00 => WritebackOffset {
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
            },
            op: get_op(size),
            size,
            value: bytes.register(0, false),
        });
    }

    // Load literal
    if equal_masked(
        bytes.0,
        0b0011_1111_0000_0000_0000_0000_0000_0000,
        0b0001_1000_0000_0000_0000_0000_0000_0000,
    ) {
        let (size, sign_extend) = match bytes.immediate_unsigned(30, 2) {
            0b00 => (SliceSize::Word, false),
            0b01 => (SliceSize::Doubleword, false), // Sign extension has no effect on 64-bit loads
            0b10 => (SliceSize::Word, true),
            0b11 => return Some(Instruction::PrefetchMemory),
            _ => unreachable!(),
        };

        return Some(Instruction::LoadLiteral {
            destination: bytes.register(0, false),
            relative_instruction_offset: bytes.immediate(5, 19, true) as i64,
            sign_extend,
            size,
        });
    }

    None
}
