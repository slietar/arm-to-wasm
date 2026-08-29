use aw_core::architecture::{Architecture, Instr, Width};
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

    fn all_registers(&self) -> Vec<u32> {
        // x0 (the zero register) never needs a backing local: `is_zero_register`
        // short-circuits every read/write of it.
        (1..32).collect()
    }

    fn local_width(&self, _register: u32) -> Width {
        Width::W64
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
