use crate::structures::{Extension, Register, Shift, SizeVariant, SliceSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode {
    Keep,
    Not,
    Zero, // Not valid in 64-bit mode
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And { set_flags: bool },
    Or,
    Xor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddSubtractOp {
    Add,
    Subtract,
}

impl AddSubtractOp {
    pub fn from_bool(value: bool) -> Self {
        if value {
            AddSubtractOp::Subtract
        } else {
            AddSubtractOp::Add
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddSubtractRightOperand {
    Immediate(u64),
    ShiftedRegister {
        register: Register,
        shift_amount: u64,
        shift_type: Shift,
    },
    ExtendedRegister {
        extension: Extension,
        left_shift_amount: u64,
        register: Register,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStoreOp {
    Load { sign_extend: bool },
    Store,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStoreOffset {
    Immediate(u64),
    Register {
        register: Register,
        extension: Extension,
        shift_amount: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalSelectMode {
    Identity,
    Increment,
    Invert,
    Negate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    // ADD, ADDS, SUB, SUBS
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Immediate?lang=en#addsub_imm
    AddSubtract {
        destination: Register,
        op: AddSubtractOp,
        set_flags: bool,
        operand1: Register,
        operand2: AddSubtractRightOperand,
        variant: SizeVariant,
    },

    // ADR, ADRP
    FormPCRelativeAddress {
        aligned_to_page: bool,
        destination: Register,
        value: i64,
    },

    // MOVN, MOVZ, MOVK
    MoveWide {
        destination: Register,
        mode: MoveMode,
        shift: u64,
        source: Register,
        value: u64,
        variant: SizeVariant,
    },

    // AND, ANDS, EOR, ORR
    LogicalImmediate {
        destination: Register,
        op: LogicalOp,
        bitmask_immediate: u64,
        source: Register,
        value: u64,
        variant: SizeVariant,
    },

    // AND, ANDS, BIC, BICS, ORR, ORN, EOR, EON
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Register#log_shift
    LogicalShiftedRegister {
        destination: Register,
        inverse_operand2: bool,
        op: LogicalOp,
        operand1: Register,
        operand2: Register,
        shift_amount: u64,
        shift_type: Shift,
        variant: SizeVariant,
    },

    // SBFM
    // https://developer.arm.com/documentation/dui0802/b/A64-General-Instructions/SBFM?lang=en
    SignedBitfieldMove {
        bit_count: u64,
        destination: Register,
        rotate_amount: u64,
        source: Register,
        variant: SizeVariant,
    },

    // STRB, LDRB, LDRSB, STRH, LDRH, LDRSH, STR, LDR
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#ldst_regoff
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#ldst_pos
    LoadStoreRegister {
        address: Register,
        offset: LoadStoreOffset,
        op: LoadStoreOp,
        size: SliceSize,
        value: Register,
    },

    // CSEL, CSINC, CSINV, CSNEG
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Register?lang=en#condsel
    ConditionalSelect {
        // condition: Condition,
        destination: Register,
        op: ConditionalSelectMode,
        operand1: Register,
        operand2: Register,
        variant: SizeVariant,
    },
}
