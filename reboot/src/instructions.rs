use std::{collections::HashMap, time::Instant};

use capstone::arch::BuildsCapstone as _;
use rayon::iter::{IntoParallelIterator, ParallelIterator as _};

use crate::{
    INSTRUCTION_SIZE,
    decoding::{decode_bool, equal_masked, get_bits, get_bits_range, sign_extend},
};

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
pub struct SizedRegister {
    pub register: Register,
    pub variant: SizeVariant,
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
            AddressingMode::PostIndexWithWriteback { offset } => 0,
            AddressingMode::PreIndex { offset } => *offset,
            AddressingMode::PreIndexWithWriteback { offset } => *offset,
        }
    }

    pub fn writeback_offset(&self) -> Option<i32> {
        match self {
            AddressingMode::PostIndexWithWriteback { offset } => Some(*offset),
            AddressingMode::PreIndex { .. } => None,
            AddressingMode::PreIndexWithWriteback { offset } => Some(*offset),
        }
    }
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
pub enum Extend {
    UXTB,
    UXTH,
    UXTW,
    UXTX,
    SXTB,
    SXTH,
    SXTW,
    SXTX,
}

impl Extend {
    // https://developer.arm.com/documentation/ddi0602/2023-03/Shared-Pseudocode/aarch64-functions-extendreg
    fn decode(value: u32) -> Self {
        match value {
            0b000 => Extend::UXTB,
            0b001 => Extend::UXTH,
            0b010 => Extend::UXTW,
            0b011 => Extend::UXTX,
            0b100 => Extend::SXTB,
            0b101 => Extend::SXTH,
            0b110 => Extend::SXTW,
            0b111 => Extend::SXTX,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShiftExtend {
    None,
    UXTW,
    LSL,
    SXTW,
    SXTX,
}

impl ShiftExtend {
    fn decode(value: u32) -> Self {
        match value {
            0b000 => ShiftExtend::None,
            0b010 => ShiftExtend::UXTW,
            0b011 => ShiftExtend::LSL,
            0b110 => ShiftExtend::SXTW,
            0b111 => ShiftExtend::SXTX,
            _ => panic!("invalid shift extend encoding: {value:03b}"),
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
            }
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Condition {
    EQ,
    NE,
    CS,
    CC,
    MI,
    PL,
    VS,
    VC,
    HI,
    LS,
    GE,
    LT,
    GT,
    LE,
    AL,
    NV,
}

impl Condition {
    fn decode(value: u32) -> Self {
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
            0b1110 => Condition::AL,
            0b1111 => Condition::NV,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Instruction {
    AddImmediate {
        destination: Register,
        operand: u64,
        source: Register,
        variant: SizeVariant,
    },
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
    BranchConditionally {
        target: i64,
        condition: Condition,
    },
    BranchWithLink {
        target: i64,
    },
    FormPCRelativeAddress {
        destination: Register,
        value: i64,
    },
    FormPCRelativeAddressToPage {
        destination: Register,
        value: i64,
    },
    LoadRegisterImmediate {
        address: Address,
        destination: Register,
        variant: SizeVariant,
    },
    MoveWideWithZero {
        destination: Register,
        value: u64,
        variant: SizeVariant,
    },
    Nop,
    Return {
        target: Register,
    },
    StoreRegisterImmediate {
        address: Address,
        value: Register,
        variant: SizeVariant,
    },
    StoreRegisterHalfwordImmediate {
        address: Address,
        value: Register,
    },
    StorePairOfRegisters {
        address: Address,
        value1: Register,
        value2: Register,
        variant: SizeVariant,
    },
    StoreRegisterRegister {
        base_address: Register,
        offset: Register,
        extend: ShiftExtend,
        extend_amount: u32,
        value: Register,
        variant: SizeVariant,
    },
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
    SubsImmediate {
        destination: Register,
        operand: u64,
        source: Register,
        variant: SizeVariant,
    },
    SupervisorCall {
        argument: u16,
    },
    TestBitAndBranchIfNonzero {
        bit: u32,
        target: i64,
        value: Register,
        variant: SizeVariant,
    },
    TestBitAndBranchIfZero {
        bit: u32,
        target: i64,
        value: Register,
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
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndexWithWriteback {
                        offset: bytes.immediate(12, 9, true),
                    },
                },
                value: bytes.register(0, true),
                variant: bytes.variant(),
            };
        }

        if (value & 0b1011_1111_1100_0000_0000_0000_0000_0000)
            == 0b1011_1001_0000_0000_0000_0000_0000_0000
        {
            return Self::StoreRegisterImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndex {
                        offset: (bytes.immediate_unsigned(10, 12)
                            << (if bytes.bool(31) { 3 } else { 2 }))
                            as i32,
                    },
                },
                value: bytes.register(0, true),
                variant: bytes.variant(),
            };
        }

        // LDR (immediate)
        // Load register (immediate)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/LDR--immediate---Load-register--immediate--?lang=en
        //
        // Same as STR with bit 22 set to 1

        if equal_masked(
            value,
            0b1011_1111_1110_0000_0000_1100_0000_0000,
            0b1011_1000_0100_0000_0000_0100_0000_0000,
        ) {
            return Self::LoadRegisterImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PostIndexWithWriteback {
                        offset: bytes.immediate(12, 9, true),
                    },
                },
                destination: bytes.register(0, false),
                variant: bytes.variant(),
            };
        }

        if (value & 0b1011_1111_1110_0000_0000_1100_0000_0000)
            == 0b1011_1000_0100_0000_0000_1100_0000_0000
        {
            return Self::LoadRegisterImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndexWithWriteback {
                        offset: bytes.immediate(12, 9, true),
                    },
                },
                destination: bytes.register(0, false),
                variant: bytes.variant(),
            };
        }

        if (value & 0b1011_1111_1100_0000_0000_0000_0000_0000)
            == 0b1011_1001_0100_0000_0000_0000_0000_0000
        {
            return Self::LoadRegisterImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndex {
                        offset: (bytes.immediate_unsigned(10, 12)
                            << (if bytes.bool(31) { 3 } else { 2 }))
                            as i32,
                    },
                },
                destination: bytes.register(0, false),
                variant: bytes.variant(),
            };
        }

        // STR (register)
        // Store register (register)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/STR--register---Store-register--register--?lang=en
        if equal_masked(
            value,
            0b1011_1111_1110_0000_0000_1100_0000_0000,
            0b1011_1000_0010_0000_0000_1000_0000_0000,
        ) {
            if !bytes.bool(14) {
                panic!();
            }

            return Self::StoreRegisterRegister {
                base_address: bytes.register(5, false),
                offset: bytes.register(16, true),
                extend: ShiftExtend::decode(get_bits(value, 22, 2)),
                extend_amount: if bytes.bool(11) {
                    if bytes.bool(31) { 3 } else { 2 }
                } else {
                    0
                },
                value: bytes.register(0, true),
                variant: bytes.variant(),
            };
        }

        // STRH (register)
        // Store register halfword (register)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/STRH--register---Store-register-halfword--register--?lang=en
        if equal_masked(
            value,
            0b1111_1111_1110_0000_0000_1100_0000_0000,
            0b0111_1000_0010_0000_0000_1000_0000_0000,
        ) {
            // return Self::StoreRegisterHalfwordImmediate {
            //     address: Address {
            //         base: bytes.register(5, false),
            //         mode: AddressingMode::PostIndexWithWriteback {
            //             offset: bytes.immediate(12, 9, true),
            //         },
            //     },
            //     value: bytes.register(0, false),
            // };

            // todo!()
        }

        // STRH (immediate)
        // Store register halfword (immediate)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/STRH--immediate---Store-register-halfword--immediate--?lang=en

        if equal_masked(
            value,
            0b1111_1111_1110_0000_0000_1100_0000_0000,
            0b0111_1000_0000_0000_0000_0100_0000_0000,
        ) {
            return Self::StoreRegisterHalfwordImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PostIndexWithWriteback {
                        offset: bytes.immediate(12, 9, true),
                    },
                },
                value: bytes.register(0, true),
            };
        }

        if equal_masked(
            value,
            0b1111_1111_1110_0000_0000_1100_0000_0000,
            0b0111_1000_0000_0000_0000_1100_0000_0000,
        ) {
            return Self::StoreRegisterHalfwordImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndexWithWriteback {
                        offset: bytes.immediate(12, 9, true),
                    },
                },
                value: bytes.register(0, true),
            };
        }

        if equal_masked(
            value,
            0b0111_1111_1100_0000_0000_0000_0000_0000,
            0b0111_1001_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::StoreRegisterHalfwordImmediate {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndex {
                        offset: (bytes.immediate_unsigned(10, 12) << 1) as i32,
                    },
                },
                value: bytes.register(0, true),
            };
        }

        // STP
        // Store pair of registers
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/STP--Store-pair-of-registers-?lang=en

        if equal_masked(
            value,
            0b0111_1111_1100_0000_0000_0000_0000_0000,
            0b0010_1000_1000_0000_0000_0000_0000_0000,
        ) {
            return Self::StorePairOfRegisters {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PostIndexWithWriteback {
                        offset: bytes.immediate(15, 7, true) * (if bytes.bool(31) { 8 } else { 4 }),
                    },
                },
                value1: bytes.register(0, true),
                value2: bytes.register(10, true),
                variant: bytes.variant(),
            };
        }

        if equal_masked(
            value,
            0b0111_1111_1100_0000_0000_0000_0000_0000,
            0b0010_1001_1000_0000_0000_0000_0000_0000,
        ) {
            return Self::StorePairOfRegisters {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndexWithWriteback {
                        offset: bytes.immediate(15, 7, true) * (if bytes.bool(31) { 8 } else { 4 }),
                    },
                },
                value1: bytes.register(0, true),
                value2: bytes.register(10, true),
                variant: bytes.variant(),
            };
        }

        if equal_masked(
            value,
            0b0111_1111_1100_0000_0000_0000_0000_0000,
            0b0010_1001_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::StorePairOfRegisters {
                address: Address {
                    base: bytes.register(5, false),
                    mode: AddressingMode::PreIndex {
                        offset: bytes.immediate(15, 7, true) * (if bytes.bool(31) { 8 } else { 4 }),
                    },
                },
                value1: bytes.register(0, true),
                value2: bytes.register(10, true),
                variant: bytes.variant(),
            };
        }

        // ADD (immediate)
        // Add immediate value
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/ADD--immediate---Add-immediate-value-?lang=en
        if equal_masked(
            value,
            0b0111_1111_1000_0000_0000_0000_0000_0000,
            0b0001_0001_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::AddImmediate {
                destination: bytes.register(0, false),
                operand: (bytes.immediate_unsigned(10, 12) as u64)
                    << (if bytes.bool(22) { 12 } else { 0 }),
                source: bytes.register(5, false),
                variant: bytes.variant(),
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

        // SUBS (immediate)
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/SUBS--immediate---Subtract-immediate-value--setting-flags-?lang=en
        if equal_masked(
            value,
            0b0111_1111_1000_0000_0000_0000_0000_0000,
            0b0111_0001_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::SubsImmediate {
                destination: bytes.register(0, true),
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
                target: bytes.immediate(0, 26, true) as i64,
            };
        }

        // B.cond
        // Branch conditionally
        if equal_masked(
            value,
            0b1111_1111_0000_0000_0000_0000_0001_0000,
            0b0101_0100_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::BranchConditionally {
                target: bytes.immediate(5, 19, true) as i64,
                condition: Condition::decode(get_bits(value, 0, 4)),
            };
        }

        // TBNZ
        // Test bit and branch if nonzero
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/TBNZ--Test-bit-and-branch-if-nonzero-

        if equal_masked(
            value,
            0b0111_1111_0000_0000_0000_0000_0000_0000,
            0b0011_0111_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::TestBitAndBranchIfNonzero {
                bit: get_bits(value, 31, 1) * 32 + get_bits(value, 19, 5),
                target: bytes.immediate(5, 14, true) as i64,
                value: bytes.register(0, true),
                variant: bytes.variant(),
            };
        }

        // TBZ
        // Test bit and branch if zero
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/TBZ--Test-bit-and-branch-if-zero-

        if equal_masked(
            value,
            0b0111_1111_0000_0000_0000_0000_0000_0000,
            0b0011_0110_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::TestBitAndBranchIfZero {
                bit: get_bits(value, 31, 1) * 32 + get_bits(value, 19, 5),
                target: bytes.immediate(5, 14, true) as i64,
                value: bytes.register(0, true),
                variant: bytes.variant(),
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
                target: bytes.immediate(0, 26, true) as i64,
            };
        }

        // RET
        // Return from subroutine
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/RET--Return-from-subroutine-?lang=en

        if equal_masked(
            value,
            0b1111_1111_1111_1111_1111_1100_0001_1111,
            0b1101_0110_0101_1111_0000_0000_0000_0000,
        ) {
            return Self::Return {
                target: bytes.register(5, true),
            };
        }

        // NOP
        // No operation
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/NOP--No-operation-?lang=en

        if equal_masked(
            value,
            0b1111_1111_1111_1111_1111_1111_1111_1111,
            0b1101_0101_0000_0011_0010_0000_0001_1111,
        ) {
            return Self::Nop;
        }

        // SVC
        // Supervisor call
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/SVC--Supervisor-call-?lang=en

        if equal_masked(
            value,
            0b1111_1111_1110_0000_0000_0000_0001_1111,
            0b1101_0100_0000_0000_0000_0000_0000_0001,
        ) {
            return Self::SupervisorCall {
                argument: get_bits(value, 5, 16) as u16,
            };
        }

        // MOVZ
        // Move wide with zero
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/MOVZ--Move-wide-with-zero-?lang=en

        if equal_masked(
            value,
            0b0111_1111_1000_0000_0000_0000_0000_0000,
            0b0101_0010_1000_0000_0000_0000_0000_0000,
        ) {
            return Self::MoveWideWithZero {
                destination: bytes.register(0, true),
                value: (get_bits(value, 5, 16) as u64) << ((get_bits(value, 21, 2) as u64) << 4),
                variant: bytes.variant(),
            };
        }

        // ADR
        // Form PC-relative address
        // https://developer.arm.com/documentation/ddi0602/2026-03/Base-Instructions/ADR--Form-PC-relative-address-?lang=en

        if equal_masked(
            value,
            0b1001_1111_0000_0000_0000_0000_0000_0000,
            0b0001_0000_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::FormPCRelativeAddress {
                destination: bytes.register(0, true),
                value: sign_extend((get_bits(value, 5, 19) << 2) | get_bits(value, 29, 2), 21)
                    as i64,
            };
        }

        // ADRP
        // Form PC-relative address to 4KB page

        if equal_masked(
            value,
            0b1001_1111_0000_0000_0000_0000_0000_0000,
            0b1001_0000_0000_0000_0000_0000_0000_0000,
        ) {
            return Self::FormPCRelativeAddressToPage {
                destination: bytes.register(0, true),
                value: (sign_extend((get_bits(value, 5, 19) << 2) | get_bits(value, 29, 2), 21)
                    as i64)
                    * (1 << 12),
            };
        }

        Self::Unknown
    }

    pub fn decode_bytes(data: &[u8]) -> Vec<Self> {
        (0..(data.len() / (INSTRUCTION_SIZE as usize)))
            .into_par_iter()
            .map(|instruction_index| {
                let offset = instruction_index * (INSTRUCTION_SIZE as usize);
                let bytes: &[_; 4] = &data[offset..][0..4].try_into().unwrap();
                Instruction::decode(u32::from_le_bytes(*bytes))
            })
            .collect::<Vec<_>>()
    }
}

pub fn decode_file(elf_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Find executable sections
    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&elf_bytes)?;
    let (section_headers_opt, section_name_table_opt) = elf_file.section_headers_with_strtab()?;

    let disassembler = capstone::Capstone::new()
        .arm64()
        .mode(capstone::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .unwrap();

    let mut unknown_counts = HashMap::new();

    if let Some(section_headers) = section_headers_opt {
        for section_header in section_headers {
            if (section_header.sh_flags & (elf::abi::SHF_EXECINSTR as u64)) != 0 {
                let section_name = section_name_table_opt
                    .as_ref()
                    .and_then(|strtab| Some(strtab.get(section_header.sh_name as usize)))
                    .unwrap_or(Ok("<unknown>"))?;

                println!("Section: {}", section_name);

                let instant = Instant::now();

                let instructions = (0..(section_header.sh_size / INSTRUCTION_SIZE))
                    .into_par_iter()
                    .map(|instruction_index| {
                        let offset =
                            section_header.sh_offset + (instruction_index * INSTRUCTION_SIZE);
                        let instruction_bytes =
                            &elf_bytes[(offset as usize)..((offset + INSTRUCTION_SIZE) as usize)];
                        let instruction_value =
                            u32::from_le_bytes(instruction_bytes.try_into().unwrap());

                        Instruction::decode(instruction_value)
                    });

                let unknown_count = instructions
                    .clone()
                    .filter(|instruction| matches!(instruction, Instruction::Unknown))
                    .count();

                let duration = instant.elapsed();

                eprintln!(
                    "Decoded {} instructions in {:?} ({:.2} M instructions/sec)",
                    section_header.sh_size / INSTRUCTION_SIZE,
                    duration,
                    (section_header.sh_size / INSTRUCTION_SIZE) as f64
                        / duration.as_secs_f64()
                        / 1e6
                );

                // unknown_counts.insert("foo".to_string(), unknown_count);

                for (instruction_index, instruction) in
                    instructions.collect::<Vec<_>>().into_iter().enumerate()
                {
                    let offset =
                        section_header.sh_offset + ((instruction_index as u64) * INSTRUCTION_SIZE);
                    let instruction_bytes =
                        &elf_bytes[(offset as usize)..((offset + INSTRUCTION_SIZE) as usize)];
                    let instruction_value =
                        u32::from_le_bytes(instruction_bytes.try_into().unwrap());
                    let instruction = Instruction::decode(instruction_value);

                    let address =
                        section_header.sh_addr + ((instruction_index as u64) * INSTRUCTION_SIZE);

                    println!("  [{:#010x}] {:?}", address, instruction);

                    if let Instruction::Unknown = instruction {
                        // if true {
                        let disassembled =
                            disassembler.disasm_all(instruction_bytes, address).unwrap();

                        for capstone_instruction in disassembled.iter() {
                            let mnemonic = capstone_instruction.mnemonic().unwrap();

                            unknown_counts
                                .entry(mnemonic.to_string())
                                .and_modify(|count| *count += 1)
                                .or_insert(1);

                            println!(
                                "                 {} {} {:032b}",
                                mnemonic,
                                capstone_instruction.op_str().unwrap(),
                                instruction_value,
                            );
                        }
                    }
                }
            }
        }
    }

    eprintln!("Unknown instruction counts:");

    let mut unknown_counts = unknown_counts.iter().collect::<Vec<_>>();

    unknown_counts.sort_by_key(|(_, count)| *count);

    for (mnemonic, count) in unknown_counts {
        eprintln!("  {:<8} {}", mnemonic, count);
    }

    Ok(())
}
