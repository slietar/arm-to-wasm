use binaryen::ffi as by;
use capstone::prelude::*;


pub const REGISTER_COUNT: u32 = 32;

fn get_arm_operand(operand: &arch::ArchOperand) -> &arch::arm64::Arm64Operand {
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
pub struct Translator {
    pub module: by::BinaryenModuleRef,
}

impl Translator {
    fn get_reg_local_index(&self, reg_index: u16) -> u32 {
        (reg_index as u32) + 1
    }

    unsafe fn read_reg64(&self, reg_index: u16, use_zero_reg: bool) -> by::BinaryenExpressionRef {
        if use_zero_reg && reg_index == 31 {
            return unsafe { by::BinaryenConst(self.module, by::BinaryenLiteralInt64(0)) };
        }

        unsafe {
            by::BinaryenLocalGet(
                self.module,
                self.get_reg_local_index(reg_index),
                by::BinaryenInt64(),
            )
        }
    }

    fn read_reg64_from_operand(&self, operand: &arch::ArchOperand) -> by::BinaryenExpressionRef {
        let reg_index = get_register_id(operand);
        unsafe { self.read_reg64(reg_index, false) }
    }

    fn write_reg64_from_operand(
        &self,
        operand: &arch::ArchOperand,
        value: by::BinaryenExpressionRef,
    ) -> by::BinaryenExpressionRef {
        let reg_index = get_register_id(operand);
        unsafe { by::BinaryenLocalSet(self.module, self.get_reg_local_index(reg_index), value) }
    }

    fn get_i64_immediate_from_operand(
        &self,
        operand: &arch::ArchOperand,
    ) -> by::BinaryenExpressionRef {
        if let arch::arm64::Arm64OperandType::Imm(imm) = get_arm_operand(operand).op_type {
            unsafe { by::BinaryenConst(self.module, by::BinaryenLiteralInt64(imm as i64)) }
        } else {
            unreachable!()
        }
    }

    pub fn translate(
        &self,
        cs: &Capstone,
        instruction: &capstone::Insn,
    ) -> by::BinaryenExpressionRef {
        let detail: InsnDetail = cs.insn_detail(&instruction).unwrap();
        let arch_detail: ArchDetail = detail.arch_detail();
        let ops = arch_detail.operands();

        eprintln!("Instruction id: {}", instruction.id().0);
        eprintln!("Instruction mnemonic: {:?}", instruction.mnemonic());

        // for op in &ops {
        //     eprintln!("Operand: {:?}", op);
        // }

        match instruction.mnemonic().unwrap() {
            "add" => self.write_reg64_from_operand(&ops[0], unsafe {
                let op0 = get_arm_operand(&ops[0]);
                let op1 = get_arm_operand(&ops[1]);
                let op2 = get_arm_operand(&ops[2]);

                eprintln!("op0: {op0:?}");
                eprintln!("op1: {op1:?}");
                eprintln!("op2: {op2:?}");

                by::BinaryenBinary(
                    self.module,
                    by::BinaryenAddInt64(),
                    self.read_reg64_from_operand(&ops[1]),
                    self.get_i64_immediate_from_operand(&ops[2]),
                )
            }),
            "sub" => {
                // let p = arch_detail.arm64().unwrap();
                // eprintln!("Is this a SUB with carry? {}", p.update_flags());

                self.write_reg64_from_operand(&ops[0], unsafe {
                by::BinaryenBinary(
                    self.module,
                    by::BinaryenSubInt64(),
                    self.read_reg64_from_operand(&ops[1]),
                    self.get_i64_immediate_from_operand(&ops[2]),
                )
            })},
            _ => {
                unsafe { by::BinaryenNop(self.module) }
            }
        }
    }
}
