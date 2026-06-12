use capstone::arch::BuildsCapstone as _;

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
    UXTW,
    LSL,
    SXTW,
    SXTX,
}

impl ShiftExtend {
    fn decode(value: u32) -> Self {
        match value {
            0b010 => ShiftExtend::UXTW,
            0b011 => ShiftExtend::LSL,
            0b110 => ShiftExtend::SXTW,
            0b111 => ShiftExtend::SXTX,
            _ => unreachable!(),
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

#[derive(Debug)]
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
    BranchWithLink {
        target: i64,
    },
    FormPCRelativeAddress {
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
    // StorePairOfRegisters {
    //     address: Register,
    //     offset: u64,
    //     value1: Register64,
    //     value2: Register64,
    // },
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
    SupervisorCall {
        argument: u16,
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

            todo!()
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
            0b1111_1111_1100_0000_0000_0000_0000_0000,
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

        Self::Unknown
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

    if let Some(section_headers) = section_headers_opt {
        for section_header in section_headers {
            if (section_header.sh_flags & (elf::abi::SHF_EXECINSTR as u64)) != 0 {
                let section_name = section_name_table_opt
                    .as_ref()
                    .and_then(|strtab| Some(strtab.get(section_header.sh_name as usize)))
                    .unwrap_or(Ok("<unknown>"))?;

                println!("Section: {}", section_name);

                for instruction_index in 0..(section_header.sh_size / INSTRUCTION_SIZE) {
                    let offset = section_header.sh_offset + (instruction_index * INSTRUCTION_SIZE);
                    let instruction_bytes =
                        &elf_bytes[(offset as usize)..((offset + INSTRUCTION_SIZE) as usize)];
                    let instruction_value =
                        u32::from_le_bytes(instruction_bytes.try_into().unwrap());
                    let instruction = Instruction::decode(instruction_value);

                    let address = section_header.sh_addr + (instruction_index * INSTRUCTION_SIZE);

                    println!("  [{:#010x}] {:?}", address, instruction);

                    if let Instruction::Unknown = instruction {
                        // if true {
                        let disassembled =
                            disassembler.disasm_all(instruction_bytes, address).unwrap();

                        for capstone_instruction in disassembled.iter() {
                            println!(
                                "                 {} {} {:032b}",
                                capstone_instruction.mnemonic().unwrap(),
                                capstone_instruction.op_str().unwrap(),
                                instruction_value,
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
