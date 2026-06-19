use crate::utilities::{decode_bool, get_bits, sign_extend};

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
    fn decode(value: u32, sp_mode: bool) -> Self {
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
                if sp_mode {
                    SP
                } else {
                    XZR
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

impl AddressingMode {
    pub fn access_offset(&self) -> i32 {
        match self {
            AddressingMode::PostIndexWithWriteback { offset: _ } => 0,
            AddressingMode::PreIndex { offset } => *offset,
            AddressingMode::PreIndexWithWriteback { offset } => *offset,
        }
    }

    pub fn writeback_offset(&self) -> Option<i32> {
        match self {
            AddressingMode::PostIndexWithWriteback { offset } => Some(*offset),
            AddressingMode::PreIndex { offset: _ } => None,
            AddressingMode::PreIndexWithWriteback { offset } => Some(*offset),
        }
    }
}

pub struct InstructionBytes(pub u32);

impl InstructionBytes {
    pub fn bool(&self, start: u32) -> bool {
        decode_bool(self.0, start)
    }

    pub fn immediate(&self, start: u32, size: u32, signed: bool) -> i32 {
        let value = get_bits(self.0, start, size);

        if signed {
            sign_extend(value, size)
        } else {
            value as i32
        }
    }

    pub fn immediate_unsigned(&self, start: u32, size: u32) -> u32 {
        get_bits(self.0, start, size)
    }

    pub fn register(&self, start: u32, sp_mode: bool) -> Register {
        Register::decode(get_bits(self.0, start, 5), sp_mode)
    }

    pub fn variant(&self) -> SizeVariant {
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
    pub fn decode(value: u32, allow_ror: bool) -> Self {
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
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceSize {
    Byte,
    Halfword,
    Word,
    Doubleword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extension {
    pub size: SliceSize,
    pub signed: bool,
}
