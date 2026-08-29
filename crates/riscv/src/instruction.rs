use aw_core::architecture::{BranchKind, Instr, StackAccess};
use aw_core::translator::RoutineContext;
use bnyr::Expression;
use raki::{BaseIOpcode, COpcode, Instruction, OpcodeKind};

/// Return-address register (`ra` / `x1`).
const RA: usize = 1;
/// Zero register (`x0`).
const ZERO: usize = 0;

/// Wraps `raki::Instruction` so `aw_core::architecture::Instr` can be
/// implemented for it (both are foreign to this crate, so the orphan rule
/// requires a local newtype).
///
/// RISC-V instructions are 16-bit-aligned even when 32 bits wide (the `C`
/// extension packs 16-bit instructions), while the rest of the analysis
/// pipeline assumes every element of a decoded instruction stream occupies
/// exactly `Architecture::instruction_size()` bytes. To reconcile the two,
/// `RiscV::instruction_size` reports a halfword (2 bytes), and a 32-bit-wide
/// instruction is represented as a real entry followed by a `Padding` entry
/// standing in for its second halfword.
#[derive(Debug)]
pub enum RiscVInstruction {
    Instruction(Instruction),
    Padding,
}

impl Instr for RiscVInstruction {
    fn translate(
        &self,
        _ctx: &RoutineContext,
        _current_address: u64,
        _block_exprs: &mut Vec<Expression>,
    ) {
        let Self::Instruction(_instruction) = self else {
            return;
        };

        todo!("RISC-V instruction translation is not implemented yet")
    }

    fn branch_condition(&self, _ctx: &RoutineContext) -> Option<Expression> {
        let Self::Instruction(_instruction) = self else {
            return None;
        };

        todo!("RISC-V branch conditions are not implemented yet")
    }

    fn branch_kind(&self) -> BranchKind {
        let Self::Instruction(instruction) = self else {
            return BranchKind::None;
        };

        match &instruction.opc {
            OpcodeKind::BaseI(
                BaseIOpcode::BEQ
                | BaseIOpcode::BNE
                | BaseIOpcode::BLT
                | BaseIOpcode::BGE
                | BaseIOpcode::BLTU
                | BaseIOpcode::BGEU,
            )
            | OpcodeKind::C(COpcode::BEQZ | COpcode::BNEZ) => BranchKind::Jump {
                conditional: true,
                target_offset: (instruction.imm.unwrap() * 2) as i64,
            },

            // JAL saves the return address in `rd`. If `rd` is zero, the return
            // address is discarded.
            OpcodeKind::BaseI(BaseIOpcode::JAL) => match instruction.rd.unwrap() {
                ZERO => BranchKind::Jump {
                    conditional: false,
                    target_offset: (instruction.imm.unwrap() * 2) as i64,
                },
                _ => BranchKind::Call,
            },
            OpcodeKind::C(COpcode::J) => BranchKind::Jump {
                conditional: false,
                target_offset: (instruction.imm.unwrap() * 2) as i64,
            },
            OpcodeKind::C(COpcode::JAL) => BranchKind::Call,

            // JALR and JR are used for returns
            OpcodeKind::BaseI(BaseIOpcode::JALR)
                if matches!(instruction.rd, Some(ZERO) | None)
                    && instruction.rs1 == Some(RA)
                    && instruction.imm == Some(0) =>
            {
                BranchKind::Return
            }
            OpcodeKind::C(COpcode::JR) if instruction.rs1 == Some(RA) => BranchKind::Return,

            _ => BranchKind::None,
        }
    }

    fn stack_frame_effect(&self, _stack_pointer: u32) -> (Option<u64>, Vec<StackAccess>) {
        let Self::Instruction(_instruction) = self else {
            return (None, Vec::new());
        };

        // todo!("RISC-V stack frame analysis is not implemented yet")
        (None, Vec::new())
    }

    fn registers_read(&self) -> Vec<u32> {
        let Self::Instruction(_instruction) = self else {
            return Vec::new();
        };

        // todo!("RISC-V register-read analysis is not implemented yet")
        Vec::new()
    }

    fn registers_written(&self) -> Vec<u32> {
        let Self::Instruction(_instruction) = self else {
            return Vec::new();
        };

        Vec::new()
        // todo!("RISC-V register-written analysis is not implemented yet")
    }
}
