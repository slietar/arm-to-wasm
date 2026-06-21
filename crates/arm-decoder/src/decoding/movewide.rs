use crate::{
    instructions::Instruction,
    structures::{InstructionBytes, SizeVariant},
    utilities::equal_masked,
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    if equal_masked(
        bytes.0,
        0b0001_1111_1000_0000_0000_0000_0000_0000,
        0b0001_0010_1000_0000_0000_0000_0000_0000,
    ) {
        let op = bytes.immediate_unsigned(29, 2);

        if op == 0b01 {
            panic!();
        }

        let variant = bytes.variant();

        let shift = bytes.immediate_unsigned(21, 2) << 4;

        if matches!(variant, SizeVariant::Reg32) && shift > 16 {
            panic!();
        }

        let mut value = (bytes.immediate_unsigned(5, 16) as u64) << shift;

        if op == 0b00 {
            value = !value;

            if matches!(variant, SizeVariant::Reg32) {
                value &= 0xffff_ffff;
            }
        }

        return Some(Instruction::MoveWide {
            destination: bytes.register(0, false),
            keep_shift: if op == 0b11 { Some(shift) } else { None },
            source: bytes.register(5, false),
            value,
            variant,
        });
    }

    None
}
