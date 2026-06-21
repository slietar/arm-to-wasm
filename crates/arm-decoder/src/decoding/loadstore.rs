use crate::{
    instructions::{Instruction, LoadStoreOffset, LoadStoreOp},
    structures::{Extension, InstructionBytes, SizeVariant, SliceSize, Transform},
    utilities::equal_masked,
};

fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // Register offset
    // Not handling SIMD load/store so enforcing VR = 0
    if equal_masked(
        bytes.0,
        0b0011_1111_0010_0000_0000_1100_0000_0000,
        0b0011_1000_0010_0000_0000_1000_0000_0000,
    ) {
        let size = match bytes.immediate_unsigned(30, 2) {
            0b00 => SliceSize::Byte,
            0b01 => SliceSize::Halfword,
            0b10 => SliceSize::Word,
            0b11 => SliceSize::Doubleword,
            _ => unreachable!(),
        };

        let option = bytes.immediate_unsigned(13, 3);

        let op = match bytes.immediate_unsigned(22, 2) {
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
            op,
            size,
            value: bytes.register(0, false),
        });
    }

    None
}
