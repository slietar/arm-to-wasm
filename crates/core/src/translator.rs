use std::{collections::HashMap, ffi::CString};

use crate::{
    analysis::{Analysis, ElfFile, Routine},
    architecture::{Architecture, Instr as _, LocalDescriptor, LocalType, Width},
    constants::PAGE_SIZE,
};
use bnyr::{Expression, MemorySegmentDescriptor, Module, Type, UnaryOp};
use elf::ElfBytes;

#[derive(Debug)]
pub struct RoutineContext<'a, A: Architecture> {
    pub global: &'a GlobalContext<A>,

    local_descriptors: Vec<LocalDescriptor>,
    local_index_by_external_local_index: HashMap<u32, u32>,
    func_return_scratch_local_index: u32,
    func_return_scratch_local_type: Type,
    pub module: Module,
}

impl<'a, A: Architecture> RoutineContext<'a, A> {
    pub fn call_routine(&self, routine_index: usize, block_exprs: &mut Vec<Expression>) {
        let routine = &self.global.analysis.routines[routine_index];
        eprintln!(
            "Calling routine {} ({})",
            routine_index,
            routine.name.as_deref().unwrap_or("none")
        );

        let arg_exprs = self
            .local_descriptors
            .iter()
            .filter(|desc| desc.argument)
            .enumerate()
            .map(|(local_index, desc)| self.read_local(local_index as u32))
            .collect::<Vec<_>>();

        block_exprs.push(self.module.local_set(
            self.func_return_scratch_local_index,
            self.module.call(
                &self.global.function_names[routine_index],
                &arg_exprs,
                self.func_return_scratch_local_type.clone(),
            ),
        ));

        for (local_index, _) in self
            .local_descriptors
            .iter()
            .filter(|desc| desc.return_value)
            .enumerate()
        {
            block_exprs.push(self.write_local(
                local_index as u32,
                self.module.tuple_extract(
                    self.module.local_get(
                        self.func_return_scratch_local_index,
                        self.func_return_scratch_local_type.clone(),
                    ),
                    local_index as u32,
                ),
            ));
        }
    }

    pub fn read_local(&self, local_index: u32) -> Expression {
        let descriptor = &self.local_descriptors[local_index as usize];
        let local_type = self.global.local_type_to_type(&descriptor.type_);
        self.module.local_get(local_index, local_type)
    }

    pub fn write_local(&self, local_index: u32, value: Expression) -> Expression {
        let descriptor = &self.local_descriptors[local_index as usize];
        let local_type = self.global.local_type_to_type(&descriptor.type_);
        self.module.local_set(local_index, value)
    }

    pub fn return_(&self) -> Expression {
        self.module.return_(
            self.module.tuple(
                &self
                    .local_descriptors
                    .iter()
                    .enumerate()
                    .filter(|(_, desc)| desc.return_value)
                    .map(|(i, _)| self.read_local(i as u32))
                    .collect::<Vec<_>>(),
            ),
        )
    }
}

#[derive(Debug)]
pub struct GlobalContext<A: Architecture> {
    pub module: Module,

    pub analysis: Analysis<A>,
    pub architecture: Box<A>,
    pub function_names: Vec<String>,
    pub memory_name: CString,
}

impl<A: Architecture> GlobalContext<A> {
    fn local_type_to_type(&self, local_type: &LocalType) -> Type {
        match local_type {
            LocalType::F32 => todo!(),
            LocalType::F64 => todo!(),
            LocalType::I32 => self.module.i32(),
            LocalType::I64 => self.module.i64(),
        }
    }

    pub fn translate_elf(
        bytes: &[u8],
        architecture: Box<A>,
    ) -> Result<Module, Box<dyn std::error::Error>> {
        // TODO: Avoid redundancy
        let elf_file = ElfFile::minimal_parse(bytes)?;

        let analysis = crate::analysis::analyze(bytes, &elf_file, architecture.as_ref())?;

        let module = Module::new();

        let function_names = analysis
            .routines
            .iter()
            .enumerate()
            .map(|(routine_index, routine)| {
                format!(
                    "routine_{}_{}",
                    routine_index,
                    routine.name.as_deref().unwrap_or("none")
                )
            })
            .collect::<Vec<_>>();

        let memory_info = set_up_memory("memory", bytes, &elf_file, &module);

        let context = Self {
            analysis,
            function_names,
            memory_name: CString::new(memory_info.name).unwrap(),
            module: module.clone(),
            architecture,
        };

        context.architecture.setup(&context)?;

        for (routine_index, routine) in context.analysis.routines.iter().enumerate() {
            let function_name = &context.function_names[routine_index];

            context.translate_routine(routine, function_name);
        }

        if let Some(entry_routine_index) = context.analysis.entry_routine_index {
            let local_descriptors = context.architecture.locals();

            let param_types = local_descriptors
                .iter()
                .filter(|desc| desc.argument)
                .map(|desc| context.local_type_to_type(&desc.type_))
                .collect::<Vec<_>>();

            let arg_exprs = local_descriptors
                .iter()
                .filter(|desc| desc.argument)
                .map(|desc| {
                    if desc.stack_pointer {
                        module.const_(memory_info.stack_internal_address as i64)
                    } else {
                        module.const_(0i64)
                    }
                })
                .collect::<Vec<_>>();

            let entry_function_name = "entry";
            let entry_function = module.function(
                entry_function_name,
                &[],
                module.none(),
                &[],
                module.block(
                    module.none(),
                    &[
                        module.drop(module.call(
                            &context.function_names[entry_routine_index],
                            &arg_exprs,
                            module.tuple_type(&param_types),
                        )),
                        module.unreachable(),
                    ],
                ),
            );

            module.export_function(entry_function_name, "_entry");
        }

        // Export all routines
        // for (routine_index, routine) in context.analysis.routines.iter().enumerate() {
        //     let function_name = &context.function_names[routine_index];
        //     module.export_function(function_name, function_name);
        // }

        Ok(module)
    }

    pub fn translate_routine(&self, routine: &Routine<A>, function_name: &str) {
        let module = &self.module;

        // Allocate parameters and locals

        let mut next_local_index = 0;

        let mut param_types = Vec::new();
        let mut local_types = Vec::new();

        let local_descriptors = self.architecture.locals();
        let mut local_index_by_external_local_index = HashMap::new();

        for (external_local_index, descriptor) in local_descriptors.iter().enumerate() {
            if !descriptor.argument {
                continue;
            }

            let local_index = next_local_index;

            param_types.push(self.local_type_to_type(&descriptor.type_));
            next_local_index += 1;
            local_index_by_external_local_index.insert(external_local_index as u32, local_index);
        }

        for (external_local_index, descriptor) in local_descriptors.iter().enumerate() {
            if descriptor.argument {
                continue;
            }

            let local_index = next_local_index;

            local_types.push(self.local_type_to_type(&descriptor.type_));
            next_local_index += 1;
            local_index_by_external_local_index.insert(external_local_index as u32, local_index);
        }

        let relooper_helper_local_index = next_local_index;

        local_types.push(module.i32());
        next_local_index += 1;

        let return_types = local_descriptors
            .iter()
            .filter(|desc| desc.return_value)
            .map(|desc| self.local_type_to_type(&desc.type_))
            .collect::<Vec<_>>();

        let return_type = module.tuple_type(&return_types);

        // Additional locals for temporary values
        let func_return_scratch_local_index = next_local_index;
        local_types.push(return_type.clone());
        next_local_index += 1;

        _ = next_local_index;

        // Run translation

        let relooper = module.relooper();

        let context = RoutineContext {
            global: self,

            func_return_scratch_local_index,
            func_return_scratch_local_type: return_type.clone(),
            local_descriptors,
            local_index_by_external_local_index,
            module: module.clone(),
        };

        let mut routine_exprs = Vec::new();

        let mut relooper_blocks = Vec::new();

        for (block_index, block) in routine.blocks.iter().enumerate() {
            let by_block = if true {
                let mut block_exprs = Vec::new();

                for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                    let current_address = block.start_address
                        + (instruction_index as u64) * self.architecture.instruction_size();

                    instruction.translate(
                        &context,
                        &block.metadata[instruction_index],
                        current_address,
                        &mut block_exprs,
                    );
                }

                if block.fallthrough_block_index.is_none() && block.jump_block_index.is_none() {
                    block_exprs.push(module.unreachable());
                    eprintln!("Unreachable block at index {}", block_index);
                }

                module.block(module.none(), &block_exprs)
            } else {
                module.unreachable()
            };

            let relooper_block = relooper.add_block(by_block);

            relooper_blocks.push(relooper_block);
        }

        for (block, relooper_block) in routine.blocks.iter().zip(relooper_blocks.iter()) {
            if let Some(fallthrough_block_index) = block.fallthrough_block_index {
                relooper.branch(
                    relooper_block,
                    &relooper_blocks[fallthrough_block_index],
                    None,
                );
            }

            if let Some(jump_block_index) = block.jump_block_index {
                let last_instruction = block.instructions.last().unwrap();
                let condition_expr = last_instruction.branch_condition(&context);

                assert!(condition_expr.is_none() || block.fallthrough_block_index.is_some());

                let condition_expr = condition_expr.map(|_| module.const_(1u32));

                relooper.branch(
                    relooper_block,
                    &relooper_blocks[jump_block_index],
                    condition_expr,
                );
            }
        }

        routine_exprs.push(relooper.finish(&relooper_blocks[0], relooper_helper_local_index));

        let func_block = module.block(module.none(), &routine_exprs);
        let _func = module.function(
            function_name,
            &param_types,
            return_type,
            &local_types,
            func_block,
        );

        // self.module.export_function(function_name, function_name);
    }
}

#[derive(Debug)]
struct MappedSegment<'a> {
    address: u64,
    data: &'a [u8],
    size: u64,
    writable: bool,
}

#[derive(Debug)]
pub struct MemoryInfo {
    pub name: String,
    pub stack_internal_address: u64,
    pub stack_size: u64,
}

pub fn set_up_memory(
    memory_name: &str,
    elf_bytes: &[u8],
    elf_file: &ElfFile,
    module: &Module,
) -> MemoryInfo {
    let mut mapped_segments = Vec::new();

    for segment in elf_file.segments().unwrap() {
        if segment.p_type == elf::abi::PT_LOAD {
            mapped_segments.push(MappedSegment {
                address: segment.p_vaddr,
                data: &elf_bytes
                    [(segment.p_offset as usize)..(segment.p_offset + segment.p_filesz) as usize],
                size: segment.p_filesz,
                writable: (segment.p_flags & elf::abi::PF_W) != 0,
            });
        }
    }

    let total_mapped_size = mapped_segments
        .iter()
        .map(|seg| seg.address + seg.size)
        .max()
        .unwrap_or(0)
        .div_ceil(PAGE_SIZE as u64)
        * PAGE_SIZE as u64;

    let segments = mapped_segments
        .iter()
        .enumerate()
        .map(|(i, seg)| MemorySegmentDescriptor {
            name: format!("segment_{}", i).into(),
            data: seg.data,
            passive: false,
            offset: module.const_(seg.address),
        })
        .collect::<Vec<_>>();

    let mapped_memory_page_count = total_mapped_size.div_ceil(PAGE_SIZE);
    let mapped_memory_size = mapped_memory_page_count * PAGE_SIZE;
    let stack_memory_page_count = 2;
    let stack_memory_internal_address = mapped_memory_size;

    module.set_memory(
        memory_name,
        memory_name,
        (mapped_memory_page_count + stack_memory_page_count) as u32,
        u32::MAX,
        false,
        true,
        &segments,
    );

    MemoryInfo {
        name: memory_name.to_string(),
        stack_internal_address: stack_memory_internal_address,
        stack_size: stack_memory_page_count * PAGE_SIZE,
    }
}
