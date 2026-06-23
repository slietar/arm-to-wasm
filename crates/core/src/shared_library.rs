use crate::analysis::ElfFile;

// Relevant sections
//
// versym
//    .gnu.version
// verneed
//    .gnu.version_r
// verdef
//    .gnu.version_d

#[derive(Debug)]
pub struct NeededLibrary {
    name: String,
    versions: Vec<String>,
}

#[derive(Debug)]
pub struct Symbol {
    name: String,
}

pub fn analyze_shared_library(elf_file: ElfFile) -> Result<(), Box<dyn std::error::Error>> {
    let dynamic_table = elf_file.dynamic()?.unwrap();
    let (dynamic_symbol_table, dynamic_symbol_string_table) =
        elf_file.dynamic_symbol_table()?.unwrap();

    let needed_library_names = dynamic_table
        .iter()
        .filter(|entry| entry.d_tag == elf::abi::DT_NEEDED)
        .map(|entry| dynamic_symbol_string_table.get(entry.d_val() as usize))
        .collect::<Result<Vec<_>, _>>()?;

    let versym_section = elf_file.section_headers().and_then(|sections| {
        sections
            .iter()
            .find(|section| section.sh_type == elf::abi::SHT_GNU_VERSYM)
    });

    let symbols = versym_section
        .map(|versym_section| -> Result<Vec<Symbol>, Box<dyn std::error::Error>> {
            let (versym_data, _) = elf_file.section_data(&versym_section)?;
            let versym_table = elf::gnu_symver::VersionIndexTable::new(
                elf_file.ehdr.endianness,
                elf_file.ehdr.class,
                versym_data,
            );

            Ok(dynamic_symbol_table
                .iter()
                .enumerate()
                .map(|(symbol_index, symbol)| {
                    let symbol_name = dynamic_symbol_string_table
                        .get(symbol.st_name as usize)
                        .unwrap_or("<unknown>");

                    match versym_table.get(symbol_index) {
                        Ok(version_index) => {
                            eprintln!(
                                "Version symbol: sym_index: {}, name: {}, version: {}, hidden: {}, local: {}, global: {}",
                                symbol_index,
                                symbol_name,
                                version_index.index(),
                                version_index.is_hidden(),
                                version_index.is_local(),
                                version_index.is_global(),
                            );
                        }
                        Err(_) => {
                            eprintln!(
                                "Version symbol: sym_index: {}, name: {}, version: <missing>",
                                symbol_index, symbol_name
                            );
                        }
                    }

                    todo!()
                })
                .collect::<Vec<_>>())
        })
        .transpose()?;

    let verneed_section = elf_file.section_headers().and_then(|sections| {
        sections
            .iter()
            .find(|section| section.sh_type == elf::abi::SHT_GNU_VERNEED)
    });

    let needed_libraries = verneed_section
        .map(
            |verneed_section| -> Result<Vec<NeededLibrary>, Box<dyn std::error::Error>> {
                let (verneed_data, _) = elf_file.section_data(&verneed_section)?;
                let linked_strtab_header = elf_file
                    .section_headers()
                    .unwrap()
                    .get(verneed_section.sh_link as usize)?;
                let verneed_strings = elf_file.section_data_as_strtab(&linked_strtab_header)?;

                Ok(elf::gnu_symver::VerNeedIterator::new(
                    elf_file.ehdr.endianness,
                    elf_file.ehdr.class,
                    verneed_section.sh_info as u64,
                    0,
                    verneed_data,
                )
                .map(|(verneed, vernaux_iter)| {
                    let aux_names = vernaux_iter
                        .filter_map(|vernaux| {
                            verneed_strings
                                .get(vernaux.vna_name as usize)
                                .ok()
                                .map(str::to_string)
                        })
                        .collect::<Vec<_>>();

                    NeededLibrary {
                        name: verneed_strings
                            .get(verneed.vn_file as usize)
                            .unwrap_or("<unknown>")
                            .to_string(),
                        versions: aux_names,
                    }
                })
                .collect::<Vec<_>>())
            },
        )
        .transpose()?;

    for entry in dynamic_table {
        match entry.d_tag {
            elf::abi::DT_NEEDED => {
                let x = dynamic_symbol_string_table.get(entry.d_val() as usize)?;
                // eprintln!("Needed shared library: {:?}", x);
                // needed_shared_libraries.push();
            }
            elf::abi::DT_VERNEED => {
                eprintln!("Version needed: {}", entry.d_val());
            }
            elf::abi::DT_VERSYM => {
                eprintln!("Version definition: {}", entry.d_val());
            }
            // elf::abi::DT_RPATH => {
            //     eprintln!("RPATH: {}", elf_file.dynstrtab()?.get(entry.d_val() as usize)?);
            // }
            // elf::abi::DT_RUNPATH => {
            //     eprintln!("RUNPATH: {}", elf_file.dynstrtab()?.get(entry.d_val() as usize)?);
            // }
            _ => {}
        }
    }

    eprintln!("Needed library names: {:?}", needed_library_names);
    eprintln!("Needed libraries: {:#?}", needed_libraries);

    // if let Some(section_headers) = elf_file.section_headers() {
    //     for versym_section in section_headers
    //         .iter()
    //         .filter(|section| section.sh_type == elf::abi::SHT_GNU_VERSYM)
    //     {
    //         let (versym_data, _) = elf_file.section_data(&versym_section)?;
    //         let versym_table = elf::gnu_symver::VersionIndexTable::new(
    //             elf_file.ehdr.endianness,
    //             elf_file.ehdr.class,
    //             versym_data,
    //         );

    //         for (symbol_index, symbol) in dynamic_symbol_table.iter().enumerate() {
    //             let symbol_name = dynamic_symbol_string_table
    //                 .get(symbol.st_name as usize)
    //                 .unwrap_or("<unknown>");

    //             match versym_table.get(symbol_index) {
    //                 Ok(version_index) => {
    //                     eprintln!(
    //                         "Version symbol: sym_index: {}, name: {}, version: {}, hidden: {}, local: {}, global: {}",
    //                         symbol_index,
    //                         symbol_name,
    //                         version_index.index(),
    //                         version_index.is_hidden(),
    //                         version_index.is_local(),
    //                         version_index.is_global(),
    //                     );
    //                 }
    //                 Err(_) => {
    //                     eprintln!(
    //                         "Version symbol: sym_index: {}, name: {}, version: <missing>",
    //                         symbol_index, symbol_name
    //                     );
    //                     break;
    //                 }
    //             }
    //         }
    //     }

    //     for verneed_section in section_headers
    //         .iter()
    //         .filter(|section| section.sh_type == elf::abi::SHT_GNU_VERNEED)
    //     {
    //         let (verneed_data, _) = elf_file.section_data(&verneed_section)?;
    //         let linked_strtab_header = section_headers.get(verneed_section.sh_link as usize)?;
    //         let verneed_strings = elf_file.section_data_as_strtab(&linked_strtab_header)?;

    //         for (verneed, vernaux_iter) in elf::gnu_symver::VerNeedIterator::new(
    //             elf_file.ehdr.endianness,
    //             elf_file.ehdr.class,
    //             verneed_section.sh_info as u64,
    //             0,
    //             verneed_data,
    //         ) {
    //             let aux_names = vernaux_iter
    //                 .filter_map(|vernaux| {
    //                     verneed_strings
    //                         .get(vernaux.vna_name as usize)
    //                         .ok()
    //                         .map(str::to_string)
    //                 })
    //                 .collect::<Vec<_>>();

    //             eprintln!(
    //                 "Version needed: index: {}, file: {}, names: {:?}",
    //                 verneed.vn_cnt,
    //                 verneed_strings
    //                     .get(verneed.vn_file as usize)
    //                     .unwrap_or("<unknown>"),
    //                 aux_names
    //             );
    //         }

    //         // for (verdef, verdaux_iter) in gnu_symver::VerDefIterator::new(
    //         //     elf_file.ehdr.endianness,
    //         //     elf_file.ehdr.class,
    //         //     verdef_section.sh_info as u64,
    //         //     0,
    //         //     verdef_data,
    //         // ) {
    //         //     let aux_names = verdaux_iter
    //         //         .filter_map(|verdaux| {
    //         //             verdef_strings
    //         //                 .get(verdaux.vda_name as usize)
    //         //                 .ok()
    //         //                 .map(str::to_string)
    //         //         })
    //         //         .collect::<Vec<_>>();

    //         //     eprintln!(
    //         //         "Version definition: index: {}, flags: {}, hash: {}, names: {:?}",
    //         //         verdef.vd_ndx, verdef.vd_flags, verdef.vd_hash, aux_names
    //         //     );
    //         // }
    //     }
    // }

    Ok(())
}
