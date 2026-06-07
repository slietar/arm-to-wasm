use std::ffi::CString;
use std::fs::File;

use binaryen::ffi as by;
use capstone::prelude::*;
use elf::ElfBytes;

use crate::module::Module;

const INSTRUCTION_SIZE: u64 = 4;
const PAGE_SIZE: u64 = 65_536;

pub const GP_REGISTER_COUNT: u32 = 31;

pub const SP_LOCAL_INDEX: u32 = 0;
pub const CARRY_FLAG_LOCAL_INDEX: u32 = 1;
pub const FIRST_GP_REGISTER_LOCAL_INDEX: u32 = 2;

pub fn get_arm_operand(operand: &arch::ArchOperand) -> &arch::arm64::Arm64Operand {
    if let arch::ArchOperand::Arm64Operand(arm_operand) = operand {
        arm_operand
    } else {
        unreachable!()
    }
}

fn get_register_id(operand: &arch::ArchOperand) -> u16 {
    if let arch::arm64::Arm64OperandType::Reg(reg_id) = get_arm_operand(operand).op_type {
        reg_id.0
    } else {
        unreachable!()
    }
}

#[derive(Debug)]
struct MappedSegment<'a> {
    address: u64,
    data: &'a [u8],
    memory_offset: u64,
    size: u64,
    writable: bool,
}

#[derive(Debug)]
struct ExecutableSection<'a> {
    address: u64,
    data: &'a [u8],
}

#[derive(Debug)]
struct ExecutableSegment {
    address: u64,
    source_offset: u64,
    size: u64,
}

fn find_mapped_segments<'a>(
    file: &'a ElfBytes<elf::endian::AnyEndian>,
    file_data: &'a [u8],
) -> (Vec<MappedSegment<'a>>, u64) {
    let mut current_offset = 0;
    let mut mapped_segments = Vec::new();

    for segment in file.segments().unwrap() {
        // eprintln!("Segment: {:?}", segment);

        if segment.p_type == elf::abi::PT_LOAD {
            // let is_executable = (segment.p_flags & elf::abi::PF_X) != 0;
            // if is_executable {
            //     eprintln!("Found executable segment at 0x{:x} with {} bytes", segment.p_offset, segment.p_filesz);
            // }

            // eprintln!("{} {}", segment.p_filesz, file_data[(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize].len());

            mapped_segments.push(MappedSegment {
                address: segment.p_vaddr,
                data: &file_data
                    [(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize],
                memory_offset: current_offset,
                size: segment.p_filesz,
                writable: (segment.p_flags & elf::abi::PF_W) != 0,
            });

            // eprintln!("{:?}", mapped_segments.last().unwrap().data);

            current_offset += segment.p_filesz;
        }
    }

    (mapped_segments, current_offset)
}

pub fn translate(elf_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Read ELF file

    let elf_file = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&elf_bytes)
        .map_err(|err| format!("Failed to parse ELF file: {err}"))?;

    if elf_file.ehdr.osabi != elf::abi::ELFOSABI_NONE
        && elf_file.ehdr.osabi != elf::abi::ELFOSABI_LINUX
    {
        return Err(format!(
            "Unsupported OS ABI: expected Linux (ELFOSABI_LINUX), found {}",
            elf_file.ehdr.osabi
        )
        .into());
    }

    if elf_file.ehdr.e_machine != elf::abi::EM_AARCH64 {
        return Err(format!(
            "Unsupported architecture: expected AArch64 (EM_AARCH64), found {}",
            elf_file.ehdr.e_machine
        )
        .into());
    }

    // Create module

    let module = Module::new();

    // List mapped segments

    let (mapped_segments, total_mapped_size) = find_mapped_segments(&elf_file, elf_bytes);

    // Create mapped memory

    let loaded_memory_name = CString::new("emul_mem").unwrap();

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

    unsafe {
        by::BinaryenSetMemory(
            module.by_module,
            total_mapped_size.div_ceil(PAGE_SIZE) as u32,
            i32::cast_unsigned(-1),
            loaded_memory_name.as_ptr(),
            segment_name_ptrs.as_mut_ptr() as *mut *const i8,
            segment_datas.as_mut_ptr() as *mut *const i8,
            segment_passives.as_mut_ptr(),
            segment_offsets.as_mut_ptr(),
            segment_sizes.as_mut_ptr(),
            mapped_segments.len() as u32,
            false,
            true,
            loaded_memory_name.as_ptr(),
        );
    }

    // Find executable segments

    let executable_segments: Vec<_> = elf_file
        .segments()
        .unwrap()
        .iter()
        .filter(|seg| (seg.p_type == elf::abi::PT_LOAD) && ((seg.p_flags & elf::abi::PF_X) != 0))
        .map(|seg| ExecutableSegment {
            address: seg.p_vaddr,
            source_offset: seg.p_offset,
            size: seg.p_filesz,
        })
        .collect();

    // eprintln!("Executable segments: {:#?}", executable_segments);

    // Find executable sections

    let (section_headers_opt, section_name_table_opt) = elf_file.section_headers_with_strtab()?;

    let section_headers =
        section_headers_opt.ok_or_else(|| "ELF has no section headers".to_string())?;
    let section_name_table =
        section_name_table_opt.ok_or_else(|| "ELF has no section name string table".to_string())?;

    let mut executable_sections = Vec::new();

    for section in section_headers.iter() {
        let is_executable = (section.sh_flags & elf::abi::SHF_EXECINSTR as u64) != 0;

        if !is_executable {
            continue;
        }

        let matching_executable_segments = executable_segments
            .iter()
            .filter(|seg| {
                let seg_start = seg.address;
                let seg_end = seg.address + seg.size;
                let sec_start = section.sh_addr;
                let sec_end = section.sh_addr + section.sh_size;

                (sec_start >= seg_start && sec_start < seg_end)
                    || (sec_end > seg_start && sec_end <= seg_end)
                    || (sec_start <= seg_start && sec_end >= seg_end)
            })
            .collect::<Vec<_>>();

        if matching_executable_segments.len() != 1 {
            return Err(format!(
                "Expected exactly one matching executable segment for section at address 0x{:x}, found {}",
                section.sh_addr,
                matching_executable_segments.len()
            )
            .into());
        }

        let matching_executable_segment = matching_executable_segments[0];
        let address = matching_executable_segment.address + section.sh_offset;

        let section_name = section_name_table
            .get(section.sh_name as usize)
            .unwrap_or("<invalid-section-name>");

        let (data, _) = elf_file.section_data(&section)?;

        executable_sections.push(ExecutableSection { address, data });
    }

    eprintln!("Executable sections: {:#?}", executable_sections);

    // Create disassembler

    let disassembler = Capstone::new()
        .arm64()
        .mode(capstone::arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .unwrap();

    let mut jump_targets = Vec::new();

    for executable_section in &executable_sections {
        let instructions = disassembler
            .disasm_all(&executable_section.data, 0x1000)
            .unwrap();

        eprintln!("Found {} instructions", instructions.len());

        for (instruction_index, instruction) in instructions.as_ref().iter().enumerate() {
            let detail: InsnDetail = disassembler.insn_detail(&instruction).unwrap();
            let arch_detail = detail.arch_detail();
            let ops = arch_detail.operands();

            if instruction.mnemonic().unwrap() == "bl" {
                if let capstone::arch::arm64::Arm64OperandType::Imm(imm) =
                    get_arm_operand(&ops[0]).op_type
                {
                    jump_targets.push(
                        executable_section.address + (instruction_index as u64) + (imm as u64),
                    );
                }
            }
        }
    }

    eprintln!("Identified jump targets: {:#x?}", jump_targets);

    // Create stack memory

    let stack_memory_name = CString::new("stack").unwrap();
    let stack_page_count = 1;
    let stack_size = stack_page_count * PAGE_SIZE;

    unsafe {
        by::BinaryenAddMemoryImport(
            module.by_module,
            stack_memory_name.as_ptr(),
            stack_memory_name.as_ptr(),
            stack_memory_name.as_ptr(),
            0u8,
        );
    }

    // Return

    let relooper = unsafe { by::RelooperCreate(module.by_module) };
    let mut translator = Translator {
        disassembler,
        executable_sections,
        module,
        stack_memory_name,
        relooper,
    };

    translator.run()?;

    Ok(())
}

#[derive(Debug)]
pub struct Translator<'a> {
    disassembler: Capstone,
    executable_sections: Vec<ExecutableSection<'a>>,
    module: Module,
    relooper: by::RelooperRef,
    stack_memory_name: CString,
}

impl Translator<'_> {
    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut exprs = Vec::new();

        for executable_section in &self.executable_sections {
            let instructions = self
                .disassembler
                .disasm_all(&executable_section.data, executable_section.address)
                .unwrap();

            for (instruction_index, instruction) in instructions.as_ref().iter().enumerate() {
                let expr = self.translate(
                    &instruction,
                    executable_section.address + (instruction_index as u64 * INSTRUCTION_SIZE),
                );

                exprs.push(expr);
            }
        }

        let block = unsafe {
            by::BinaryenBlock(
                self.module.by_module,
                "block".as_ptr() as *const i8,
                exprs.as_mut_ptr(),
                exprs.len() as u32,
                by::BinaryenTypeNone(),
            )
        };

        let relooped_block = unsafe { by::RelooperAddBlock(self.relooper, block) };
        // }

        // fn finish(&mut self, entry: by::BinaryenExpressionRef) -> Result<(), Box<dyn std::error::Error>> {
        let expr = unsafe { by::RelooperRenderAndDispose(self.relooper, relooped_block, 0) };

        let mut var_types = self.var_types();

        let main_func_name = CString::new("main").unwrap();

        let main_func = unsafe {
            by::BinaryenAddFunction(
                self.module.by_module,
                main_func_name.as_ptr(),
                by::BinaryenTypeNone(),
                by::BinaryenTypeNone(),
                var_types.as_mut_ptr(),
                var_types.len() as u32,
                expr,
            )
        };

        // Create entry function

        let entry_func_body = unsafe {
            by::BinaryenCall(
                self.module.by_module,
                main_func_name.as_ptr(),
                [].as_mut_ptr(),
                0,
                by::BinaryenTypeNone(),
            )
        };

        let entry_func_name = CString::new("_entry").unwrap();

        let _entry_func = unsafe {
            by::BinaryenAddFunction(
                self.module.by_module,
                entry_func_name.as_ptr(),
                by::BinaryenTypeNone(),
                by::BinaryenTypeNone(),
                [].as_mut_ptr(),
                0,
                entry_func_body,
            )
        };

        let _export = unsafe {
            by::BinaryenAddExport(
                self.module.by_module,
                entry_func_name.as_ptr(),
                entry_func_name.as_ptr(),
            )
        };

        let mut syscall_handler_params = unsafe {
            [
                by::BinaryenTypeInt32(),
                by::BinaryenTypeInt64(),
                by::BinaryenTypeInt64(),
                by::BinaryenTypeInt64(),
                by::BinaryenTypeInt64(),
                by::BinaryenTypeInt64(),
                by::BinaryenTypeInt64(),
                by::BinaryenTypeInt64(),
            ]
        };

        let external_module_name = CString::new("env").unwrap();
        let syscall_handler_name = CString::new("syscall_handler").unwrap();

        let _import = unsafe {
            by::BinaryenAddFunctionImport(
                self.module.by_module,
                syscall_handler_name.as_ptr(),
                external_module_name.as_ptr(),
                syscall_handler_name.as_ptr(),
                by::BinaryenTypeCreate(
                    syscall_handler_params.as_mut_ptr(),
                    syscall_handler_params.len() as u32,
                ),
                by::BinaryenTypeInt64(),
            )
        };

        let mut output_file = File::create("output.wasm")?;

        // self.module.optimize();
        self.module.print();
        self.module.save(&mut output_file)?;

        Ok(())
    }
}

impl Translator<'_> {
    fn get_reg_local_index(&self, reg_id: u16) -> u32 {
        use arch::arm64::Arm64Reg::*;

        match reg_id as u32 {
            ARM64_REG_SP => SP_LOCAL_INDEX,

            ARM64_REG_X0 | ARM64_REG_W0 => FIRST_GP_REGISTER_LOCAL_INDEX + 0,
            ARM64_REG_X1 | ARM64_REG_W1 => FIRST_GP_REGISTER_LOCAL_INDEX + 1,
            ARM64_REG_X2 | ARM64_REG_W2 => FIRST_GP_REGISTER_LOCAL_INDEX + 2,
            ARM64_REG_X3 | ARM64_REG_W3 => FIRST_GP_REGISTER_LOCAL_INDEX + 3,
            ARM64_REG_X4 | ARM64_REG_W4 => FIRST_GP_REGISTER_LOCAL_INDEX + 4,
            ARM64_REG_X5 | ARM64_REG_W5 => FIRST_GP_REGISTER_LOCAL_INDEX + 5,
            ARM64_REG_X6 | ARM64_REG_W6 => FIRST_GP_REGISTER_LOCAL_INDEX + 6,
            ARM64_REG_X7 | ARM64_REG_W7 => FIRST_GP_REGISTER_LOCAL_INDEX + 7,
            ARM64_REG_X8 | ARM64_REG_W8 => FIRST_GP_REGISTER_LOCAL_INDEX + 8,
            ARM64_REG_X9 | ARM64_REG_W9 => FIRST_GP_REGISTER_LOCAL_INDEX + 9,
            ARM64_REG_X10 | ARM64_REG_W10 => FIRST_GP_REGISTER_LOCAL_INDEX + 10,
            ARM64_REG_X11 | ARM64_REG_W11 => FIRST_GP_REGISTER_LOCAL_INDEX + 11,
            ARM64_REG_X12 | ARM64_REG_W12 => FIRST_GP_REGISTER_LOCAL_INDEX + 12,
            ARM64_REG_X13 | ARM64_REG_W13 => FIRST_GP_REGISTER_LOCAL_INDEX + 13,
            ARM64_REG_X14 | ARM64_REG_W14 => FIRST_GP_REGISTER_LOCAL_INDEX + 14,
            ARM64_REG_X15 | ARM64_REG_W15 => FIRST_GP_REGISTER_LOCAL_INDEX + 15,
            ARM64_REG_X16 | ARM64_REG_W16 => FIRST_GP_REGISTER_LOCAL_INDEX + 16,
            ARM64_REG_X17 | ARM64_REG_W17 => FIRST_GP_REGISTER_LOCAL_INDEX + 17,
            ARM64_REG_X18 | ARM64_REG_W18 => FIRST_GP_REGISTER_LOCAL_INDEX + 18,
            ARM64_REG_X19 | ARM64_REG_W19 => FIRST_GP_REGISTER_LOCAL_INDEX + 19,
            ARM64_REG_X20 | ARM64_REG_W20 => FIRST_GP_REGISTER_LOCAL_INDEX + 20,
            ARM64_REG_X21 | ARM64_REG_W21 => FIRST_GP_REGISTER_LOCAL_INDEX + 21,
            ARM64_REG_X22 | ARM64_REG_W22 => FIRST_GP_REGISTER_LOCAL_INDEX + 22,
            ARM64_REG_X23 | ARM64_REG_W23 => FIRST_GP_REGISTER_LOCAL_INDEX + 23,
            ARM64_REG_X24 | ARM64_REG_W24 => FIRST_GP_REGISTER_LOCAL_INDEX + 24,
            ARM64_REG_X25 | ARM64_REG_W25 => FIRST_GP_REGISTER_LOCAL_INDEX + 25,
            ARM64_REG_X26 | ARM64_REG_W26 => FIRST_GP_REGISTER_LOCAL_INDEX + 26,
            ARM64_REG_X27 | ARM64_REG_W27 => FIRST_GP_REGISTER_LOCAL_INDEX + 27,
            ARM64_REG_X28 | ARM64_REG_W28 => FIRST_GP_REGISTER_LOCAL_INDEX + 28,
            ARM64_REG_X29 | ARM64_REG_W29 => FIRST_GP_REGISTER_LOCAL_INDEX + 29,
            ARM64_REG_X30 | ARM64_REG_W30 => FIRST_GP_REGISTER_LOCAL_INDEX + 30,
            _ => todo!(),
        }
    }

    fn is_half_register(&self, reg_id: u16) -> bool {
        use arch::arm64::Arm64Reg::*;

        matches!(reg_id as u32, ARM64_REG_W0..=ARM64_REG_W30)
    }

    unsafe fn read_register(&self, reg_id: u16, use_zero_reg: bool) -> by::BinaryenExpressionRef {
        // if use_zero_reg && reg_id == 31 {
        //     return unsafe { by::BinaryenConst(self.module, by::BinaryenLiteralInt64(0)) };
        // }

        let full_register_expr = unsafe {
            by::BinaryenLocalGet(
                self.module.by_module,
                self.get_reg_local_index(reg_id),
                by::BinaryenInt64(),
            )
        };

        if self.is_half_register(reg_id) {
            // Extract the lower 32 bits
            unsafe {
                by::BinaryenUnary(
                    self.module.by_module,
                    by::BinaryenWrapInt64(),
                    full_register_expr,
                )
            }
        } else {
            full_register_expr
        }
    }

    fn read_register_from_operand(&self, operand: &arch::ArchOperand) -> by::BinaryenExpressionRef {
        unsafe { self.read_register(get_register_id(operand), false) }
    }

    fn write_register_from_operand(
        &self,
        operand: &arch::ArchOperand,
        value: by::BinaryenExpressionRef,
    ) -> by::BinaryenExpressionRef {
        let register_id = get_register_id(operand);
        let local_index = self.get_reg_local_index(register_id);

        let expr = if self.is_half_register(register_id) {
            unsafe {
                by::BinaryenBinary(
                    self.module.by_module,
                    by::BinaryenOrInt64(),
                    by::BinaryenUnary(self.module.by_module, by::BinaryenExtendUInt32(), value),
                    by::BinaryenBinary(
                        self.module.by_module,
                        by::BinaryenAndInt64(),
                        by::BinaryenLocalGet(
                            self.module.by_module,
                            local_index,
                            by::BinaryenInt64(),
                        ),
                        by::BinaryenConst(
                            self.module.by_module,
                            by::BinaryenLiteralInt64(u64::cast_signed(0xffff_ffff_0000_0000u64)),
                        ),
                    ),
                )
            }
        } else {
            value
        };

        unsafe {
            by::BinaryenLocalSet(
                self.module.by_module,
                self.get_reg_local_index(register_id),
                expr,
            )
        }
    }

    fn get_immediate_from_operand(
        &self,
        operand: &arch::ArchOperand,
        mode_32bit: bool,
    ) -> by::BinaryenExpressionRef {
        if let arch::arm64::Arm64OperandType::Imm(imm) = get_arm_operand(operand).op_type {
            let value = if mode_32bit {
                unsafe { by::BinaryenLiteralInt32(u32::cast_signed(imm as u32)) }
            } else {
                unsafe { by::BinaryenLiteralInt64(imm) }
            };

            unsafe { by::BinaryenConst(self.module.by_module, value) }
        } else {
            unreachable!()
        }
    }

    fn read_operand(
        &self,
        operand: &arch::ArchOperand,
        mode_32bit: bool,
    ) -> by::BinaryenExpressionRef {
        match get_arm_operand(operand).op_type {
            arch::arm64::Arm64OperandType::Reg(_) => self.read_register_from_operand(operand),
            arch::arm64::Arm64OperandType::Imm(_) => {
                self.get_immediate_from_operand(operand, mode_32bit)
            }
            _ => unimplemented!(),
        }
    }

    // fn resolve_address(
    //     &self,
    //     op: arch::arm64::Arm64OpMem,
    // ) { }

    pub fn setup(&self) -> by::BinaryenExpressionRef {
        let mut steps = unsafe {
            [
                by::BinaryenLocalSet(
                    self.module.by_module,
                    CARRY_FLAG_LOCAL_INDEX,
                    by::BinaryenConst(self.module.by_module, by::BinaryenLiteralInt32(0)),
                ),
                by::BinaryenLocalSet(
                    self.module.by_module,
                    SP_LOCAL_INDEX,
                    by::BinaryenConst(
                        self.module.by_module,
                        by::BinaryenLiteralInt64(PAGE_SIZE as i64),
                    ),
                ),
            ]
        };

        unsafe {
            by::BinaryenBlock(
                self.module.by_module,
                std::ptr::null(),
                steps.as_mut_ptr(),
                steps.len() as u32,
                by::BinaryenTypeNone(),
            )
        }
    }

    pub fn var_types(&self) -> Vec<by::BinaryenType> {
        let mut var_types = Vec::new();

        // SP
        var_types.push(unsafe { by::BinaryenTypeInt64() });

        // Carry flag
        var_types.push(unsafe { by::BinaryenTypeInt32() });

        // GP registers
        var_types.extend((0..GP_REGISTER_COUNT).map(|_| unsafe { by::BinaryenTypeInt64() }));

        var_types
    }

    pub fn translate(
        &self,
        instruction: &capstone::Insn,
        address: u64,
    ) -> by::BinaryenExpressionRef {
        let detail: InsnDetail = self.disassembler.insn_detail(&instruction).unwrap();
        let arch_detail = detail.arch_detail();
        let ops = arch_detail.operands();

        // eprintln!("Instruction id: {}", instruction.id().0);
        // eprintln!("Instruction mnemonic: {:?}", instruction.mnemonic());

        match instruction.mnemonic().unwrap() {
            "bl" => {
                for op in &ops {
                    // eprintln!("Operand: {:?}", op);
                }

                unsafe { by::BinaryenNop(self.module.by_module) }
            }

            "add" => {
                let mode_32bit = self.is_half_register(get_register_id(&ops[0]));

                self.write_register_from_operand(&ops[0], unsafe {
                    by::BinaryenBinary(
                        self.module.by_module,
                        by::BinaryenAddInt64(),
                        self.read_register_from_operand(&ops[1]),
                        self.get_immediate_from_operand(&ops[2], mode_32bit),
                    )

                    // For ADC
                    // by::BinaryenBinary(
                    //     self.module.by_module,
                    //     by::BinaryenAddInt64(),
                    //     by::BinaryenBinary(
                    //         self.module.by_module,
                    //         by::BinaryenAddInt64(),
                    //         self.read_reg64_from_operand(&ops[1]),
                    //         self.get_i64_immediate_from_operand(&ops[2]),
                    //     ),
                    //     by::BinaryenLocalGet(self.module.by_module, CARRY_FLAG_LOCAL_INDEX, by::BinaryenInt32()),
                    // )
                })
            }
            "adrp" => {
                let imm = if let arch::arm64::Arm64OperandType::Imm(imm) =
                    get_arm_operand(&ops[1]).op_type
                {
                    imm
                } else {
                    unreachable!()
                };

                self.write_register_from_operand(&ops[0], unsafe {
                    by::BinaryenConst(
                        self.module.by_module,
                        by::BinaryenLiteralInt64(u64::cast_signed(
                            (address & (0xffff_ffff_ffff_f000)) | ((imm as u64) << 12),
                        )),
                    )
                })

                // self.write_register_from_operand(&ops[0], unsafe {
                // by::BinaryenBinary(
                //     self.module.by_module,
                //     by::BinaryenAddInt32(),
                //     by::BinaryenConst(
                //         self.module.by_module,
                //         by::BinaryenLiteralInt64(u64::cast_signed(
                //             address & (0xffff_ffff_ffff_f000),
                //         )),
                //     ),
                //     by::BinaryenBinary(
                //         self.module.by_module,
                //         by::BinaryenShlInt64(),
                //         self.read_operand(&ops[1], false),
                //         by::BinaryenConst(self.module.by_module, by::BinaryenLiteralInt32(12)),
                //     ),
                // )
            }
            "mov" => {
                // let op0 = get_arm_operand(&ops[0]);
                // let op1 = get_arm_operand(&ops[1]);
                // let op2 = get_arm_operand(&ops[2]);

                // eprintln!("op0: {op0:?}");
                // eprintln!("op1: {op1:?}");
                // eprintln!("op2: {op2:?}");

                // Is 32 bit?
                let mode_32bit = self.is_half_register(get_register_id(&ops[0]));

                self.write_register_from_operand(&ops[0], self.read_operand(&ops[1], mode_32bit))
            }
            "str" => {
                for op in &ops {
                    eprintln!("Operand: {:?}", op);
                }

                let mem_op = if let arch::arm64::Arm64OperandType::Mem(mem_op) =
                    get_arm_operand(&ops[1]).op_type
                {
                    mem_op
                } else {
                    unreachable!()
                };

                if mem_op.base().0 != (arch::arm64::Arm64Reg::ARM64_REG_SP as u16) {
                    unimplemented!("Only STR with SP as base is supported for now");
                }

                let memory_name = CString::new("stack").unwrap();

                unsafe {
                    by::BinaryenStore(
                        self.module.by_module,
                        8,
                        mem_op.disp() as u32,
                        8,
                        by::BinaryenLocalGet(
                            self.module.by_module,
                            SP_LOCAL_INDEX,
                            by::BinaryenInt64(),
                        ),
                        self.read_register_from_operand(&ops[0]),
                        by::BinaryenTypeInt64(),
                        memory_name.as_ptr(),
                    )
                }
            }
            "svc" => {
                use arch::arm64::Arm64Reg::*;

                let imm_value = self.get_immediate_from_operand(&ops[0], true);
                let mut operands = unsafe {
                    [
                        imm_value,
                        self.read_register(ARM64_REG_X8 as u16, false),
                        self.read_register(ARM64_REG_X0 as u16, false),
                        self.read_register(ARM64_REG_X1 as u16, false),
                        self.read_register(ARM64_REG_X2 as u16, false),
                        self.read_register(ARM64_REG_X3 as u16, false),
                        self.read_register(ARM64_REG_X4 as u16, false),
                        self.read_register(ARM64_REG_X5 as u16, false),
                    ]
                };

                let func_name = CString::new("syscall_handler").unwrap();

                unsafe {
                    by::BinaryenLocalSet(
                        self.module.by_module,
                        FIRST_GP_REGISTER_LOCAL_INDEX,
                        by::BinaryenCall(
                            self.module.by_module,
                            func_name.as_ptr(),
                            operands.as_mut_ptr(),
                            operands.len() as u32,
                            by::BinaryenTypeInt64(),
                        ),
                    )
                }
            }
            "sub" => {
                // let p = arch_detail.arm64().unwrap();
                // eprintln!("Is this a SUB with carry? {}", p.update_flags());

                let mode_32bit = self.is_half_register(get_register_id(&ops[0]));

                self.write_register_from_operand(&ops[0], unsafe {
                    by::BinaryenBinary(
                        self.module.by_module,
                        by::BinaryenSubInt64(),
                        self.read_register_from_operand(&ops[1]),
                        self.get_immediate_from_operand(&ops[2], mode_32bit),
                    )
                })
            }
            _ => unsafe { by::BinaryenNop(self.module.by_module) },
        }
    }
}
