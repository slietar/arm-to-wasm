use std::collections::HashMap;

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
pub enum LibraryVersion {
    Exported {
        dependencies: Vec<String>,
        version: String,
    },
    Imported {
        soname: String,
        version: String,
    },
}

#[derive(Debug)]
pub struct Symbol {
    pub dynamic_symbol_index: usize,
    pub name: String,
    pub kind: SymbolKind,
}

#[derive(Debug)]
pub enum SymbolKind {
    Local,
    Exported {
        address: u64,
        version_index: Option<usize>,
    },
    Imported {
        version_index: Option<usize>,
        weak: bool,
    },
}

#[derive(Debug)]
pub struct JumpRelocation {
    addend: i64,
    symbol_index: usize,
    source_address: u64,
}

#[derive(Debug)]
pub struct RelativeRelocation {
    addend: i64,
    source_address: u64,
}

#[derive(Debug)]
pub struct SharedLibraryAnalysis {
    pub jump_relocations: Vec<JumpRelocation>,
    pub relative_relocations: Vec<RelativeRelocation>,
    pub library_versions: Vec<LibraryVersion>,
    pub soname: Option<String>,
    pub symbols: Vec<Symbol>,
}

pub fn analyze_shared_library(
    elf_file: ElfFile,
) -> Result<SharedLibraryAnalysis, Box<dyn std::error::Error>> {
    let dynamic_table = elf_file.dynamic()?.unwrap();
    let (dynamic_symbol_table, dynamic_symbol_string_table) =
        elf_file.dynamic_symbol_table()?.unwrap();

    let imported_library_names = dynamic_table
        .iter()
        .filter(|entry| entry.d_tag == elf::abi::DT_NEEDED)
        .map(|entry| dynamic_symbol_string_table.get(entry.d_val() as usize))
        .collect::<Result<Vec<_>, _>>()?;

    let versym_section = elf_file.section_headers().and_then(|sections| {
        sections
            .iter()
            .find(|section| section.sh_type == elf::abi::SHT_GNU_VERSYM)
    });

    let mut library_versions = Vec::<LibraryVersion>::new();
    let mut library_versions_map = HashMap::<u16, usize>::new();
    let mut self_soname = None;

    if let Some(verdef_section) = elf_file.section_headers().and_then(|sections| {
        sections
            .iter()
            .find(|section| section.sh_type == elf::abi::SHT_GNU_VERDEF)
    }) {
        let (verdef_data, _) = elf_file.section_data(&verdef_section)?;
        let linked_strtab_header = elf_file
            .section_headers()
            .unwrap()
            .get(verdef_section.sh_link as usize)?;
        let verdef_strings = elf_file.section_data_as_strtab(&linked_strtab_header)?;

        for (verdef, verdaux_iter) in elf::gnu_symver::VerDefIterator::new(
            elf_file.ehdr.endianness,
            elf_file.ehdr.class,
            verdef_section.sh_info as u64,
            0,
            verdef_data,
        ) {
            let mut names = verdaux_iter.map(|verdaux| {
                verdef_strings
                    .get(verdaux.vda_name as usize)
                    .unwrap()
                    .to_string()
            });

            let first_name = names.next().unwrap();

            if verdef.vd_flags & elf::abi::VER_FLG_BASE != 0 {
                self_soname = Some(first_name);
            } else {
                library_versions_map.insert(verdef.vd_ndx, library_versions.len());

                library_versions.push(LibraryVersion::Exported {
                    dependencies: names.collect(),
                    version: first_name,
                });
            }
        }
    }

    if let Some(verneed_section) = elf_file.section_headers().and_then(|sections| {
        sections
            .iter()
            .find(|section| section.sh_type == elf::abi::SHT_GNU_VERNEED)
    }) {
        let (verneed_data, _) = elf_file.section_data(&verneed_section)?;
        let linked_strtab_header = elf_file
            .section_headers()
            .unwrap()
            .get(verneed_section.sh_link as usize)?;
        let verneed_strings = elf_file.section_data_as_strtab(&linked_strtab_header)?;

        for (verneed, vernaux_iter) in elf::gnu_symver::VerNeedIterator::new(
            elf_file.ehdr.endianness,
            elf_file.ehdr.class,
            verneed_section.sh_info as u64,
            0,
            verneed_data,
        ) {
            for (relative_index, vernaux) in vernaux_iter.enumerate() {
                library_versions_map.insert(vernaux.vna_other, library_versions.len());

                library_versions.push(LibraryVersion::Imported {
                    soname: verneed_strings
                        .get(verneed.vn_file as usize)
                        .unwrap()
                        .to_string(),
                    version: verneed_strings
                        .get(vernaux.vna_name as usize)
                        .unwrap()
                        .to_string(),
                });
            }
        }
    }

    let symbols = versym_section
        .map(
            |versym_section| -> Result<Vec<Symbol>, Box<dyn std::error::Error>> {
                let (versym_data, _) = elf_file.section_data(&versym_section)?;
                let versym_table = elf::gnu_symver::VersionIndexTable::new(
                    elf_file.ehdr.endianness,
                    elf_file.ehdr.class,
                    versym_data,
                );

                Ok(dynamic_symbol_table
                    .iter()
                    .zip(versym_table.iter())
                    .enumerate()
                    .filter_map(|(dynamic_symbol_index, (dynamic_symbol, version_index))| {
                        if
                        /* dynamic_symbol.st_symtype() == elf::abi::STT_NOTYPE || */
                        dynamic_symbol.st_symtype() == elf::abi::STT_SECTION
                            || dynamic_symbol.st_shndx == elf::abi::SHN_ABS
                        {
                            return None;
                        }

                        let symbol_name = dynamic_symbol_string_table
                            .get(dynamic_symbol.st_name as usize)
                            .unwrap_or("<unknown>")
                            .to_string();

                        let address = (dynamic_symbol.st_shndx != elf::abi::SHN_UNDEF)
                            .then_some(dynamic_symbol.st_value);

                        let raw_version_index = version_index.index();
                        let version_index = (raw_version_index != elf::abi::VER_NDX_LOCAL
                            && raw_version_index != elf::abi::VER_NDX_GLOBAL)
                            .then_some(raw_version_index as usize);

                        let kind = if let Some(address) = address {
                            SymbolKind::Exported {
                                address,
                                version_index,
                            }
                        } else {
                            SymbolKind::Imported {
                                version_index,
                                weak: dynamic_symbol.st_bind() == elf::abi::STB_WEAK,
                            }
                        };

                        Some(Symbol {
                            dynamic_symbol_index,
                            name: symbol_name,
                            kind,
                        })
                    })
                    .collect::<Vec<_>>())
            },
        )
        .transpose()?
        .unwrap_or_default();

    let string_table = elf_file.section_headers_with_strtab()?.1.unwrap();

    let plt_section = elf_file
        .section_headers()
        .and_then(|sections| {
            sections.iter().find(|section| {
                section.sh_type == elf::abi::SHT_PROGBITS
                    && string_table.get(section.sh_name as usize).unwrap_or("") == ".plt"
            })
        })
        .unwrap();

    let mut jump_relocations = Vec::<JumpRelocation>::new();
    let mut relative_relocations = Vec::<RelativeRelocation>::new();

    if let Some(sections) = elf_file.section_headers() {
        for section in sections.iter() {
            if section.sh_type == elf::abi::SHT_RELA {
                // println!("Section name: {:?}", string_table.get(section.sh_name as usize));
                // continue;

                if let Ok(relas) = elf_file.section_data_as_relas(&section) {
                    for (rela_index, rela) in relas.enumerate() {
                        match rela.r_type {
                            elf::abi::R_AARCH64_JUMP_SLOT => {
                                let address = plt_section.sh_addr + (rela_index as u64 + 2) * 4;
                                let symbol_index = symbols
                                    .iter()
                                    .position(|symbol| {
                                        symbol.dynamic_symbol_index == rela.r_sym as usize
                                    })
                                    .unwrap();
                                eprintln!("{:?}", symbols[symbol_index]);

                                jump_relocations.push(JumpRelocation {
                                    addend: rela.r_addend,
                                    symbol_index,
                                    source_address: address,
                                });
                            }
                            // elf::abi::R_AARCH64_GLOB_DAT => {
                            //     let symbol_index = symbols
                            //         .iter()
                            //         .position(|symbol| {
                            //             symbol.dynamic_symbol_index == rela.r_sym as usize
                            //         })
                            //         .unwrap();
                            //     eprintln!("{:?}", symbols[symbol_index]);

                            //     jump_relocations.push(JumpRelocation {
                            //         addend: rela.r_addend,
                            //         symbol_index,
                            //         source_address: rela.r_offset,
                            //     });
                            // }
                            elf::abi::R_AARCH64_RELATIVE => {
                                relative_relocations.push(RelativeRelocation {
                                    addend: rela.r_addend,
                                    source_address: rela.r_offset,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    Ok(SharedLibraryAnalysis {
        jump_relocations,
        relative_relocations,
        library_versions,
        soname: self_soname,
        symbols,
    })
}
