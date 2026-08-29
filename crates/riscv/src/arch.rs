use aw_core::architecture::{Architecture, Instr, LocalDescriptor, LocalType, Width};
use raki::{Decode, Isa};

use crate::instruction::RiscVInstruction;

/// Stack pointer register (`sp` / `x2`).
const SP: u32 = 2;
/// Argument/return-value registers (`a0`..`a7` / `x10`..`x17`).
const ARG_REGISTERS: [u32; 8] = [10, 11, 12, 13, 14, 15, 16, 17];
/// Syscall-number register (`a7` / `x17`).
const SYSCALL_NUMBER_REGISTER: u32 = 17;
/// Zero register (`x0`).
const ZERO: u32 = 0;

#[derive(Debug)]
pub struct RiscV;

impl Architecture for RiscV {
    fn instruction_size(&self) -> u64 {
        // Halfword-granular: see the `RiscVInstruction` doc comment.
        2
    }

    fn decode_instructions(&self, bytes: &[u8]) -> Vec<Box<dyn Instr>> {
        let mut instructions: Vec<Box<dyn Instr>> = Vec::with_capacity(bytes.len() / 2);
        let mut offset = 0;

        while offset + 2 <= bytes.len() {
            let halfword = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());

            // The two low bits of the first halfword are `0b11` iff the
            // instruction is a full 32-bit (uncompressed) instruction.
            if halfword & 0b11 == 0b11 {
                let word = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
                let instruction = word
                    .decode(Isa::Rv64)
                    .expect("Failed to decode RISC-V instruction");

                instructions.push(Box::new(RiscVInstruction(instruction)));

                offset += 4;
            } else {
                let instruction = halfword
                    .decode(Isa::Rv64)
                    .expect("Failed to decode RISC-V instruction");

                instructions.push(Box::new(RiscVInstruction(instruction)));

                offset += 2;
            }
        }

        instructions
    }

    fn locals(&self) -> Vec<LocalDescriptor> {
        vec![
            // x1 (ra)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x2 (sp)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x3 (gp)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x4 (tp)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x5 (t0)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x6 (t1)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x7 (t2)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x8 (s0/fp)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x9 (s1)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x10 (a0)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x11 (a1)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x12 (a2)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x13 (a3)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x14 (a4)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x15 (a5)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x16 (a6)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x17 (a7)
            LocalDescriptor {
                argument: true,
                return_value: true,
                type_: LocalType::I64,
            },
            // x18 (s2)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x19 (s3)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x20 (s4)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x21 (s5)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x22 (s6)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x23 (s7)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x24 (s8)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x25 (s9)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x26 (s10)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x27 (s11)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x28 (t3)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x29 (t4)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x30 (t5)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
            // x31 (t6)
            LocalDescriptor {
                argument: false,
                return_value: false,
                type_: LocalType::I64,
            },
        ]
    }

    fn param_registers(&self) -> Vec<u32> {
        ARG_REGISTERS
            .iter()
            .copied()
            .chain(std::iter::once(SP))
            .collect()
    }

    fn svc_param_registers(&self) -> Vec<u32> {
        std::iter::once(SYSCALL_NUMBER_REGISTER)
            .chain(ARG_REGISTERS[..6].iter().copied())
            .collect()
    }

    fn svc_return_registers(&self) -> Vec<u32> {
        ARG_REGISTERS[..2].to_vec()
    }

    fn stack_pointer_register(&self) -> u32 {
        SP
    }

    fn is_zero_register(&self, register: u32) -> bool {
        register == ZERO
    }
}
