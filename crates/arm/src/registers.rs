use arm_decoder::structures::Register;

/// Number of variants in `arm_decoder::structures::Register` (X0..X31, SP, XZR).
pub const GP_ID_COUNT: u32 = 34;

pub fn id(register: Register) -> u32 {
    register as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Carry,
    Negative,
    Zero,
    Overflow,
}

impl Flag {
    pub fn id(self) -> u32 {
        GP_ID_COUNT + (self as u32)
    }
}

pub const FLAG_IDS: [u32; 4] = [
    GP_ID_COUNT,
    GP_ID_COUNT + 1,
    GP_ID_COUNT + 2,
    GP_ID_COUNT + 3,
];

pub const DEFAULT_PARAM_REGISTERS: [Register; 9] = [
    Register::X0,
    Register::X1,
    Register::X2,
    Register::X3,
    Register::X4,
    Register::X5,
    Register::X6,
    Register::X7,
    Register::SP,
];

pub const SVC_PARAM_REGISTERS: [Register; 7] = [
    Register::X8,
    Register::X0,
    Register::X1,
    Register::X2,
    Register::X3,
    Register::X4,
    Register::X5,
];

pub const SVC_RETURN_REGISTERS: [Register; 2] = [Register::X0, Register::X1];

/// General-purpose registers that get a local allocated (X0..X30 - excludes X31/XZR).
pub const GP_REGISTER_COUNT: u32 = 31;
