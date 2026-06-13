use std::ffi::CString;

use binaryen::ffi as by;
use elf::ElfBytes;

use crate::{constants::PAGE_SIZE, module::Module};

pub fn translate(elf_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let analysis = crate::analysis::analyze(elf_bytes)?;

    // eprintln!("Analysis result: {:#?}", analysis);
    let mut module = Module::new();
    let relooper = module.relooper();

    let routine_names = analysis
        .routines
        .iter()
        .enumerate()
        .map(|(routine_index, routine)| format!("routine_{}", routine_index))
        .collect::<Vec<_>>();

    for (routine_index, routine) in analysis.routines.iter().enumerate() {
        let routine_name = &routine_names[routine_index];

        let func = module.function(
            routine_name,
            &[],
            module.none(),
            &[],
            module.nop(),
        );
    }


    Ok(())
}


#[derive(Debug)]
struct MappedSegment<'a> {
    address: u64,
    data: &'a [u8],
    memory_offset: u64,
    size: u64,
    writable: bool,
}

pub fn set_memory(
    memory_name: &str,
    elf_bytes: &[u8],
    elf_file: ElfBytes<elf::endian::AnyEndian>,
    module: &Module,
) {
    let mut current_offset = 0;
    let mut mapped_segments = Vec::new();

    for segment in elf_file.segments().unwrap() {
        // eprintln!("Segment: {:?}", segment);

        if segment.p_type == elf::abi::PT_LOAD {
            // let is_executable = (segment.p_flags & elf::abi::PF_X) != 0;
            // if is_executable {
            //     eprintln!("Found executable segment at 0x{:x} with {} bytes", segment.p_offset, segment.p_filesz);
            // }

            // eprintln!("{} {}", segment.p_filesz, file_data[(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize].len());

            mapped_segments.push(MappedSegment {
                address: segment.p_vaddr,
                data: &elf_bytes
                    [(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize],
                memory_offset: current_offset,
                size: segment.p_filesz,
                writable: (segment.p_flags & elf::abi::PF_W) != 0,
            });

            // eprintln!("{:?}", mapped_segments.last().unwrap().data);

            current_offset += segment.p_filesz;
        }
    }

    let total_mapped_size = current_offset;

    let segment_names = mapped_segments
        .iter()
        .enumerate()
        .map(|(i, _)| CString::new(format!("segment_{}", i)).unwrap())
        .collect::<Vec<_>>();

    let mut segment_name_ptrs = segment_names.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();

    let mut segment_datas = mapped_segments
        .iter()
        .map(|seg| seg.data.as_ptr())
        .collect::<Vec<_>>();

    let mut segment_passives = vec![false; mapped_segments.len()];

    let mut segment_offsets = mapped_segments
        .iter()
        .map(|seg| unsafe {
            by::BinaryenConst(
                module.by_module,
                by::BinaryenLiteralInt64(seg.memory_offset as i64),
            )
        })
        .collect::<Vec<_>>();

    let mut segment_sizes = mapped_segments
        .iter()
        .map(|seg| seg.size as u32)
        .collect::<Vec<_>>();

    let mapped_memory_page_count = total_mapped_size.div_ceil(PAGE_SIZE);
    let mapped_memory_size = mapped_memory_page_count * PAGE_SIZE;
    let stack_memory_page_count = 2;
    let stack_memory_internal_address = mapped_memory_size;

    let mapped_memory_name = CString::new(memory_name).unwrap();

    unsafe {
        by::BinaryenSetMemory(
            module.by_module,
            (mapped_memory_page_count + stack_memory_page_count) as u32,
            i32::cast_unsigned(-1),
            mapped_memory_name.as_ptr(),
            segment_name_ptrs.as_mut_ptr() as *mut *const i8,
            segment_datas.as_mut_ptr() as *mut *const i8,
            segment_passives.as_mut_ptr(),
            segment_offsets.as_mut_ptr(),
            segment_sizes.as_mut_ptr(),
            mapped_segments.len() as u32,
            false,
            true,
            mapped_memory_name.as_ptr(),
        );
    }
}
