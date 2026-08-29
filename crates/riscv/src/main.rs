mod arch;
mod instruction;

use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = clap::Command::new("awsm")
        .subcommand_required(true)
        .subcommand(
            clap::command!("analyze")
                .about("Analyze an ELF file")
                .arg(clap::arg!(<FILE> "The ELF file to analyze").required(true)),
        )
        .subcommand(
            clap::command!("translate")
                .about("Translate an ELF file to WebAssembly")
                .arg(clap::arg!(<FILE> "The ELF file to translate").required(true))
                .arg(clap::arg!(--optimize "Optimize the generated WebAssembly").required(false)),
        );

    let matches = command.get_matches();
    let (subcommand, subcommand_matches) = matches.subcommand().unwrap();

    let file_path = subcommand_matches.get_one::<String>("FILE").unwrap();
    let elf_bytes = std::fs::read(file_path).expect("Could not read file.");

    match subcommand {
        "analyze" => {
            aw_core::analysis::main_analyze(&elf_bytes, &arch::RiscV)?;
        }
        "translate" => {
            let module = aw_core::translator::GlobalContext::translate_elf(
                &elf_bytes,
                Box::new(arch::RiscV),
            )?;

            let ok = module.validate();
            let optimize = subcommand_matches.get_flag("optimize");

            if ok {
                if optimize {
                    module.optimize();
                }

                module.write(&mut File::create("output.wasm")?)?;
            }

            println!("Module is valid: {}", ok);
        }
        _ => unreachable!(),
    }

    Ok(())
}
