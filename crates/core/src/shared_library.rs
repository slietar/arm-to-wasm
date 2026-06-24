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
pub struct NeededLibrary {
    name: String,
    versions: Vec<String>,
}

#[derive(Debug)]
pub struct Symbol {
    pub name: String,
    pub variant: SymbolVariant,
}

#[derive(Debug)]
pub struct SharedLibraryAnalysis {
    pub needed_libraries: Vec<NeededLibrary>,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug)]
pub enum SymbolVariant {
    Local,
    Global,
    Versioned(VersionReference),
}

#[derive(Debug, Clone)]
pub struct VersionReference {
    library_index: usize,
    relative_index: usize,
}

pub fn analyze_shared_library(
    elf_file: ElfFile,
) -> Result<SharedLibraryAnalysis, Box<dyn std::error::Error>> {
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

    let mut needed_libraries = Vec::<NeededLibrary>::new();
    let mut vernaux_to_needed_library_index = HashMap::<u16, VersionReference>::new();

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
            let needed_library_index = needed_libraries.len();
            let mut versions = Vec::<String>::new();

            for (relative_index, vernaux) in vernaux_iter.enumerate() {
                let version = verneed_strings
                    .get(vernaux.vna_name as usize)
                    .unwrap_or("<unknown>")
                    .to_string();
                versions.push(version);

                vernaux_to_needed_library_index.insert(
                    vernaux.vna_other,
                    VersionReference {
                        library_index: needed_library_index,
                        relative_index,
                    },
                );
            }

            needed_libraries.push(NeededLibrary {
                name: verneed_strings
                    .get(verneed.vn_file as usize)
                    .unwrap_or("<unknown>")
                    .to_string(),
                versions,
            });
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
                    .filter_map(|(symbol, version_index)| {
                        if symbol.st_symtype() == elf::abi::STT_NOTYPE
                            || symbol.st_symtype() == elf::abi::STT_SECTION
                            || symbol.st_shndx == elf::abi::SHN_ABS
                        {
                            return None;
                        }

                        let symbol_name = dynamic_symbol_string_table
                            .get(symbol.st_name as usize)
                            .unwrap_or("<unknown>")
                            .to_string();

                        let variant = if version_index.is_local() {
                            SymbolVariant::Local
                        } else if version_index.is_global() {
                            SymbolVariant::Global
                        } else if let Some(version_reference) =
                            vernaux_to_needed_library_index.get(&version_index.index())
                        {
                            SymbolVariant::Versioned(version_reference.clone())
                        } else {
                            SymbolVariant::Global
                        };

                        Some(Symbol {
                            name: symbol_name,
                            variant,
                        })
                    })
                    .collect::<Vec<_>>())
            },
        )
        .transpose()?
        .unwrap_or_default();

    Ok(SharedLibraryAnalysis {
        needed_libraries,
        symbols,
    })
}
