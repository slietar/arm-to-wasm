use crate::{instructions::Instruction, structures::InstructionBytes};

mod addsub;
mod bitfield;
mod branch;
mod loadstore;
mod loadstore_pair;
mod logical;
mod misc;
mod movewide;

pub fn decode(value: u32) -> Instruction {
    let bytes = InstructionBytes(value);

    if let Some(instruction) = addsub::decode(bytes) {
        instruction
    } else if let Some(instruction) = bitfield::decode(bytes) {
        instruction
    } else if let Some(instruction) = branch::decode(bytes) {
        instruction
    } else if let Some(instruction) = loadstore::decode(bytes) {
        instruction
    } else if let Some(instruction) = loadstore_pair::decode(bytes) {
        instruction
    } else if let Some(instruction) = logical::decode(bytes) {
        instruction
    } else if let Some(instruction) = misc::decode(bytes) {
        instruction
    } else if let Some(instruction) = movewide::decode(bytes) {
        instruction
    } else {
        Instruction::Unknown
    }
}
