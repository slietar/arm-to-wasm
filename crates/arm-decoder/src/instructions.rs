use crate::structures::{Condition, Extension, Register, Shift, SizeVariant, SliceSize, Transform, WritebackOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And { set_flags: bool },
    Or,
    Xor,
}

impl LogicalOp {
    pub fn decode(value: u32) -> Self {
        match value {
            0b00 => LogicalOp::And { set_flags: false },
            0b01 => LogicalOp::Or,
            0b10 => LogicalOp::Xor,
            0b11 => LogicalOp::And { set_flags: true },
            _ => panic!("invalid logical op encoding: {value}"),
        }
    }
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
    LoadZeroExtend,
    LoadSignExtend { variant: SizeVariant },
    Store,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStoreOffset {
    Immediate {
        offset: WritebackOffset,
    },
    Register {
        extension: Option<Extension>,
        register: Register,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchTarget {
    RelativeInstructionOffset(i64),
    Register(Register),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitfieldMoveMode {
    Default,
    Signed,
    Unsigned,
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
        keep_shift: Option<u32>,
        source: Register,
        value: u64,
        variant: SizeVariant,
    },

    // AND, ANDS, EOR, ORR
    LogicalImmediate {
        destination: Register,
        op: LogicalOp,
        operand1: Register,
        operand2: u64,
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

    // STRB, LDRB, LDRSB, STRH, LDRH, LDRSH, STR, LDR, LDRSW
    // STURB, LDURB, LDURSB, STURH, LDURH, LDURSH, STUR, LDUR, LDURSW
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#ldst_regoff
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#ldst_pos
    LoadStoreRegister {
        address: Register,
        offset: LoadStoreOffset,
        op: LoadStoreOp,
        size: SliceSize,
        value: Register,
    },

    // LDR, LDRSW
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#loadlit
    LoadLiteral {
        destination: Register,
        relative_instruction_offset: i64,
        sign_extend: bool,
        size: SliceSize,
    },

    // STP, LDP, LDPSW
    LoadStorePairOfRegisters {
        address: Register,
        offset: WritebackOffset,
        op: LoadStoreOp,
        size: SizeVariant,
        value1: Register,
        value2: Register,
        variant: SizeVariant,
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

    // SBFM, BFM, UBFM
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Immediate?lang=en#bitfield
    BitfieldMove {
        destination: Register,
        leftmost_bit_number: u32,
        mode: BitfieldMoveMode,
        right_rotate_amount: u32,
        source: Register,
        variant: SizeVariant,
    },

    // BRK
    Breakpoint {
        immediate: u16,
    },

    // PRFM
    PrefetchMemory,

    // NOP
    Nop,

    // UDF
    PermanentlyUndefined {
        immediate: u16,
    },

    // SVC
    SupervisorCall {
        argument: u16,
    },

    // RET
    Return {
        target: Register,
    },

    // B, BR, BL, BLR
    UnconditionalBranch {
        link: bool,
        target: BranchTarget,
    },

    // B.cond
    BranchConditionally {
        condition: Condition,
        target: i64,
    },

    Unknown,
}
