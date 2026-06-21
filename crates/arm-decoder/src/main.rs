#![allow(unused)]

mod decoding;
mod instructions;
mod structures;
mod utilities;

use std::{collections::HashMap, time::Instant};
use capstone::arch::BuildsCapstone as _;
use crate::{decoding::{decode, logical::decode_bitmask}, instructions::Instruction};

const INSTRUCTION_SIZE: u64 = 4;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args().nth(1).expect("Please provide an ELF file path as an argument.");

    let elf_bytes = std::fs::read(&arg).expect("Could not read file.");
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
                    .into_iter()
                    .map(|instruction_index| {
                        let offset =
                            section_header.sh_offset + (instruction_index * INSTRUCTION_SIZE);
                        let instruction_bytes =
                            &elf_bytes[(offset as usize)..((offset + INSTRUCTION_SIZE) as usize)];
                        let instruction_value =
                            u32::from_le_bytes(instruction_bytes.try_into().unwrap());

                        decode(instruction_value)
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
                    let instruction = decode(instruction_value);

                    let address =
                        section_header.sh_addr + ((instruction_index as u64) * INSTRUCTION_SIZE);

                    print!("  [{:#010x}]", address);

                    let disassembled =
                        disassembler.disasm_all(instruction_bytes, address).unwrap();

                    let capstone_instruction = disassembled.iter().next().unwrap();
                    let mnemonic = capstone_instruction.mnemonic().unwrap();

                    unknown_counts
                        .entry(mnemonic.to_string())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    println!(
                        " {} {}",
                        mnemonic,
                        capstone_instruction.op_str().unwrap(),
                    );

                    println!("    {:?}", instruction);
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
