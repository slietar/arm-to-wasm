use crate::decoding::{decode_bool, equal_masked, get_bits, get_bits_range, sign_extend};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Register {
    X0,
    X1,
    X2,
    X3,
    X4,
    X5,
    X6,
    X7,
    X8,
    X9,
    X10,
    X11,
    X12,
    X13,
    X14,
    X15,
    X16,
    X17,
    X18,
    X19,
    X20,
    X21,
    X22,
    X23,
    X24,
    X25,
    X26,
    X27,
    X28,
    X29,
    X30,
    SP,
    XZR,
}

impl Register {
    fn decode(value: u32, zero_mode: bool) -> Self {
        use Register::*;

        match value {
            0 => X0,
            1 => X1,
            2 => X2,
            3 => X3,
            4 => X4,
            5 => X5,
            6 => X6,
            7 => X7,
            8 => X8,
            9 => X9,
            10 => X10,
            11 => X11,
            12 => X12,
            13 => X13,
            14 => X14,
            15 => X15,
            16 => X16,
            17 => X17,
            18 => X18,
            19 => X19,
            20 => X20,
            21 => X21,
            22 => X22,
            23 => X23,
            24 => X24,
            25 => X25,
            26 => X26,
            27 => X27,
            28 => X28,
            29 => X29,
            30 => X30,
            31 => {
                if zero_mode {
                    XZR
                } else {
                    SP
                }
            }
            _ => panic!("invalid register encoding: {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SizeVariant {
    Reg32,
    Reg64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SizedRegister {
    register: Register,
    size: SizeVariant,
}

#[derive(Debug, Clone)]
pub struct Address {
    pub base: Register,
    pub mode: AddressingMode,
}

#[derive(Debug, Clone)]
pub enum AddressingMode {
    PostIndexWithWriteback { offset: i32 },
    PreIndex { offset: i32 },
    PreIndexWithWriteback { offset: i32 },
}

struct InstructionBytes(u32);

impl InstructionBytes {
    fn bool(&self, start: u32) -> bool {
        decode_bool(self.0, start)
    }

    fn immediate(&self, start: u32, size: u32, signed: bool) -> i32 {
        let value = get_bits(self.0, start, size);

        if signed {
            sign_extend(value, size)
        } else {
            value as i32
        }
    }

    fn immediate_unsigned(&self, start: u32, size: u32) -> u32 {
        get_bits(self.0, start, size)
    }

    fn register(&self, start: u32, zero_mode: bool) -> Register {
        Register::decode(get_bits(self.0, start, 5), zero_mode)
    }

    fn variant(&self) -> SizeVariant {
        if decode_bool(self.0, 31) {
            SizeVariant::Reg64
        } else {
            SizeVariant::Reg32
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Shift {
    LSL,
    LSR,
    ASR,
    ROR,
}

impl Shift {
    fn decode(value: u32, allow_ror: bool) -> Self {
        match value {
            0b00 => Shift::LSL,
            0b01 => Shift::LSR,
            0b10 => Shift::ASR,
            0b11 => {
                if allow_ror {
                    Shift::ROR
                } else {
                    panic!("invalid shift encoding: {value}")
                }
            },
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub enum Instruction {
    BitwiseOrShiftedRegister {
        destination: Register,
        operand1: Register,
        operand2: Register,
        shift_amount: u32,
        shift_type: Shift,
        variant: SizeVariant,
    },
    Branch {
        target: i64,
    },
    BranchWithLink {
        target: i64,
    },
    StoreRegisterImmediate {
        address: Address,
        value: Register,
        variant: SizeVariant,
    },
    // StorePairOfRegisters {
    //     address: Register,
    //     offset: u64,
    //     value1: Register64,
    //     value2: Register64,
    // },
    SubImmediate {
        destination: Register,
        operand: u64,
        source: Register,
        variant: SizeVariant,
    },
    SubShiftedRegister {
        destination: Register,
        operand2: Register,
        shift_amount: u32,
        shift_type: Shift,
        operand1: Register,
        variant: SizeVariant,
    },
    Unknown,
}

impl Instruction {
    pub fn decode(value: u32) -> Self {
        let bytes = InstructionBytes(value);

        // STR (immediate)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/STR--immediate---Store-register--immediate--
        if (value & 0b1011_1111_1110_0000_0000_1100_0000_0000)
            == 0b1011_1000_0000_0000_0000_0100_0000_0000
        {
            return Self::StoreRegisterImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PostIndexWithWriteback {
                        offset: bytes.immediate(12, 9, true),
                    },
                },
                value: bytes.register(0, false),
                variant: bytes.variant(),
            };
        }

        if (value & 0b1011_1111_1110_0000_0000_1100_0000_0000)
            == 0b1011_1000_0000_0000_0000_1100_0000_0000
        {
            return Self::StoreRegisterImmediate {
                address: Address {
                    base: Register::decode(get_bits(value, 5, 5), false),
                    mode: AddressingMode::PreIndexWithWriteback {
                        offset: sign_extend(get_bits(value, 12, 9), 9),
                    },
                },
                value: Register::decode(get_bits(value, 0, 5), false),
                variant: if decode_bool(value, 31) {
                    SizeVariant::Reg64
                } else {
                    SizeVariant::Reg32
                },
            };
        }

        if (value & 0b1011_1111_1100_0000_0000_1100_0000_0000)
            == 0b1011_1001_0000_0000_0000_1100_0000_0000
        {
            return Self::StoreRegisterImmediate {
                address: Address {
                    base: Register::decode(get_bits(value, 5, 5), false),
                    mode: AddressingMode::PreIndex {
                        offset: get_bits(value, 10, 12) as i32,
                    },
                },
                value: Register::decode(get_bits(value, 0, 5), false),
                variant: if decode_bool(value, 31) {
                    SizeVariant::Reg64
                } else {
                    SizeVariant::Reg32
                },
            };
        }

        // SUB (immediate)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/SUB--immediate---Subtract-immediate-value-
        if equal_masked(
            value,
            0b0111_1111_1000_0000_0000_0000_0000_0000,
            0b0101_0001_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::SubImmediate {
                destination: bytes.register(0, false),
                operand: (bytes.immediate_unsigned(10, 12) as u64)
                    << (if bytes.bool(22) { 12 } else { 0 }),
                source: bytes.register(5, false),
                variant: bytes.variant(),
            };
        }

        // SUB (shifted register)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/SUB--shifted-register---Subtract-optionally-shifted-register-
        if equal_masked(
            value,
            0b0111_1111_0010_0000_0000_0000_0000_0000,
            0b0100_1011_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::SubShiftedRegister {
                destination: bytes.register(0, true),
                operand1: bytes.register(5, true),
                operand2: bytes.register(16, true),
                shift_amount: get_bits(value, 10, 6),
                shift_type: Shift::decode(get_bits(value, 22, 2), false),
                variant: bytes.variant(),
            };
        }

        // ORR (shifted register)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/ORR--shifted-register---Bitwise-OR--shifted-register--?lang=en
        if equal_masked(
            value,
            0b0111_1111_0010_0000_0000_0000_0000_0000,
            0b0010_1010_0000_0000_0000_0000_0000_0000,
        ) {
            if !bytes.bool(31) && bytes.bool(15) {
                panic!();
            }

            return Self::BitwiseOrShiftedRegister {
                destination: bytes.register(0, true),
                operand1: bytes.register(5, true),
                operand2: bytes.register(16, true),
                shift_amount: get_bits(value, 10, 6),
                shift_type: Shift::decode(get_bits(value, 22, 2), false),
                variant: bytes.variant(),
            };
        }

        // B
        // Branch
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/B--Branch-?lang=en

        if equal_masked(
            value,
            0b1111_1100_0000_0000_0000_0000_0000_0000,
            0b0001_0100_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::Branch {
                target: sign_extend(get_bits(value, 0, 26), 26) as i64,
            };
        }

        // BL
        // Branch with link
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/BL--Branch-with-link-?lang=en

        if equal_masked(
            value,
            0b1111_1100_0000_0000_0000_0000_0000_0000,
            0b1001_0100_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::BranchWithLink {
                target: sign_extend(get_bits(value, 0, 26), 26) as i64,
            };
        }

        Self::Unknown
    }
}
