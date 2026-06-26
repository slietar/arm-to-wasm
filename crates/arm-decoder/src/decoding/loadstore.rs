use crate::{
    instructions::{
        BivalentObject, GPLoadMode, IndexMode, Instruction, LoadLiteralGPMode, LoadStoreIndex,
        LoadStoreOp,
    },
    structures::{
        AnySize, Extension, InstructionBytes, LargeSize, RegisterMode, SizeVariant, Sized as _,
        SliceSize, WritebackOffset,
    },
    utilities::equal_masked,
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // VR = 1 => Floating point
    let is_fp = bytes.bool(26);

    // Covered instructions
    // 1 Register offset [base address + index register]
    // 2 Unscaled immediate [base address + signed immediate offset]
    // 3 Immediate
    //    [base address + (un)signed immediate offset * size of access]
    //    a Post-index
    //    b Pre-index
    //    c Unsigned offset/immediate

    // 3c
    let is_unsigned_immediate = equal_masked(
        bytes.0,
        0b0011_1011_0000_0000_0000_0000_0000_0000,
        0b0011_1001_0000_0000_0000_0000_0000_0000,
    );

    // 1
    let is_register_offset = equal_masked(
        bytes.0,
        0b0011_1011_0010_0000_0000_1100_0000_0000,
        0b0011_1000_0010_0000_0000_1000_0000_0000,
    );

    // 2 + 3a + 3b
    let is_signed_immediate = equal_masked(
        bytes.0,
        0b0011_1011_0010_0000_0000_0000_0000_0000,
        0b0011_1000_0000_0000_0000_0000_0000_0000,
    );

    if is_signed_immediate || is_register_offset || is_unsigned_immediate {
        let is_unscaled_immediate = is_signed_immediate && bytes.immediate_unsigned(10, 2) == 0b00;

        let op = if is_fp {
            match (
                bytes.immediate_unsigned(30, 2),
                bytes.immediate_unsigned(22, 2),
            ) {
                (0b00, 0b00) => LoadStoreOp::Store(BivalentObject::FloatingPoint(AnySize::Byte)),
                (0b00, 0b01) => LoadStoreOp::Load(BivalentObject::FloatingPoint(AnySize::Byte)),
                (0b00, 0b10) => LoadStoreOp::Store(BivalentObject::FloatingPoint(AnySize::Quad)),
                (0b00, 0b11) => LoadStoreOp::Load(BivalentObject::FloatingPoint(AnySize::Quad)),
                (0b10, 0b00) => LoadStoreOp::Store(BivalentObject::FloatingPoint(AnySize::Half)),
                (0b10, 0b01) => LoadStoreOp::Load(BivalentObject::FloatingPoint(AnySize::Half)),
                (0b11, 0b00) => LoadStoreOp::Store(BivalentObject::FloatingPoint(AnySize::Single)),
                (0b11, 0b01) => LoadStoreOp::Load(BivalentObject::FloatingPoint(AnySize::Single)),
                _ => return None,
            }
        } else {
            match (
                bytes.immediate_unsigned(30, 2),
                bytes.immediate_unsigned(22, 2),
            ) {
                (0b00, 0b00) => LoadStoreOp::Store(BivalentObject::GeneralPurpose(SliceSize::Byte)),
                (0b00, 0b01) => {
                    LoadStoreOp::Load(BivalentObject::GeneralPurpose(GPLoadMode::UnsignedByte))
                }
                (0b00, 0b10) => LoadStoreOp::Load(BivalentObject::GeneralPurpose(
                    GPLoadMode::SignedByteToDouble,
                )),
                (0b00, 0b11) => LoadStoreOp::Load(BivalentObject::GeneralPurpose(
                    GPLoadMode::SignedByteToSingle,
                )),

                (0b01, 0b00) => {
                    LoadStoreOp::Store(BivalentObject::GeneralPurpose(SliceSize::Halfword))
                }
                (0b01, 0b01) => {
                    LoadStoreOp::Load(BivalentObject::GeneralPurpose(GPLoadMode::UnsignedHalf))
                }
                (0b01, 0b10) => LoadStoreOp::Load(BivalentObject::GeneralPurpose(
                    GPLoadMode::SignedHalfToDouble,
                )),
                (0b01, 0b11) => LoadStoreOp::Load(BivalentObject::GeneralPurpose(
                    GPLoadMode::SignedHalfToSingle,
                )),

                (0b10, 0b00) => LoadStoreOp::Store(BivalentObject::GeneralPurpose(SliceSize::Word)),
                (0b10, 0b01) => {
                    LoadStoreOp::Load(BivalentObject::GeneralPurpose(GPLoadMode::UnsignedSingle))
                }
                (0b10, 0b10) => LoadStoreOp::Load(BivalentObject::GeneralPurpose(
                    GPLoadMode::SignedSingleToDouble,
                )),
                (0b10, 0b11) => return None,

                (0b11, 0b00) => {
                    LoadStoreOp::Store(BivalentObject::GeneralPurpose(SliceSize::Doubleword))
                }
                (0b11, 0b01) => {
                    LoadStoreOp::Store(BivalentObject::GeneralPurpose(SliceSize::Doubleword))
                }
                (0b11, 0b10) => return None,
                (0b11, 0b11) => return None,
                _ => unreachable!(),
            }
        };

        if is_register_offset {
            return Some(Instruction::LoadStoreRegister {
                address_base: bytes.register(5, true),
                address_index: LoadStoreIndex::Register {
                    register: bytes.register(16, false),
                    mode: match bytes.immediate_unsigned(13, 3) {
                        0b010 => IndexMode::UnsignedSingle,
                        0b011 => IndexMode::Double,
                        0b110 => IndexMode::SignedSingle,
                        0b111 => IndexMode::Double,
                        _ => panic!(),
                    },
                    shift_amount: if bytes.bool(12) {
                        op.access_size().log_byte_count()
                    } else {
                        0
                    },
                },
                op,
                value: bytes.register_any(
                    0,
                    if is_fp {
                        RegisterMode::SIMD
                    } else {
                        RegisterMode::SP
                    },
                ),
            });
        } else {
            let offset = if is_unscaled_immediate {
                bytes.immediate(12, 9, true)
            } else if is_unsigned_immediate {
                (bytes.immediate_unsigned(10, 12) << op.access_size().log_byte_count()) as i32
            } else {
                bytes.immediate(12, 9, true) << op.access_size().log_byte_count()
            };

            return Some(Instruction::LoadStoreRegister {
                address_base: bytes.register(5, true),
                address_index: LoadStoreIndex::Immediate {
                    offset: match bytes.immediate_unsigned(10, 2) {
                        // Unsigned offset
                        _ if is_unsigned_immediate => WritebackOffset {
                            access: offset,
                            writeback: None,
                        },

                        // Unscaled immediate
                        _ if is_unscaled_immediate => WritebackOffset {
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
                        _ => panic!(),
                    },
                },
                op,
                value: bytes.register_any(
                    0,
                    if is_fp {
                        RegisterMode::SIMD
                    } else {
                        RegisterMode::GP
                    },
                ),
            });
        }
    }

    // Load literal
    if equal_masked(
        bytes.0,
        0b0011_1111_0000_0000_0000_0000_0000_0000,
        0b0001_1000_0000_0000_0000_0000_0000_0000,
    ) {
        let mode = match (bytes.immediate_unsigned(30, 2), is_fp) {
            (0b00, false) => BivalentObject::GeneralPurpose(LoadLiteralGPMode::UnsignedSingle),
            (0b01, false) => BivalentObject::GeneralPurpose(LoadLiteralGPMode::Double),
            (0b10, false) => {
                BivalentObject::GeneralPurpose(LoadLiteralGPMode::SignedSingleToDouble)
            }
            (0b11, false) => return Some(Instruction::PrefetchMemory),
            (0b00, true) => BivalentObject::FloatingPoint(LargeSize::Single),
            (0b01, true) => BivalentObject::FloatingPoint(LargeSize::Double),
            (0b10, true) => BivalentObject::FloatingPoint(LargeSize::Quad),
            (0b11, true) => return None,
            _ => unreachable!(),
        };

        return Some(Instruction::LoadLiteral {
            destination: bytes.register(0, false),
            relative_instruction_offset: bytes.immediate(5, 19, true) as i64,
            op: mode,
        });
    }

    None
}
