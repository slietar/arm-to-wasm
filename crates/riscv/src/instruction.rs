use aw_core::analysis::AnalysisContext;
use aw_core::architecture::{BranchKind, Instr, StackAccess};
use aw_core::translator::RoutineContext;
use bnyr::{BinaryOp, Expression, LoadVariant, StoreVariant};
use raki::{BaseIOpcode, Instruction, OpcodeKind};

use crate::arch::RiscV;
use crate::simplify_instruction::expand_compressed;

#[derive(Clone, Debug, Default)]
pub struct RiscVInstructionMetadata {
    call_routine_index: Option<usize>,
}

#[derive(Debug)]
pub struct RiscVInstruction(pub Instruction);

fn register_to_local_index(register: usize) -> u32 {
    (register as u32) - 1
}

impl RiscVInstruction {
    fn read_register(&self, ctx: &RoutineContext<RiscV>, register: usize) -> Expression {
        if register == crate::arch::ZERO {
            ctx.module.const_(0i64)
        } else {
            ctx.read_local(register_to_local_index(register))
        }
    }

    fn write_register(
        &self,
        ctx: &RoutineContext<RiscV>,
        register: usize,
        value: Expression,
    ) -> Expression {
        if register == crate::arch::ZERO {
            ctx.module.nop()
        } else {
            ctx.write_local(register_to_local_index(register), value)
        }
    }
}

impl Instr<RiscV> for RiscVInstruction {
    type Metadata = RiscVInstructionMetadata;

    fn size(&self) -> u64 {
        if self.0.is_compressed { 2 } else { 4 }
    }

    fn translate(
        &self,
        ctx: &RoutineContext<RiscV>,
        metadata: &Self::Metadata,
        current_address: u64,
        block_exprs: &mut Vec<Expression>,
    ) {
        let uncompressed_instr = expand_compressed(&self.0);
        let instr = uncompressed_instr.as_ref().unwrap_or(&self.0);

        if let Some(call_routine_index) = metadata.call_routine_index {
            ctx.call_routine(call_routine_index, block_exprs);
            return;
        }

        match &instr.opc {
            OpcodeKind::BaseI(BaseIOpcode::ADD) => block_exprs.push(self.write_register(
                ctx,
                instr.rd.unwrap(),
                ctx.module.binary(
                    self.read_register(ctx, instr.rs1.unwrap()),
                    self.read_register(ctx, instr.rs2.unwrap()),
                    BinaryOp::AddInt64,
                ),
            )),
            OpcodeKind::BaseI(BaseIOpcode::ADDI) => block_exprs.push(self.write_register(
                ctx,
                instr.rd.unwrap(),
                ctx.module.binary(
                    self.read_register(ctx, instr.rs1.unwrap()),
                    ctx.module.const_(instr.imm.unwrap() as i64),
                    BinaryOp::AddInt64,
                ),
            )),
            OpcodeKind::BaseI(BaseIOpcode::LD | BaseIOpcode::LW) => block_exprs.push(self.write_register(
                ctx,
                instr.rd.unwrap(),
                ctx.module.load(
                    match instr.opc {
                        OpcodeKind::BaseI(BaseIOpcode::LD) => LoadVariant::I64,
                        OpcodeKind::BaseI(BaseIOpcode::LW) => LoadVariant::I64L32 { signed: true },
                        _ => unreachable!(),
                    },
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
            OpcodeKind::BaseI(BaseIOpcode::SD | BaseIOpcode::SW) => block_exprs.push(ctx.module.store(
                match instr.opc {
                    OpcodeKind::BaseI(BaseIOpcode::SW) => StoreVariant::I64L32,
                    OpcodeKind::BaseI(BaseIOpcode::SD) => StoreVariant::I64,
                    _ => unreachable!(),
                },
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
                if instr.rd == Some(crate::arch::ZERO)
                    && instr.rs1 == Some(crate::arch::RA)
                    && instr.imm == Some(0) =>
            {
                block_exprs.push(ctx.return_());
            }

            OpcodeKind::BaseI(BaseIOpcode::ECALL) => {
                let syscall_name = "environment_call";

                block_exprs.push(self.write_register(
                    ctx,
                    crate::arch::A0,
                    ctx.module.call(
                        syscall_name,
                        &[
                            self.read_register(ctx, crate::arch::A7),
                            self.read_register(ctx, crate::arch::A0),
                            self.read_register(ctx, crate::arch::A1),
                            self.read_register(ctx, crate::arch::A2),
                            self.read_register(ctx, crate::arch::A3),
                            self.read_register(ctx, crate::arch::A4),
                            self.read_register(ctx, crate::arch::A5),
                        ],
                        ctx.module.i64(),
                    ),
                ));
            }

            OpcodeKind::BaseI(BaseIOpcode::AUIPC) => block_exprs.push(self.write_register(
                ctx,
                instr.rd.unwrap(),
                ctx.module.binary(
                    ctx.module.const_(current_address as i64),
                    ctx.module.const_(instr.imm.unwrap() as i64),
                    BinaryOp::AddInt64,
                ),
            )),

            _ => {
                eprintln!("Unimplemented RISC-V instruction: {:?}", instr);
            }
        }
    }

    fn branch_condition(&self, _ctx: &RoutineContext<RiscV>) -> Option<Expression> {
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

    fn branch_kind(
        &self,
        address: u64,
        prev_instruction: Option<&Self>,
        context: &AnalysisContext,
    ) -> (BranchKind, Self::Metadata) {
        let expanded = expand_compressed(&self.0);
        let instruction = expanded.as_ref().unwrap_or(&self.0);

        eprintln!("Instruction: {:?}", instruction);

        match &instruction.opc {
            OpcodeKind::BaseI(
                BaseIOpcode::BEQ
                | BaseIOpcode::BNE
                | BaseIOpcode::BLT
                | BaseIOpcode::BGE
                | BaseIOpcode::BLTU
                | BaseIOpcode::BGEU,
            ) => (
                BranchKind::Jump {
                    conditional: true,
                    target_address: address + (instruction.imm.unwrap() as u64),
                },
                Default::default(),
            ),

            // JAL saves the return address in `rd`. If `rd` is zero, the return
            // address is discarded.
            OpcodeKind::BaseI(BaseIOpcode::JAL) => match instruction.rd.unwrap() {
                crate::arch::ZERO => (
                    BranchKind::Jump {
                        conditional: false,
                        target_address: address + (instruction.imm.unwrap() as u64),
                    },
                    Default::default(),
                ),
                crate::arch::RA => {
                    let target_address = address + (instruction.imm.unwrap() as u64);
                    let target_routine_index = context.resolve_routine_jump(target_address);

                    assert!(target_routine_index.is_some());

                    // The return address is expected to be `ra`.
                    (
                        BranchKind::Call { target_address },
                        RiscVInstructionMetadata {
                            call_routine_index: target_routine_index,
                        },
                    )
                }
                _ => (BranchKind::Unknown, Default::default()),
            },

            OpcodeKind::BaseI(BaseIOpcode::JALR) => {
                let return_address_register = instruction.rd.unwrap();
                let target_address_register = instruction.rs1.unwrap();

                if (return_address_register != crate::arch::RA)
                    && (return_address_register != crate::arch::ZERO)
                {
                    return (BranchKind::Unknown, Default::default());
                }

                if let Some(
                    prev_instruction @ RiscVInstruction(Instruction {
                        opc: OpcodeKind::BaseI(BaseIOpcode::AUIPC),
                        rd: Some(prev_rd),
                        ..
                    }),
                ) = prev_instruction
                    && (*prev_rd == target_address_register)
                {
                    let target_address = (((address - (prev_instruction.size())) as i64)
                        + (instruction.imm.unwrap() as i64))
                        as u64;

                    let target_routine_index = context.resolve_routine_jump(target_address);

                    eprintln!("Computed target address: {:#x}", target_address);

                    match return_address_register {
                        crate::arch::RA if target_routine_index.is_some() => (
                            BranchKind::Call { target_address },
                            RiscVInstructionMetadata {
                                call_routine_index: target_routine_index,
                            },
                        ),
                        crate::arch::ZERO => (
                            BranchKind::Jump {
                                conditional: false,
                                target_address,
                            },
                            Default::default(),
                        ),
                        _ => unreachable!(),
                    }
                } else if (return_address_register == crate::arch::ZERO)
                    && (target_address_register == crate::arch::RA)
                    && (instruction.imm.unwrap() == 0)
                {
                    (BranchKind::Return, Default::default())
                } else {
                    (BranchKind::Unknown, Default::default())
                }
            }

            _ => (BranchKind::None, Default::default()),
        }
    }

    fn stack_frame_effect(&self) -> (Option<u64>, Vec<StackAccess>) {
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
