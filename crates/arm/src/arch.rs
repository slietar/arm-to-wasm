use arm_decoder::instructions::Instruction;
use arm_decoder::structures::Register;
use aw_core::architecture::{Architecture, Instr, Width};

use crate::registers;
use crate::translation::ArmInstruction;

#[derive(Debug)]
pub struct Arm;

impl Architecture for Arm {
    fn instruction_size(&self) -> u64 {
        4
    }

    fn decode_instructions(&self, bytes: &[u8]) -> Vec<Box<dyn Instr>> {
        Instruction::decode_bytes(bytes)
            .map(|instruction| Box::new(ArmInstruction(instruction)) as Box<dyn Instr>)
            .collect()
    }

    fn all_registers(&self) -> Vec<u32> {
        registers::DEFAULT_PARAM_REGISTERS
            .iter()
            .map(|r| registers::id(*r))
            .chain((0..registers::GP_REGISTER_COUNT).map(|reg_index| {
                registers::id(Register::decode(reg_index, false, true))
            }))
            .chain(registers::FLAG_IDS)
            .collect()
    }

    fn local_width(&self, register: u32) -> Width {
        if registers::FLAG_IDS.contains(&register) {
            Width::W32
        } else {
            Width::W64
        }
    }

    fn param_registers(&self) -> Vec<u32> {
        registers::DEFAULT_PARAM_REGISTERS
            .iter()
            .map(|r| registers::id(*r))
            .collect()
    }

    fn svc_param_registers(&self) -> Vec<u32> {
        registers::SVC_PARAM_REGISTERS
            .iter()
            .map(|r| registers::id(*r))
            .collect()
    }

    fn svc_return_registers(&self) -> Vec<u32> {
        registers::SVC_RETURN_REGISTERS
            .iter()
            .map(|r| registers::id(*r))
            .collect()
    }

    fn stack_pointer_register(&self) -> u32 {
        registers::id(Register::SP)
    }

    fn is_zero_register(&self, register: u32) -> bool {
        register == registers::id(Register::XZR)
    }
}
