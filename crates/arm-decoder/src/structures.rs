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

impl SizeVariant {
    pub fn byte_count(&self) -> u64 {
        match self {
            SizeVariant::Reg32 => 4,
            SizeVariant::Reg64 => 8,
        }
    }

    pub fn log_byte_count(&self) -> u64 {
        match self {
            SizeVariant::Reg32 => 2,
            SizeVariant::Reg64 => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WritebackOffset {
    pub access: i32,
    pub writeback: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl SliceSize {
    pub fn byte_count(&self) -> u64 {
        match self {
            SliceSize::Byte => 1,
            SliceSize::Halfword => 2,
            SliceSize::Word => 4,
            SliceSize::Doubleword => 8,
        }
    }

    pub fn log_byte_count(&self) -> u64 {
        match self {
            SliceSize::Byte => 0,
            SliceSize::Halfword => 1,
            SliceSize::Word => 2,
            SliceSize::Doubleword => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extension {
    pub size: SliceSize,
    pub signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transform {
    Extension(Extension),
    LeftShift,
}

impl Extension {
    pub const UXTB: Extension = Extension {
        size: SliceSize::Byte,
        signed: false,
    };

    pub const SXTB: Extension = Extension {
        size: SliceSize::Byte,
        signed: true,
    };

    pub const UXTW: Extension = Extension {
        size: SliceSize::Word,
        signed: false,
    };

    pub const SXTW: Extension = Extension {
        size: SliceSize::Word,
        signed: true,
    };

    pub const SXTX: Extension = Extension {
        size: SliceSize::Doubleword,
        signed: true,
    };

    pub const UXTX: Extension = Extension {
        size: SliceSize::Doubleword,
        signed: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Promotion {
    P32 { sign_extend: bool },
    P64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    EQ, // Equal
    NE, // Not equal
    CS, // Carry set
    CC, // Carry clear
    MI, // Minus
    PL, // Plus
    VS, // Overflow
    VC, // No overflow
    HI, // Unsigned higher
    LS, // Unsigned lower or same
    GE, // Signed greater than or equal
    LT, // Signed less than
    GT, // Signed greater than
    LE, // Signed less than or equal
    AL, // Always
    NV, // Never (unpredictable)
}

impl Condition {
    pub fn decode(value: u32, allow_al_and_nv: bool) -> Self {
        match value {
            0b0000 => Condition::EQ,
            0b0001 => Condition::NE,
            0b0010 => Condition::CS,
            0b0011 => Condition::CC,
            0b0100 => Condition::MI,
            0b0101 => Condition::PL,
            0b0110 => Condition::VS,
            0b0111 => Condition::VC,
            0b1000 => Condition::HI,
            0b1001 => Condition::LS,
            0b1010 => Condition::GE,
            0b1011 => Condition::LT,
            0b1100 => Condition::GT,
            0b1101 => Condition::LE,
            0b1110 => {
                if !allow_al_and_nv {
                    panic!()
                }

                Condition::AL
            }
            0b1111 => {
                if !allow_al_and_nv {
                    panic!()
                }

                Condition::NV
            }
            _ => unreachable!(),
        }
    }
}
