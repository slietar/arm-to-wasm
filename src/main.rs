#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

mod analysis;
mod constants;
mod decoding;
mod instruction_helper;
mod instructions;
mod module;
mod translation;
// mod translator;

use clap::Parser;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = clap::Command::new("awsm")
        .subcommand_required(true)
        .subcommand(
            clap::command!("analyze")
                .about("Analyze an ELF file")
                .arg(clap::arg!(<FILE> "The ELF file to analyze").required(true)),
        )
        .subcommand(
            clap::command!("disassemble")
                .about("Disassemble an ELF file")
                .arg(clap::arg!(<FILE> "The ELF file to disassemble").required(true)),
        )
        .subcommand(
            clap::command!("translate")
                .about("Translate an ELF file to WebAssembly")
                .arg(clap::arg!(<FILE> "The ELF file to translate").required(true)),
        );

    let matches = command.get_matches();
    let (subcommand, subcommand_matches) = matches.subcommand().unwrap();

    let file_path = subcommand_matches.get_one::<String>("FILE").unwrap();
    let elf_bytes = std::fs::read(file_path).expect("Could not read file.");

    match subcommand {
        "analyze" => {
            analysis::main_analyze(&elf_bytes)?;
        }
        "disassemble" => {
            instructions::decode_file(&elf_bytes)?;
        }
        "translate" => {
            translation::translate(&elf_bytes)?;
        }
        _ => unreachable!(),
    }

    Ok(())
}
