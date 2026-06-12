use crate::decoding::{decode_bool, get_bits, get_bits_range, sign_extend};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Register {
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
enum SizeVariant {
    Reg32,
    Reg64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SizedRegister {
    register: Register,
    size: SizeVariant,
}

#[derive(Debug, Clone)]
struct Address {
    base: Register,
    mode: AddressingMode,
}

#[derive(Debug, Clone)]
enum AddressingMode {
    PostIndexWithWriteback { offset: i32 },
    PreIndex { offset: i32 },
    PreIndexWithWriteback { offset: i32 },
}

#[derive(Debug)]
enum Instruction {
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
        operand: i32,
        source: Register,
        variant: SizeVariant,
    },
}

// if get_bits_range(value, 22, 30) == 0b010100010 {
// if (get_bits_range(value, 21, 31) & 0b10111111111) == 0b10111000000 {

impl Instruction {
    fn decode(value: u32) -> Self {
        if (value & 0b1011_1111_1110_0000_0000_1100_0000_0000) == 0b1011_1000_0000_0000_0000_0100_0000_0000 {
            return Self::StoreRegisterImmediate {
                address: Address {
                    base: Register::decode(get_bits(value, 5, 5), false),
                    mode: AddressingMode::PostIndexWithWriteback {
                        offset: sign_extend(get_bits(value, 12, 9), 9),
                    },
                },
                value: Register::decode(get_bits(value, 0, 5), false),
                variant: if decode_bool(value, 31) { SizeVariant::Reg64 } else { SizeVariant::Reg32 },
            };
        }

        if (value & 0b1011_1111_1110_0000_0000_1100_0000_0000) == 0b1011_1000_0000_0000_0000_1100_0000_0000 {
            return Self::StoreRegisterImmediate {
                address: Address {
                    base: Register::decode(get_bits(value, 5, 5), false),
                    mode: AddressingMode::PreIndexWithWriteback {
                        offset: sign_extend(get_bits(value, 12, 9), 9),
                    },
                },
                value: Register::decode(get_bits(value, 0, 5), false),
                variant: if decode_bool(value, 31) { SizeVariant::Reg64 } else { SizeVariant::Reg32 },
            };
        }

        if (value & 0b1011_1111_1100_0000_0000_1100_0000_0000) == 0b1011_1001_0000_0000_0000_1100_0000_0000 {
            return Self::StoreRegisterImmediate {
                address: Address {
                    base: Register::decode(get_bits(value, 5, 5), false),
                    mode: AddressingMode::PreIndex {
                        offset: get_bits(value, 10, 12) as i32,
                    },
                },
                value: Register::decode(get_bits(value, 0, 5), false),
                variant: if decode_bool(value, 31) { SizeVariant::Reg64 } else { SizeVariant::Reg32 },
            };
        }

        // if (value & 0b0111_1111_1000_0000_0000_0000_0000_0000) == 0b0101_0001_0000_0000_0000_0000_0000_0000 {
        //     return Self::SubImmediate {

        //         variant: if decode_bool(value, 31) { SizeVariant::Reg64 } else { SizeVariant::Reg32 },
        //     };
        // }

        todo!()
    }
}
