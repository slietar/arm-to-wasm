#![allow(unused)]

mod decoding;
mod instructions;
mod structures;
mod utilities;

use crate::{decoding::decode, instructions::Instruction};
use capstone::arch::BuildsCapstone as _;
use std::{collections::HashMap, time::Instant};

const INSTRUCTION_SIZE: u64 = 4;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = std::env::args()
        .nth(1)
        .expect("Please provide an ELF file path as an argument.");

    let elf_bytes = std::fs::read(&arg).expect("Could not read file.");
    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&elf_bytes)?;
    let (section_headers_opt, section_name_table_opt) = elf_file.section_headers_with_strtab()?;

    let disassembler = capstone::Capstone::new()
        .arm64()
        .mode(capstone::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .unwrap();

    let mut counts = HashMap::new();

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
                    })
                    // .take(5)
                    .collect::<Vec<_>>();

                let unknown_count = instructions
                    .iter()
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

                for (instruction_index, _) in instructions.iter().enumerate() {
                    let offset =
                        section_header.sh_offset + ((instruction_index as u64) * INSTRUCTION_SIZE);
                    let instruction_bytes =
                        &elf_bytes[(offset as usize)..((offset + INSTRUCTION_SIZE) as usize)];
                    let instruction_value =
                        u32::from_le_bytes(instruction_bytes.try_into().unwrap());
                    let instruction = decode(instruction_value);

                    let address =
                        section_header.sh_addr + ((instruction_index as u64) * INSTRUCTION_SIZE);

                    let disassembled = disassembler.disasm_all(instruction_bytes, address).unwrap();

                    let capstone_instruction = disassembled.iter().next().unwrap();
                    let mnemonic = capstone_instruction.mnemonic().unwrap();

                    let is_unknown = matches!(instruction, Instruction::Unknown);

                    counts
                        .entry(mnemonic.to_string())
                        .and_modify(|(known_count, unknown_count)| {
                            if is_unknown {
                                *unknown_count += 1;
                            } else {
                                *known_count += 1;
                            }
                        })
                        .or_insert((
                            if is_unknown { 0 } else { 1 },
                            if is_unknown { 1 } else { 0 },
                        ));

                    if true {
                    // if let Instruction::Unknown = instruction {
                        print!("[{:#010x}]", address);
                        println!(" {} {}", mnemonic, capstone_instruction.op_str().unwrap(),);

                        println!("    {:032b}", instruction_value);
                        println!("    {:?}", instruction);
                    }
                }
            }
        }
    }

    eprintln!("Unknown instruction counts:");

    let mut unknown_counts = counts.iter().collect::<Vec<_>>();

    unknown_counts.sort_by_key(|(_, (known_count, unknown_count))| *unknown_count);

    for (mnemonic, (known_count, unknown_count)) in &unknown_counts {
        eprintln!("  {:<8} {} / {}", mnemonic, unknown_count, known_count + unknown_count);
    }

    let total_unknown_count: usize = unknown_counts
        .iter()
        .map(|(_, (known_count, unknown_count))| known_count + unknown_count)
        .sum();

    let total_known_count: usize = unknown_counts
        .iter()
        .map(|(_, (known_count, unknown_count))| known_count)
        .sum();

    eprintln!(
        "  {:<8} {} / {}",
        "Total", total_unknown_count, total_known_count + total_unknown_count
    );

    Ok(())
}
