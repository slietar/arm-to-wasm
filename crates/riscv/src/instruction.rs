use aw_core::architecture::{BranchKind, Instr, StackAccess};
use aw_core::translator::RoutineContext;
use bnyr::Expression;
use raki::{BaseIOpcode, COpcode, Instruction, OpcodeKind};

/// Return-address register (`ra` / `x1`).
const RA: usize = 1;
/// Zero register (`x0`).
const ZERO: usize = 0;

#[derive(Debug)]
pub struct RiscVInstruction(pub Instruction);

impl Instr for RiscVInstruction {
    fn size(&self) -> u64 {
        if self.0.is_compressed {
            2
        } else {
            4
        }
    }

    fn translate(
        &self,
        _ctx: &RoutineContext,
        _current_address: u64,
        _block_exprs: &mut Vec<Expression>,
    ) {
        todo!("RISC-V instruction translation is not implemented yet")
    }

    fn branch_condition(&self, _ctx: &RoutineContext) -> Option<Expression> {
        todo!("RISC-V branch conditions are not implemented yet")
    }

    fn branch_kind(&self, address: u64) -> BranchKind {
        let instruction = &self.0;

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
                target_address: address + (instruction.imm.unwrap() as u64),
            },

            // JAL saves the return address in `rd`. If `rd` is zero, the return
            // address is discarded.
            OpcodeKind::BaseI(BaseIOpcode::JAL) => match instruction.rd.unwrap() {
                ZERO => BranchKind::Jump {
                    conditional: false,
                    target_address: address + (instruction.imm.unwrap() as u64),
                },
                _ => BranchKind::Call,
            },
            OpcodeKind::C(COpcode::J) => BranchKind::Jump {
                conditional: false,
                target_address: address + (instruction.imm.unwrap() as u64),
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
        // todo!("RISC-V stack frame analysis is not implemented yet")
        (None, Vec::new())
    }

    fn registers_read(&self) -> Vec<u32> {
        // todo!("RISC-V register-read analysis is not implemented yet")
        Vec::new()
    }

    fn registers_written(&self) -> Vec<u32> {
        Vec::new()
        // todo!("RISC-V register-written analysis is not implemented yet")
    }
}
