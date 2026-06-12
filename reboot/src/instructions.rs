#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Register32 {
    W0,
    W1,
    W2,
    W3,
    W4,
    W5,
    W6,
    W7,
    W8,
    W9,
    W10,
    W11,
    W12,
    W13,
    W14,
    W15,
    W16,
    W17,
    W18,
    W19,
    W20,
    W21,
    W22,
    W23,
    W24,
    W25,
    W26,
    W27,
    W28,
    W29,
    W30,
    WSP,
    WZR,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Register64 {
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

impl Register64 {
    fn decode(value: u64, zero_mode: bool) -> Self {
        use Register64::*;

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
            31 => if zero_mode { XZR } else { SP },
            _ => panic!("invalid register encoding: {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Register {
    Reg32(Register32),
    Reg64(Register64),
}


#[derive(Debug, Clone)]
struct Address {
    base: Register,
    mode: AddressingMode,
}

#[derive(Debug, Clone)]
enum AddressingMode {
    PostIndexWithWriteback {
        offset: i64,
    },
    PreIndex {
        offset: i64,
    },
    PreIndexWithWriteback {
        offset: i64,
    },
}


#[derive(Debug)]
enum Instruction {
    StoreRegisterImmediate {
        address: Address,
        value: Register,
    },
    // StorePairOfRegisters {
    //     address: Register,
    //     offset: u64,
    //     value1: Register64,
    //     value2: Register64,
    // },
    SubImmediate64 {
        destination: Register64,
        operand: u64,
        source: Register64,
    },
    SubImmediate32 {
        destination: Register32,
        operand: u64,
        source: Register32,
    },
}
