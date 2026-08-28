use crate::{
    structures::{
        AnySize, Arrangement, Condition, Extension, LargeSize, Register, Shift, SizeVariant,
        SliceSize, WritebackOffset,
    },
    utilities::INSTRUCTION_SIZE,
};

// Any: Byte, Half, Single, Double, Quad
// General: Byte, Half, Single, Double
// Register: Single, Double
// Large: Single, Double, Quad
// Small: Byte, Half

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicalImmediateOperand {
    Immediate(u64),
    ShiftedRegister {
        inverse: bool,
        register: Register,
        shift_amount: u64,
        shift_type: Shift,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BivalentObject<GP, FP> {
    GeneralPurpose(GP),
    FloatingPoint(FP),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStoreOp {
    Load(BivalentObject<GPLoadMode, AnySize>),
    Store(BivalentObject<SliceSize, AnySize>),
}

impl LoadStoreOp {
    pub fn access_size(&self) -> AnySize {
        match self {
            LoadStoreOp::Load(BivalentObject::GeneralPurpose(mode)) => mode.access_size().into(),
            LoadStoreOp::Load(BivalentObject::FloatingPoint(size)) => *size,
            LoadStoreOp::Store(BivalentObject::GeneralPurpose(size)) => (*size).into(),
            LoadStoreOp::Store(BivalentObject::FloatingPoint(size)) => *size,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadStoreIndex {
    Immediate {
        offset: WritebackOffset,
    },
    Register {
        mode: IndexMode,
        register: Register,
        shift_amount: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexMode {
    UnsignedSingle,
    SignedSingle,
    Double,
}

impl IndexMode {
    pub fn variant(&self) -> SizeVariant {
        match self {
            IndexMode::UnsignedSingle | IndexMode::SignedSingle => SizeVariant::Reg32,
            IndexMode::Double => SizeVariant::Reg64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GPLoadMode {
    UnsignedByte,
    SignedByteToSingle,
    SignedByteToDouble,

    UnsignedHalf,
    SignedHalfToSingle,
    SignedHalfToDouble,

    UnsignedSingle,
    SignedSingleToDouble,

    Double,
}

impl GPLoadMode {
    pub fn access_size(&self) -> SliceSize {
        match self {
            GPLoadMode::UnsignedByte
            | GPLoadMode::SignedByteToSingle
            | GPLoadMode::SignedByteToDouble => SliceSize::Byte,
            GPLoadMode::UnsignedHalf
            | GPLoadMode::SignedHalfToSingle
            | GPLoadMode::SignedHalfToDouble => SliceSize::Halfword,
            GPLoadMode::UnsignedSingle | GPLoadMode::SignedSingleToDouble => SliceSize::Word,
            GPLoadMode::Double => SliceSize::Doubleword,
        }
    }

    pub fn register_size(&self) -> SizeVariant {
        use GPLoadMode::*;

        match self {
            UnsignedByte => SizeVariant::Reg32,
            SignedByteToSingle => SizeVariant::Reg32,
            SignedByteToDouble => SizeVariant::Reg64,

            UnsignedHalf => SizeVariant::Reg32,
            SignedHalfToSingle => SizeVariant::Reg32,
            SignedHalfToDouble => SizeVariant::Reg64,

            UnsignedSingle => SizeVariant::Reg32,
            SignedSingleToDouble => SizeVariant::Reg64,

            Double => SizeVariant::Reg64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadLiteralGPMode {
    UnsignedSingle,
    SignedSingleToDouble,
    Double,
}

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct SmallSizeUsageTarget {
//     pub sign_extend: bool,
//     pub size: SizeVariant,
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum SingleSizeUsageTarget {
//     Single,
//     Double { sign_extend: bool },
// }

// #[derive(Debug, Clone, PartialEq, Eq)]
// pub enum GeneralSizeUsage {
//     Byte { sign_extend: bool },
//     Half { sign_extend: bool },
//     Single { sign_extend_size: Option<SizeVariant> },
//     Double,
// }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FPToIntegerRoundingMode {
    Away,
    Even,
    PlusInfinity,
    MinusInfinity,
    Zero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FPMoveDirection {
    FPToInteger,
    IntegerToFP,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FPMoveFPSize {
    Half,
    Single,
    LowerDouble,
    UpperDouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertFPIntegerOp {
    IntegerToFP,
    FPToInteger {
        rounding_mode: FPToIntegerRoundingMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FPProcessingOp {
    Multiply,        // FMUL
    Divide,          // FDIV
    Add,             // FADD
    Subtract,        // FSUB
    Maximum,         // FMAX
    Minimum,         // FMIN
    MaximumNumber,   // FMAXNM
    MinimumNumber,   // FMINNM
    NegatedMultiply, // FNMUL
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SIMDTripleOp {
    SignedHalvingAdd,         // SHADD
    SignedSaturatingAdd,      // SQADD
    SignedRoundingHalvingAdd, // SRHADD
    SignedHalvingSubtract,    // SHSUB
    SignedSaturatingSubtract, // SQSUB

    CompareSignedGT, // CMGT
    CompareSignedGE, // CMGE

    SignedShiftLeft,                   // SSHL
    SignedSaturatingShiftLeft,         // SQSHL
    SignedRoundingShiftLeft,           // SRSHL
    SignedSaturatingRoundingShiftLeft, // SQRSHL

    SignedMaximum, // SMAX
    SignedMinimum, // SMIN

    SignedAbsoluteDifference,              // SABD
    SignedAbsoluteDifferenceAndAccumulate, // SABA

    Add,                                               // ADD
    CompareBitwiseTestBitsNonzero,                     // CMTST
    MultiplyAddAccumulate,                             // MLA
    Multiply,                                          // MUL
    SignedMaximumPairwise,                             // SMAXP
    SignedMinimumPairwise,                             // SMINP
    SignedSaturatingDoublingMultiplyReturningHighHalf, // SQDMULH
    AddPairwise,                                       // ADDP
    FPMaximumNumber,                                   // FMAXNMP
    FPMultiplyAddAccumulate,                           // FMLA
    FPAdd,                                             // FADD
    FPMultiplyExtended,                                // FMULX
    FPCompareEqual,                                    // FCMEQ
    FPMaximum,                                         // FMAX
    FPReciprocalStep,                                  // FRECPS
    BitwiseAnd,                                        // AND
    FPFusedMultiplyAddLongToAccumulator,               // FMLAL, FMLAL2
    BitwiseBitClear,                                   // BIC
    FPMinimumNumber,                                   // FMINNMP
    FPFusedMultiplySubtractFromAccumulator,            // FMLS
    FPSubtract,                                        // FSUB
    FPAbsoluteMaximum,                                 // FAMAX
    FPMinimum,                                         // FMIN
    FPReciprocalSquareRootStep,                        // FRSQRTS
    BitwiseOr,                                         // ORR
    FPFusedMultiplySubtractLongFromAccumulator,        // FMLSL, FMLSL2
    BitwiseOrNot,                                      // ORN

    UnsignedHalvingAdd,         // UHADD
    UnsignedSaturatingAdd,      // UQADD
    UnsignedRoundingHalvingAdd, // URHADD
    UnsignedHalvingSubtract,    // UHSUB
    UnsignedSaturatingSubtract, // UQSUB

    CompareUnsignedGT, // CMHI
    CompareUnsignedGE, // CMHS

    UnsignedShiftLeft,                   // USHL
    UnsignedSaturatingShiftLeft,         // UQSHL
    UnsignedRoundingShiftLeft,           // URSHL
    UnsignedSaturatingRoundingShiftLeft, // UQRSHL

    UnsignedMaximum, // UMAX
    UnsignedMinimum, // UMIN

    UnsignedAbsoluteDifference,              // UABD
    UnsignedAbsoluteDifferenceAndAccumulate, // UABA

    Subtract,                                                  // SUB
    CompareEqual,                                              // CMEQ
    MultiplySubtract,                                          // MLS
    PolynomialMultiply,                                        // PMUL
    UnsignedMaximumPairwise,                                   // UMAXP
    UnsignedMinimumPairwise,                                   // UMINP
    SignedSaturatingRoundingDoublingMultiplyReturningHighHalf, // SQRDMULH

    FPMaximumNumberPairwise,                             // FMAXNMP
    FPAddPairwise,                                       // FADDP
    FPMultiply,                                          // FMUL
    FPCompareGE,                                         // FCMGE
    FPAbsoluteCompareGE,                                 // FACGE
    FPMaximumPairwise,                                   // FMAXP
    FPDivide,                                            // FDIV
    BitwiseXor,                                          // EOR
    FPFusedMultiplyAddLongToAccumulatorUpperHalf,        // FMLAL2
    BitwiseSelect,                                       // BSL
    FPMinimumNumberPairwise,                             // FMINNMP
    FPAbsoluteDifference,                                // FABD
    FPAbsoluteMinimum,                                   // FAMIN
    FPCompareGT,                                         // FCMGT
    FPAbsoluteCompareGT,                                 // FACGT
    FPMinimumPairwise,                                   // FMINP
    FPScale,                                             // FSCALE
    BitwiseInsertIfTrue,                                 // BIT
    FPFusedMultiplySubtractLongFromAccumulatorUpperHalf, // FMLSL2
    BitwiseInsertIfFalse,                                // BIF
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

    // AND, ANDS, BIC, BICS, ORR, ORN, EOR, EON
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Register#log_shift
    Logical {
        destination: Register,
        op: LogicalOp,
        operand1: Register,
        operand2: LogicalImmediateOperand,
        variant: SizeVariant,
    },

    // STRB, LDRB, LDRSB, STRH, LDRH, LDRSH, STR, LDR, LDRSW
    // STURB, LDURB, LDURSB, STURH, LDURH, LDURSH, STUR, LDUR, LDURSW
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#ldst_regoff
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#ldst_pos
    LoadStoreRegister {
        address_base: Register,
        address_index: LoadStoreIndex,
        op: LoadStoreOp,
        value: Register,
    },

    // LDR, LDRSW
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Loads-and-Stores?lang=en#loadlit
    LoadLiteral {
        destination: Register,
        op: BivalentObject<LoadLiteralGPMode, LargeSize>,
        relative_instruction_offset: i64,
    },

    // STP, LDP, LDPSW
    LoadStorePairOfRegisters {
        address_base: Register,
        address_index: WritebackOffset,
        op: LoadStoreOp,
        size: SizeVariant,
        value1: Register,
        value2: Register,
        variant: SizeVariant,
    },

    // CSEL, CSINC, CSINV, CSNEG
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Register?lang=en#condsel
    ConditionalSelect {
        condition: Condition,
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

    // CBZ, CBNZ
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Branches--Exception-Generating-and-System-instructions?lang=en#compbranch
    // TODO: Combine with other CB instructions
    CompareAndBranch {
        branch_if_zero: bool,
        register: Register,
        target: i64,
        variant: SizeVariant,
    },

    // TBZ, TBNZ
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Branches--Exception-Generating-and-System-instructions?lang=en#testbranch
    TestBitAndBranch {
        branch_if_zero: bool,
        register: Register,
        target: i64,
        test_bit: u32,
        variant: SizeVariant,
    },

    // Allowed conversions are (W, X) <-> (S, D, H)
    ConvertFPInteger {
        destination: Register,
        fp_size: AnySize,
        integer_size: SizeVariant,
        op: ConvertFPIntegerOp,
        operand: Register,
        signed: bool,
    },

    // Allowed conversions are:
    //  (W, X) <-> H
    //  W      <-> S
    //  X      <-> (D, V.D[1])
    FPMove {
        destination: Register,
        direction: FPMoveDirection,
        fp_size: FPMoveFPSize,
        integer_size: SizeVariant,
        operand: Register,
    },

    FPProcessing {
        destination: Register,
        op: FPProcessingOp,
        operand1: Register,
        operand2: Register,
        size: AnySize,
    },

    SIMDTriple {
        arrangement: Arrangement,
        destination: Register,
        op: SIMDTripleOp,
        operand1: Register,
        operand2: Register,
    },

    Unknown,
}

impl Instruction {
    pub fn decode(value: u32) -> Self {
        crate::decoding::decode(value)
    }

    pub fn decode_bytes(data: &[u8]) -> impl Iterator<Item = Self> {
        (0..(data.len() / (INSTRUCTION_SIZE as usize)))
            .into_iter()
            .map(|instruction_index| {
                let offset = instruction_index * (INSTRUCTION_SIZE as usize);
                let bytes: &[_; 4] = &data[offset..][0..4].try_into().unwrap();
                Instruction::decode(u32::from_le_bytes(*bytes))
            })
    }
}
