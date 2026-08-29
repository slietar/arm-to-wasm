use aw_core::architecture::{BranchKind, Instr, StackAccess};
use aw_core::translator::RoutineContext;
use bnyr::{BinaryOp, Expression, LoadVariant, StoreVariant};
use raki::{BaseIOpcode, COpcode, Instruction, OpcodeKind};

use crate::simplify_instruction::expand_compressed;

/// Return-address register (`ra` / `x1`).
const RA: usize = 1;
/// Zero register (`x0`).
const ZERO: usize = 0;
/// Stack pointer register (`sp` / `x2`).
const SP: usize = 2;

#[derive(Debug)]
pub struct RiscVInstruction(pub Instruction);

fn register_to_local_index(register: usize) -> u32 {
    (register as u32) - 1
}

impl RiscVInstruction {
    fn read_register(&self, ctx: &RoutineContext, register: usize) -> Expression {
        if register == ZERO {
            ctx.module.const_(0i64)
        } else {
            ctx.read_local(register_to_local_index(register))
        }
    }

    fn write_register(
        &self,
        ctx: &RoutineContext,
        register: usize,
        value: Expression,
    ) -> Expression {
        if register == ZERO {
            ctx.module.nop()
        } else {
            ctx.write_local(register_to_local_index(register), value)
        }
    }
}

impl Instr for RiscVInstruction {
    fn size(&self) -> u64 {
        if self.0.is_compressed { 2 } else { 4 }
    }

    fn translate(
        &self,
        ctx: &RoutineContext,
        _current_address: u64,
        block_exprs: &mut Vec<Expression>,
    ) {
        let uncompressed_instr = expand_compressed(&self.0);
        let instr = uncompressed_instr.as_ref().unwrap_or(&self.0);

        match &instr.opc {
            OpcodeKind::BaseI(BaseIOpcode::ADDI) => block_exprs.push(self.write_register(
                ctx,
                instr.rd.unwrap(),
                ctx.module.binary(
                    self.read_register(ctx, instr.rs1.unwrap()),
                    ctx.module.const_(instr.imm.unwrap() as i64),
                    BinaryOp::AddInt64,
                ),
            )),
            OpcodeKind::BaseI(BaseIOpcode::LW) => block_exprs.push(self.write_register(
                ctx,
                instr.rd.unwrap(),
                ctx.module.load(
                    LoadVariant::I64L32 { signed: true },
                    ctx.module.binary(
                        self.read_register(ctx, instr.rs1.unwrap()),
                        ctx.module.const_(instr.imm.unwrap() as i64),
                        BinaryOp::AddInt64,
                    ),
                    0,
                    4,
                    &ctx.global.memory_name,
                ),
            )),
            OpcodeKind::BaseI(BaseIOpcode::SW) => block_exprs.push(ctx.module.store(
                StoreVariant::I64L32,
                self.read_register(ctx, instr.rs2.unwrap()),
                ctx.module.binary(
                    self.read_register(ctx, instr.rs1.unwrap()),
                    ctx.module.const_(instr.imm.unwrap() as i64),
                    BinaryOp::AddInt64,
                ),
                0,
                4,
                &ctx.global.memory_name,
            )),
            OpcodeKind::BaseI(BaseIOpcode::JALR)
                if instr.rd == Some(ZERO) && instr.rs1 == Some(RA) && instr.imm == Some(0) =>
            {
                block_exprs.push(ctx.return_());
            }

            _ => {
                eprintln!("Unimplemented RISC-V instruction: {:?}", instr);
            }
        }
    }

    fn branch_condition(&self, _ctx: &RoutineContext) -> Option<Expression> {
        // todo!("RISC-V branch conditions are not implemented yet")
        // eprintln!("Unimplemented RISC-V branch condition: {:?}", self.0);

        match &self.0.opc {
            OpcodeKind::BaseI(BaseIOpcode::BLT) => Some(_ctx.module.binary(
                _ctx.read_local(register_to_local_index(self.0.rs1.unwrap())),
                _ctx.read_local(register_to_local_index(self.0.rs2.unwrap())),
                BinaryOp::LtSInt64,
            )),
            _ => None,
        }
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
