use std::borrow::Cow;
use binaryen::ffi as by;
use std::collections::HashSet;
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

    let entry_addr = commands
        .iter()
        .find_map(|cmd| {
            if let MachCommand(LoadCommand::EntryPoint { entryoff, .. }, _) = cmd {
                Some(*entryoff)
            } else {
                None
            }
        })
        .ok_or(CompilationError("no entry point".into()))?;

    // eprintln!("{:?}", entry_command);

    const INSTRUCTION_SIZE: usize = 4;

    let mut entries = HashSet::new();

    for &MachCommand(ref cmd, _cmdsize) in &commands {
        match cmd {
            LoadCommand::Segment64 { ref sections, .. } => {
                // eprintln!("segment: {}", segname);
                // eprintln!("  ({} -> {})", vmaddr, vmsize);
                // eprintln!("  {:?}", flags);

                // for ref sect in sections {
                //     eprintln!("  section: {}", sect.sectname);
                //     eprintln!("    {}, {}", sect.addr, sect.size);
                //     eprintln!("    {}", sect.offset);
                //     // eprintln!("    {:0>32b}", <SectionFlags as Into<u32>>::into(sect.flags));
                //     eprintln!("    {:?}", sect.flags.sect_attrs());
                //     eprintln!("    {:?}", sect.flags.sect_type());
                // }

                for section in sections {
                    let attributes = section.flags.sect_attrs();

                    if attributes.contains(SectionAttributes::S_ATTR_PURE_INSTRUCTIONS) {
                        entries.insert(section.addr as u64);

                        // let mut register_values = [Option::<u64>::None; 32];

                        for instruction_index in 0..(section.size / INSTRUCTION_SIZE) {
                            let current_addr = (section.addr as u64) + (instruction_index * INSTRUCTION_SIZE) as u64;
                            let offset = (section.offset as usize + instruction_index * INSTRUCTION_SIZE) as usize;
                            let instruction_encoded = u32::from_le_bytes(buffer[offset..(offset + 4)].try_into().unwrap());
                            let instruction = match disarm64::decoder::decode(instruction_encoded) {
                                Some(instruction) => instruction,
                                None => continue,
                            };

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
                                    let addr_offset = inst.imm26() as u64;
                                    entries.insert(current_addr + (addr_offset << 2));

                                    // eprintln!("{instruction} {:x} {:x}", current_addr, current_addr + (target << 2));
                                    // entries.insert(target);
                                },
                                Operation::CONDBRANCH(decoder::CONDBRANCH::B__ADDR_PCREL19(inst)) => {
                                    let addr_offset = inst.imm19() as u64;
                                    entries.insert(current_addr + (addr_offset << 2));
                                },
                                Operation::BRANCH_REG(decoder::BRANCH_REG::RET_Rn(inst)) => {
                                    let _reg = inst.rn();

                                    // eprintln!(">> {:?}", reg);
                                    // let target = instruction.operands[0].value.unwrap();
                                    // entries.insert(target);
                                },
                                _ => {},
                            }
                        }
                    }
                }
            },
            LoadCommand::SymTab { symoff, nsyms, stroff, .. } => {
                let entry_size = 16;

                for entry_index in 0..(*nsyms) {
                    let offset = (symoff + entry_index * entry_size) as usize;
                    let str_offset = (stroff + u32::from_le_bytes(buffer[offset..(offset + 4)].try_into().unwrap())) as usize;
                    let str_length = buffer[str_offset..].iter().position(|&r| r == b'\0').unwrap();
                    let _name = unsafe { String::from_utf8_unchecked(buffer[str_offset..(str_offset + str_length)].to_vec()) };

                    let addr = u64::from_le_bytes(buffer[(offset + 8)..(offset + 16)].try_into().unwrap());
                    entries.insert(addr);

                    // eprintln!("{:?}", name);
                    // eprintln!("{:x?}", &buffer[(offset + 8)..(offset + 16)]);

                    // let name = String::from_utf8_lossy(&buf[str_offset..(str_offset + str_length)]);

                    // if !name.starts_with("__") {
                    //     // let x = &buf[(symoff + entry_index * entry_size) as usize..(symoff + (entry_index + 1) * entry_size) as usize];
                    //     // eprintln!("{:x?}", x);

                    //     println!("{}", name);
                    // }

                    // c.push(str_offset);
                }
            },
            _ => {},
        }
    }

    // eprintln!("{:?}", entries.len());

    // let mut p = entries.iter().collect::<Vec<_>>();
    // p.sort();
    // let p = p.windows(2).map(|v| v[1] - v[0]).collect::<Vec<_>>();
    // eprintln!("{:?}", p);


    let module = by::BinaryenModuleCreate();

    by::BinaryenModuleSetFeatures(module, by::BinaryenFeatureMemory64());


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


    // Initialize memory

    const PAGE_SIZE: usize = 65_536;

    let emul_mem_name = CString::new("emul_mem").unwrap();

    by::BinaryenSetMemory(
        module,
        mem_size.div_ceil(PAGE_SIZE) as u32,
        std::mem::transmute(-1),
        std::ptr::null(),
        segment_names.iter().map(|name| name.as_ptr()).collect::<Vec<_>>().as_mut_ptr(),
        segment_datas.as_mut_ptr() as *mut *const i8,
        segment_passives.as_mut_ptr(),
        segment_offsets.as_mut_ptr(),
        segment_sizes.as_mut_ptr(),
        segment_names.len() as u32,
        false,
        true,
        emul_mem_name.as_ptr(),
    );


    // Translate instructions

    let loop_name = CString::new("loop").unwrap();
    let loop_body_name = CString::new("body").unwrap();

    let current_pointer_expr = by::BinaryenLocalGet(module, 0, by::BinaryenTypeInt64());

    let loop_ = by::BinaryenLoop(module, loop_name.as_ptr(),
        by::BinaryenBlock(module, loop_body_name.as_ptr(),
            [
                by::BinaryenIf(
                    module,
                    by::BinaryenBinary(
                        module,
                        by::BinaryenEqInt64(),
                        current_pointer_expr,
                        by::BinaryenConst(module, by::BinaryenLiteralInt64(0)),
                    ),
                    by::BinaryenNop(module),
                    by::BinaryenNop(module),
                ),
                by::BinaryenBreak(
                    module,
                    loop_name.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ),
            ].as_mut_ptr(), 2, by::BinaryenTypeNone()
        )
    );


    // Create functions

    let mut var_types = [by::BinaryenTypeInt64()];

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
        [by::BinaryenConst(module, by::BinaryenLiteralInt64(std::mem::transmute(entry_addr)))].as_mut_ptr(),
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


/*     for &MachCommand(ref cmd, _cmdsize) in &commands {
        match cmd {
            LoadCommand::Segment64 { ref sections, .. } => {
                for section in sections {
                    let attributes = section.flags.sect_attrs();

                    if attributes.contains(SectionAttributes::S_ATTR_PURE_INSTRUCTIONS) {
                        entries.insert(section.addr as u64);

                        for instruction_index in 0..(section.size / INSTRUCTION_SIZE) {
                            let current_addr = (section.addr as u64) + (instruction_index * INSTRUCTION_SIZE) as u64;
                            let offset = (section.offset as usize + instruction_index * INSTRUCTION_SIZE) as usize;
                            let instruction_encoded = u32::from_le_bytes(buffer[offset..(offset + 4)].try_into().unwrap());
                            let instruction = match disarm64::decoder::decode(instruction_encoded) {
                                Some(instruction) => instruction,
                                None => continue,
                            };

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
                                    let addr_offset = inst.imm26() as u64;
                                    entries.insert(current_addr + (addr_offset << 2));

                                    // eprintln!("{instruction} {:x} {:x}", current_addr, current_addr + (target << 2));
                                    // entries.insert(target);
                                },
                                Operation::CONDBRANCH(decoder::CONDBRANCH::B__ADDR_PCREL19(inst)) => {
                                    let addr_offset = inst.imm19() as u64;
                                    entries.insert(current_addr + (addr_offset << 2));
                                },
                                Operation::BRANCH_REG(decoder::BRANCH_REG::RET_Rn(inst)) => {
                                    let _reg = inst.rn();

                                    // eprintln!(">> {:?}", reg);
                                    // let target = instruction.operands[0].value.unwrap();
                                    // entries.insert(target);
                                },
                                _ => {},
                            }
                        }
                    }
                }
            },
            LoadCommand::SymTab { symoff, nsyms, stroff, .. } => {
                let entry_size = 16;

                for entry_index in 0..(*nsyms) {
                    let offset = (symoff + entry_index * entry_size) as usize;
                    let str_offset = (stroff + u32::from_le_bytes(buffer[offset..(offset + 4)].try_into().unwrap())) as usize;
                    let str_length = buffer[str_offset..].iter().position(|&r| r == b'\0').unwrap();
                    let _name = unsafe { String::from_utf8_unchecked(buffer[str_offset..(str_offset + str_length)].to_vec()) };

                    let addr = u64::from_le_bytes(buffer[(offset + 8)..(offset + 16)].try_into().unwrap());
                    entries.insert(addr);

                    // eprintln!("{:?}", name);
                    // eprintln!("{:x?}", &buffer[(offset + 8)..(offset + 16)]);

                    // let name = String::from_utf8_lossy(&buf[str_offset..(str_offset + str_length)]);

                    // if !name.starts_with("__") {
                    //     // let x = &buf[(symoff + entry_index * entry_size) as usize..(symoff + (entry_index + 1) * entry_size) as usize];
                    //     // eprintln!("{:x?}", x);

                    //     println!("{}", name);
                    // }

                    // c.push(str_offset);
                }
            },
            _ => {},
        }
    } */


    // Emit WASM binary

    by::BinaryenModulePrint(module);
    by::BinaryenModuleValidate(module);

    let mut output = vec![0u8; 10_000_000];
    let written = by::BinaryenModuleWrite(module, output.as_mut_ptr() as *mut i8, output.len());

    // Clean up the module, which owns all the objects we created above
    by::BinaryenModuleDispose(module);


    let mut output_file = File::create("output.wasm").unwrap();
    output_file.write_all(&output[..written]).unwrap();


    Ok(())
}
