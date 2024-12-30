use std::borrow::Cow;
use binaryen::ffi as by;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::io::{Cursor, Read, Write};
use std::fs::File;
use disarm64::decoder::{self, Operation};
use mach_object::{LoadCommand, MachCommand, OFile, SectionAttributes};


// p. 322
// Wn => 0-30, 32 bits
// Xn => 0-30, 64 bits
// WSP, WZR => 31, 32 bits
// SP, XZR => 31, 64 bits
// #[derive(Debug)]
// struct Register(u32);


#[derive(Debug)]
struct CompilationError(Cow<'static, str>);

impl std::error::Error for CompilationError {
    fn description(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "compilation error: {}", self.0)
    }
}


fn main() {
    unsafe {
        run().unwrap();
    }
}


unsafe fn run() -> Result<(), CompilationError> {
    // Load input Mach-O file

    // let mut file = File::open("/bin/sh").unwrap();
    // let mut file = File::open("test/molcv").unwrap();
    // let mut file = File::open("/opt/homebrew/lib/python3.11/site-packages/numpy/random/_bounded_integers.cpython-311-darwin.so").unwrap();
    // let mut file = File::open("simple-lib/target/debug/simple-lib").unwrap();
    let mut file = File::open("test/hello").unwrap();

    let mut buffer = Vec::new();
    let size = file.read_to_end(&mut buffer).unwrap();
    let mut cursor = Cursor::new(&buffer[..size]);

    let ofile = OFile::parse(&mut cursor).unwrap();

    let (_header, commands) = if let OFile::MachFile { header, commands } = ofile {
        (header, commands)
    } else {
        return Err(CompilationError("not a Mach-O file".into()));
    };


    // Find entry point

    let entry_addr = commands
        .iter()
        .find_map(|cmd| {
            if let MachCommand(LoadCommand::EntryPoint { entryoff, .. }, _) = cmd {
                Some(*entryoff)
            } else {
                None
            }
        })
        // .unwrap_or(0);
        .ok_or(CompilationError("no entry point".into()))?;

    // eprintln!("{:?}", entry_command);


    // Find all symbols

    const SYMBOL_TABLE_ENTRY_SIZE: u32 = 16;

    let mut symbols = HashMap::new();

    for MachCommand(cmd, _cmdsize) in &commands {
        if let LoadCommand::SymTab { symoff, nsyms, stroff, .. } = cmd {
            symbols.reserve(*nsyms as usize);

            for entry_index in 0..(*nsyms) {
                let offset = (symoff + entry_index * SYMBOL_TABLE_ENTRY_SIZE) as usize;
                let str_offset = (stroff + u32::from_le_bytes(buffer[offset..(offset + 4)].try_into().unwrap())) as usize;
                let str_length = buffer[str_offset..].iter().position(|&r| r == b'\0').unwrap();
                let name = String::from_utf8(buffer[str_offset..(str_offset + str_length)].to_vec()).unwrap();

                let addr = u64::from_le_bytes(buffer[(offset + 8)..(offset + 16)].try_into().unwrap());
                symbols.insert(name, addr);
            }
        }
    }

    // return Ok(());

    let symbol_addrs = symbols.values().copied().collect::<HashSet<_>>();
    // eprintln!("{:?}", symbol_addrs);


    // Find block address ranges

    const INSTRUCTION_SIZE: usize = 4;

    let mut block_addr_ranges = Vec::new();
    let mut exec_sections = Vec::new();

    #[derive(Debug)]
    struct ExecutableSection<'a> {
        addr: u64,
        buffer: &'a [u8],
    }


    for &MachCommand(ref cmd, _cmdsize) in &commands {
        // eprintln!("{:#?}", cmd);
        // continue;

        if let LoadCommand::Segment64 { maxprot, sections, .. } = cmd {
            // eprintln!("segment: {}", segname);
            // eprintln!("  ({} -> {})", vmaddr, vmsize);
            // eprintln!("  {:?}", flags);

            // for sect in sections {
            //     eprintln!("  section: {}", sect.sectname);
            //     eprintln!("    {}, {}", sect.addr, sect.size);
            //     eprintln!("    {}", sect.offset);
            //     // eprintln!("    {:0>32b}", <SectionFlags as Into<u32>>::into(sect.flags));
            //     eprintln!("    {:?}", sect.flags.sect_attrs());
            //     eprintln!("    {:?}", sect.flags.sect_type());
            // }

            // continue;

            // Skip if no execute permission
            if maxprot & 0x1 == 0 {
                continue;
            }

            for section in sections {
                let attributes = section.flags.sect_attrs();

                if !attributes.contains(SectionAttributes::S_ATTR_SOME_INSTRUCTIONS) {
                    continue;
                }

                // let mut start_addr = section.addr as u64;
                // entries.insert(section.addr as u64);

                let section_index = exec_sections.len();

                exec_sections.push(ExecutableSection {
                    addr: section.addr as u64,
                    buffer: &buffer[(section.offset as usize)..(section.offset as usize + section.size)],
                });

                let instruction_count = section.size / INSTRUCTION_SIZE;

                let mut jump_instr_indices = HashSet::new();
                let mut end_instr_index = instruction_count;

                for instr_index in 0..instruction_count {
                    let instruction_addr = (section.addr as u64) + (instr_index * INSTRUCTION_SIZE) as u64;

                    let offset = (section.offset as usize + instr_index * INSTRUCTION_SIZE) as usize;
                    let instruction_encoded = u32::from_le_bytes(buffer[offset..(offset + INSTRUCTION_SIZE)].try_into().unwrap());
                    let instruction = match disarm64::decoder::decode(instruction_encoded) {
                        Some(instruction) => instruction,
                        None => {
                            end_instr_index = instr_index;
                            break;
                        },
                    };

                    if symbol_addrs.contains(&instruction_addr) {
                        jump_instr_indices.insert(instr_index);
                        continue;
                    }

                    // use disarm64_defn::defn::InsnOpcode;
                    // let def = instruction.definition();
                    // eprintln!("{:?}", def);
                    // eprintln!("{instruction:?}");

                    match instruction.operation {
                        // Operation::LDST_POS(decoder::LDST_POS::LDR_Rt_ADDR_UIMM12(inst)) => {
                        //     eprintln!("{instruction}");
                        //     eprintln!(">> {:?}", inst);
                        // },
                        Operation::BRANCH_IMM(decoder::BRANCH_IMM::B_ADDR_PCREL26(inst)) => {
                            jump_instr_indices.insert(instr_index + (inst.imm26() as usize));

                            // eprintln!("{instruction} {:x} {:x}", current_addr, current_addr + (target << 2));
                            // entries.insert(target);
                        },
                        Operation::CONDBRANCH(decoder::CONDBRANCH::B__ADDR_PCREL19(inst)) => {
                            jump_instr_indices.insert(instr_index + (inst.imm19() as usize));
                            // eprintln!("insert {:x?}", instruction_addr + (addr_offset << 2));
                        },
                        // Operation::BRANCH_REG(decoder::BRANCH_REG::RET_Rn(inst)) => {
                        //     let _reg = inst.rn();

                        //     // eprintln!(">> {:?}", reg);
                        //     // let target = instruction.operands[0].value.unwrap();
                        //     // entries.insert(target);
                        // },
                        _ => {},
                    }
                }

                let mut jump_instr_indices_vec = jump_instr_indices.iter().copied().collect::<Vec<_>>();

                jump_instr_indices_vec.push(end_instr_index);
                jump_instr_indices_vec.sort();

                for range_addrs in jump_instr_indices_vec.windows(2) {
                    block_addr_ranges.push((section_index, range_addrs[0], range_addrs[1]));
                }

                // eprintln!("{:?}", jump_instr_indices_vec);
            }
        }
    }

    // eprintln!("{:?}", symbols);
    // eprintln!("{:x?}", block_addr_ranges);
    // return Ok(());

    // let entries = HashSet::<u64>::new();


    // for entry in &entries {
    //     eprintln!("{:x?}", entry);
    // }

    // let mut p = entries.iter().collect::<Vec<_>>();
    // p.sort();
    // let p = p.windows(2).map(|v| v[1] - v[0]).collect::<Vec<_>>();
    // eprintln!("{:?}", p);


    // Create module

    let module = by::BinaryenModuleCreate();

    by::BinaryenModuleSetFeatures(module, by::BinaryenFeatureMemory64());


    // Initialize memory

    let mut segment_names = Vec::new();
    let mut segment_datas = Vec::new();
    let mut segment_passives = Vec::new();
    let mut segment_offsets = Vec::new();
    // let mut segment_sizes = [by::BinaryenLiteralInt32(4)];
    let mut segment_sizes = Vec::new();

    let mut mem_size = 0;


    for &MachCommand(ref cmd, _cmdsize) in &commands {
        match cmd {
            LoadCommand::Segment64 { ref sections, segname, .. } => {
                // eprintln!("segment: {}", segname);
                // eprintln!("  ({} -> {})", vmaddr, vmsize);
                // eprintln!("  {:?}", flags);

                // TODO: Update mem_size here instead

                for sect in sections {
                    if segment_names.len() >= 2 {
                        // break;
                    }

                    // if sect.offset == 7816 { continue; }

                    if sect.offset == 0 {
                        continue;
                    }

                    // segment_names.push(CString::new(format!("a{}", segment_names.len())).unwrap());
                    segment_names.push(CString::new(format!("{segname}/{}", sect.sectname)).unwrap());

                    // eprintln!("{:?}", sect.offset);
                    // eprintln!("{:?}", buffer.as_mut_ptr().add(sect.offset as usize));

                    // segment_datas.push(&buffer[(sect.offset as usize)..(sect.offset as usize + sect.size)].as_mut_ptr());
                    segment_datas.push(buffer.as_ptr().offset(sect.offset as isize)); //.add(sect.offset as usize));
                    // segment_datas.push(0x140009e88 as *mut *const i8);

                    segment_passives.push(false);
                    segment_offsets.push(by::BinaryenConst(module, by::BinaryenLiteralInt64(sect.addr as i64)));
                    segment_sizes.push(sect.size as u32);
                    // segment_sizes.push(1);

                    mem_size = mem_size.max(sect.addr + sect.size);

                    // eprintln!("  section: {}", sect.sectname);
                    // eprintln!("    {}, {}", sect.addr, sect.size);
                    // eprintln!("    {}", sect.offset);
                    // // eprintln!("    {:0>32b}", <SectionFlags as Into<u32>>::into(sect.flags));
                    // eprintln!("    {:?}", sect.flags.sect_attrs());
                    // eprintln!("    {:?}", sect.flags.sect_type());
                }

                // for section in sections { }
            },
            _ => {},
        }
    }

    let special_mem_addr = (mem_size.div_ceil(PAGE_SIZE) * PAGE_SIZE) as i64;


    const PAGE_SIZE: usize = 65_536;

    let mem_name_internal = CString::new("emul_mem").unwrap();
    let mem_name_exported = CString::new("memory").unwrap();

    by::BinaryenSetMemory(
        module,
        mem_size.div_ceil(PAGE_SIZE) as u32 + 1, // Reserve 1 page for special use
        std::mem::transmute(-1),
        mem_name_exported.as_ptr(),
        segment_names.iter().map(|name| name.as_ptr()).collect::<Vec<_>>().as_mut_ptr(),
        segment_datas.as_mut_ptr() as *mut *const i8,
        segment_passives.as_mut_ptr(),
        segment_offsets.as_mut_ptr(),
        segment_sizes.as_mut_ptr(),
        segment_names.len() as u32,
        false,
        true,
        // mem_name.as_ptr(),
        mem_name_internal.as_ptr(),
    );

    // For WASI
    // by::BinaryenAddMemoryExport(module, mem_name.as_ptr(), emul_mem_name.as_ptr());


    // Import WASI functions

    let wasi_prefix = CString::new("wasi_snapshot_preview1").unwrap();
    let wasi_filesystem_write_name = CString::new("fd_write").unwrap();

    let mut wasi_filesystem_write_params = [
        by::BinaryenTypeInt32(),
        by::BinaryenTypeInt32(),
        by::BinaryenTypeInt32(),
        by::BinaryenTypeInt32(),
    ];

    // by::BinaryenAddFunctionImport(module, internalName, externalModuleName, externalBaseName, params, results);
    by::BinaryenAddFunctionImport(
        module,
        wasi_filesystem_write_name.as_ptr(),
        wasi_prefix.as_ptr(),
        wasi_filesystem_write_name.as_ptr(),
        by::BinaryenTypeCreate(wasi_filesystem_write_params.as_mut_ptr(), wasi_filesystem_write_params.len() as u32),
        by::BinaryenTypeInt32(),
    );


    // Translate instructions

    let loop_name = CString::new("loop").unwrap();
    let loop_body_name = CString::new("body").unwrap();

    let mut branches = Vec::new();
    // let register_vars = (0..32).map(|reg_index| {
    //     by::BinaryenLocalGet(module, reg_index + 1, by::BinaryenInt64())
    // });

    const REGISTER_COUNT: usize = 32;
    let get_reg_local_index = |reg_index: u32| reg_index + 1;

    for (section_index, instr_index_start, instr_index_end) in &block_addr_ranges {
        let section = &exec_sections[*section_index];
        let instr_count = *instr_index_end - *instr_index_start;
        let range_start_addr = (section.addr as u64) + (*instr_index_start * INSTRUCTION_SIZE) as u64;

        let mut commands = Vec::new();

        for instr_index in (*instr_index_start)..(*instr_index_end) {
            let instr_addr = (section.addr as u64) + (instr_index * INSTRUCTION_SIZE) as u64;
            let instr_encoded = u32::from_le_bytes(section.buffer[(instr_index * INSTRUCTION_SIZE)..((instr_index + 1) * INSTRUCTION_SIZE)].try_into().unwrap());
            let instruction = disarm64::decoder::decode(instr_encoded).unwrap();

            // eprintln!("{:?}", instruction);

            match instruction.operation {
                Operation::MOVEWIDE(decoder::MOVEWIDE::MOVZ_Rd_HALF(inst)) => {
                    commands.push(
                        by::BinaryenLocalSet(
                            module,
                            get_reg_local_index(inst.rd()),
                            by::BinaryenConst(module, by::BinaryenLiteralInt64(inst.imm16_5() as i64)),
                        ),
                    );
                },
                Operation::PCRELADDR(decoder::PCRELADDR::ADR_Rd_ADDR_PCREL21(inst)) => {
                    let imm = sign_extend((inst.immhi() << 2) + inst.immlo(), 21);

                    commands.push(
                        by::BinaryenLocalSet(
                            module,
                            get_reg_local_index(inst.rd()),
                            by::BinaryenConst(module, by::BinaryenLiteralInt64(instr_addr as i64 + imm as i64)),
                        ),
                    );
                },
                Operation::EXCEPTION(decoder::EXCEPTION::SVC_EXCEPTION(_inst)) => {
                    let base_addr = special_mem_addr;
                    let len_addr = special_mem_addr + 4;
                    let ret_addr = special_mem_addr + 8;

                    let mut operands = [
                        by::BinaryenUnary(module, by::BinaryenWrapInt64(), by::BinaryenLocalGet(module, get_reg_local_index(0), by::BinaryenInt64())),
                        by::BinaryenConst(module, by::BinaryenLiteralInt32(base_addr as i32)),
                        by::BinaryenConst(module, by::BinaryenLiteralInt32(1)),
                        by::BinaryenConst(module, by::BinaryenLiteralInt32(ret_addr as i32)),
                    ];

                    let mut children = [
                        // by::BinaryenStore(module, bytes, offset, align, ptr, value, type_, memoryName)
                        by::BinaryenStore(
                            module,
                            4,
                            0,
                            0,
                            by::BinaryenConst(module, by::BinaryenLiteralInt64(base_addr)),
                            by::BinaryenUnary(module, by::BinaryenWrapInt64(), by::BinaryenLocalGet(module, get_reg_local_index(1), by::BinaryenInt64())),
                            by::BinaryenInt32(),
                            mem_name_internal.as_ptr(),
                        ),
                        by::BinaryenStore(
                            module,
                            4,
                            0,
                            0,
                            by::BinaryenConst(module, by::BinaryenLiteralInt64(len_addr)),
                            by::BinaryenUnary(module, by::BinaryenWrapInt64(), by::BinaryenLocalGet(module, get_reg_local_index(2), by::BinaryenInt64())),
                            by::BinaryenInt32(),
                            mem_name_internal.as_ptr(),
                        ),
                        by::BinaryenDrop(
                            module,
                            by::BinaryenCall(
                                module,
                                wasi_filesystem_write_name.as_ptr(),
                                operands.as_mut_ptr(),
                                operands.len() as u32,
                                by::BinaryenInt32(),
                            ),
                        ),
                        by::BinaryenLocalSet(
                            module,
                            get_reg_local_index(0),
                            by::BinaryenUnary(
                                module,
                                by::BinaryenExtendUInt32(),
                                by::BinaryenLoad(
                                    module,
                                    4,
                                    false,
                                    0,
                                    0,
                                    by::BinaryenInt32(),
                                    by::BinaryenConst(module, by::BinaryenLiteralInt64(ret_addr)),
                                    mem_name_internal.as_ptr()
                                ),
                            ),
                        ),
                    ];

                    let ptr = children.as_mut_ptr();
                    let len = children.len() as u32;

                    commands.push(
                        by::BinaryenIf(
                            module,
                            by::BinaryenBinary(
                                module,
                                by::BinaryenEqInt64(),
                                by::BinaryenLocalGet(module, get_reg_local_index(16), by::BinaryenTypeInt64()),
                                by::BinaryenConst(module, by::BinaryenLiteralInt64(0x1)),
                            ),
                            by::BinaryenReturn(module, std::ptr::null_mut()),
                            by::BinaryenNop(module),
                        ),
                    );

                    commands.push(
                        by::BinaryenIf(
                            module,
                            by::BinaryenBinary(
                                module,
                                by::BinaryenEqInt64(),
                                by::BinaryenLocalGet(module, get_reg_local_index(16), by::BinaryenTypeInt64()),
                                by::BinaryenConst(module, by::BinaryenLiteralInt64(0x4)),
                            ),
                            by::BinaryenBlock(
                                module,
                                std::ptr::null_mut(),
                                ptr,
                                len,
                                by::BinaryenTypeNone(),
                            ),
                            by::BinaryenNop(module),
                        ),
                    );
                },
                _ => {
                    commands.push(
                        by::BinaryenUnreachable(module),
                    );
                },
            }
        }

        commands.push(
            by::BinaryenLocalSet(
                module,
                0,
                by::BinaryenConst(module, by::BinaryenLiteralInt64((section.addr as i64) + (instr_count * INSTRUCTION_SIZE) as i64)),
            ),
        );

        commands.push(
            by::BinaryenBreak(
                module,
                loop_name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ),
        );

        branches.push(
            by::BinaryenIf(
                module,
                by::BinaryenBinary(
                    module,
                    by::BinaryenEqInt64(),
                    by::BinaryenLocalGet(module, 0, by::BinaryenTypeInt64()),
                    by::BinaryenConst(module, by::BinaryenLiteralInt64(range_start_addr as i64)),
                ),
                by::BinaryenBlock(
                    module,
                    std::ptr::null_mut(),
                    commands.as_mut_ptr(),
                    commands.len() as u32, by::BinaryenTypeNone()
                ),
                by::BinaryenNop(module),
            ),
        );
    }


    // branches.push(
    //     by::BinaryenBreak(
    //         module,
    //         loop_name.as_ptr(),
    //         std::ptr::null_mut(),
    //         std::ptr::null_mut(),
    //     ),
    // );

    // let current_pointer_expr = by::BinaryenLocalGet(module, 0, by::BinaryenTypeInt64());

    let loop_ = by::BinaryenLoop(module, loop_name.as_ptr(),
        by::BinaryenBlock(module, loop_body_name.as_ptr(),
            branches.as_mut_ptr(), branches.len() as u32, by::BinaryenTypeNone()
        )
    );


    // Create functions

    let mut var_types = vec![by::BinaryenTypeInt64()];
    var_types.extend((0..REGISTER_COUNT).map(|_| by::BinaryenTypeInt64()));

    let main_func_name = CString::new("_main").unwrap();
    let _main_func = by::BinaryenAddFunction(
        module,
        main_func_name.as_ptr(),
        by::BinaryenTypeInt64(),
        by::BinaryenTypeNone(),
        var_types.as_mut_ptr(),
        var_types.len() as u32,
        loop_,
    );


    let entry_func_body = by::BinaryenCall(
        module,
        main_func_name.as_ptr(),
        [by::BinaryenConst(module, by::BinaryenLiteralInt64(std::mem::transmute(0x100000 + entry_addr)))].as_mut_ptr(),
        1,
        by::BinaryenTypeNone(),
    );

    let entry_func_name = CString::new("_entry").unwrap();

    let _entry_func = by::BinaryenAddFunction(
        module,
        entry_func_name.as_ptr(),
        by::BinaryenTypeNone(),
        by::BinaryenTypeNone(),
        [].as_mut_ptr(),
        0,
        entry_func_body,
    );

    let _export = by::BinaryenAddExport(module, entry_func_name.as_ptr(), entry_func_name.as_ptr());


    // Emit WASM binary

    by::BinaryenModulePrint(module);
    by::BinaryenModuleValidate(module);

    by::BinaryenModuleOptimize(module);
    by::BinaryenModulePrint(module);

    let mut output = vec![0u8; 10_000_000];
    let written = by::BinaryenModuleWrite(module, output.as_mut_ptr() as *mut i8, output.len());
    assert!(written <= output.len());

    // Clean up the module, which owns all the objects we created above
    by::BinaryenModuleDispose(module);


    let mut output_file = File::create("output.wasm").unwrap();
    output_file.write_all(&output[..written]).unwrap();


    Ok(())
}


fn sign_extend(value: u32, bit_count: u32) -> i32 {
    let shift = 32 - bit_count;
    (value << shift) as i32 >> shift
    // (!offset & ((1 << 20) - 1)) + 1)
}
