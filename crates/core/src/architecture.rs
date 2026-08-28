use bnyr::Expression;

use crate::translator::RoutineContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    W32,
    W64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    /// Ordinary instruction, doesn't affect the control-flow graph.
    None,
    /// Link-setting branch (e.g. `BL`): doesn't affect the CFG, but ends prologue detection.
    Call,
    /// Transfers control to `current_address + target_offset * instruction_size`.
    /// `conditional == false` means execution never continues past this instruction.
    Jump { conditional: bool, target_offset: i64 },
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
    fn translate(&self, ctx: &RoutineContext, current_address: u64, block_exprs: &mut Vec<Expression>);
    fn branch_condition(&self, ctx: &RoutineContext) -> Option<Expression>;

    // Used only by the generic CFG-building analysis.
    fn branch_kind(&self) -> BranchKind;

    /// `(new_frame_size_if_this_instruction_establishes_one, memory_accesses_relative_to_stack_pointer)`.
    /// Purely descriptive - the caller decides whether it's actually in a prologue.
    fn stack_frame_effect(&self, stack_pointer: u32) -> (Option<u64>, Vec<StackAccess>);

    fn registers_read(&self) -> Vec<u32>;
    fn registers_written(&self) -> Vec<u32>;
}

pub trait Architecture: std::fmt::Debug {
    fn instruction_size(&self) -> u64;
    fn decode_instructions(&self, bytes: &[u8]) -> Vec<Box<dyn Instr>>;

    /// Every register id that needs a local allocated for it.
    fn all_registers(&self) -> Vec<u32>;
    /// Storage width of the local backing this register.
    fn local_width(&self, register: u32) -> Width;

    fn param_registers(&self) -> Vec<u32>;
    fn svc_param_registers(&self) -> Vec<u32>;
    fn svc_return_registers(&self) -> Vec<u32>;
    fn stack_pointer_register(&self) -> u32;
    fn is_zero_register(&self, register: u32) -> bool;
}
