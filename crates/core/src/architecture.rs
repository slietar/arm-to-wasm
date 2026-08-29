use bnyr::Expression;

use crate::translator::RoutineContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    W32,
    W64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    None,
    Call,
    Jump { conditional: bool, target_address: u64 },
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackAccess {
    pub offset: i32,
    pub size_bytes: u32,
    pub read: bool,
    pub write: bool,
}

pub trait Instr: std::fmt::Debug {
    fn size(&self) -> u64;

    fn translate(&self, ctx: &RoutineContext, current_address: u64, block_exprs: &mut Vec<Expression>);
    fn branch_condition(&self, ctx: &RoutineContext) -> Option<Expression>;

    // Used only by the generic CFG-building analysis.
    fn branch_kind(&self, address: u64) -> BranchKind;

    /// `(new_frame_size_if_this_instruction_establishes_one, memory_accesses_relative_to_stack_pointer)`.
    /// Purely descriptive - the caller decides whether it's actually in a prologue.
    fn stack_frame_effect(&self, stack_pointer: u32) -> (Option<u64>, Vec<StackAccess>);

    fn registers_read(&self) -> Vec<u32>;
    fn registers_written(&self) -> Vec<u32>;
}

#[derive(Debug)]
pub enum LocalType {
    F32,
    F64,
    I32,
    I64,
}

#[derive(Debug)]
pub struct LocalDescriptor {
    pub argument: bool,
    pub return_value: bool,
    pub type_: LocalType,
}

pub trait Architecture: std::fmt::Debug {
    fn instruction_size(&self) -> u64;
    fn decode_instructions(&self, bytes: &[u8]) -> Vec<Box<dyn Instr>>;

    fn locals(&self) -> Vec<LocalDescriptor>;

    fn param_registers(&self) -> Vec<u32>;
    fn svc_param_registers(&self) -> Vec<u32>;
    fn svc_return_registers(&self) -> Vec<u32>;
    fn stack_pointer_register(&self) -> u32;
    fn is_zero_register(&self, register: u32) -> bool;
}
