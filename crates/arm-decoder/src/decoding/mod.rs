use crate::{instructions::Instruction, structures::InstructionBytes};

pub mod addsub;
pub mod loadstore;
pub mod logical;
pub mod misc;


pub fn decode(value: u32) -> Instruction {
    let bytes = InstructionBytes(value);

    if let Some(instruction) = addsub::decode(bytes) {
        instruction
    } else if let Some(instruction) = loadstore::decode(bytes) {
        instruction
    } else if let Some(instruction) = logical::decode(bytes) {
        instruction
    } else if let Some(instruction) = misc::decode(bytes) {
        instruction
    } else {
        Instruction::Unknown
    }
}
