use binaryen::ffi as by;
use capstone::prelude::*;

pub const GP_REGISTER_COUNT: u32 = 31;

pub const SP_LOCAL_INDEX: u32 = 0;
pub const CARRY_FLAG_LOCAL_INDEX: u32 = 1;
pub const FIRST_GP_REGISTER_LOCAL_INDEX: u32 = 2;

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
    fn get_reg_local_index(&self, reg_id: u16) -> u32 {
        use arch::arm64::Arm64Reg::*;

        match reg_id as u32 {
            ARM64_REG_SP => SP_LOCAL_INDEX,

            ARM64_REG_X0 => FIRST_GP_REGISTER_LOCAL_INDEX + 0,
            ARM64_REG_X1 => FIRST_GP_REGISTER_LOCAL_INDEX + 1,
            ARM64_REG_X2 => FIRST_GP_REGISTER_LOCAL_INDEX + 2,
            ARM64_REG_X3 => FIRST_GP_REGISTER_LOCAL_INDEX + 3,
            ARM64_REG_X4 => FIRST_GP_REGISTER_LOCAL_INDEX + 4,
            ARM64_REG_X5 => FIRST_GP_REGISTER_LOCAL_INDEX + 5,
            ARM64_REG_X6 => FIRST_GP_REGISTER_LOCAL_INDEX + 6,
            ARM64_REG_X7 => FIRST_GP_REGISTER_LOCAL_INDEX + 7,
            ARM64_REG_X8 => FIRST_GP_REGISTER_LOCAL_INDEX + 8,
            ARM64_REG_X9 => FIRST_GP_REGISTER_LOCAL_INDEX + 9,
            ARM64_REG_X10 => FIRST_GP_REGISTER_LOCAL_INDEX + 10,
            ARM64_REG_X11 => FIRST_GP_REGISTER_LOCAL_INDEX + 11,
            ARM64_REG_X12 => FIRST_GP_REGISTER_LOCAL_INDEX + 12,
            ARM64_REG_X13 => FIRST_GP_REGISTER_LOCAL_INDEX + 13,
            ARM64_REG_X14 => FIRST_GP_REGISTER_LOCAL_INDEX + 14,
            ARM64_REG_X15 => FIRST_GP_REGISTER_LOCAL_INDEX + 15,
            ARM64_REG_X16 => FIRST_GP_REGISTER_LOCAL_INDEX + 16,
            ARM64_REG_X17 => FIRST_GP_REGISTER_LOCAL_INDEX + 17,
            ARM64_REG_X18 => FIRST_GP_REGISTER_LOCAL_INDEX + 18,
            ARM64_REG_X19 => FIRST_GP_REGISTER_LOCAL_INDEX + 19,
            ARM64_REG_X20 => FIRST_GP_REGISTER_LOCAL_INDEX + 20,
            ARM64_REG_X21 => FIRST_GP_REGISTER_LOCAL_INDEX + 21,
            ARM64_REG_X22 => FIRST_GP_REGISTER_LOCAL_INDEX + 22,
            ARM64_REG_X23 => FIRST_GP_REGISTER_LOCAL_INDEX + 23,
            ARM64_REG_X24 => FIRST_GP_REGISTER_LOCAL_INDEX + 24,
            ARM64_REG_X25 => FIRST_GP_REGISTER_LOCAL_INDEX + 25,
            ARM64_REG_X26 => FIRST_GP_REGISTER_LOCAL_INDEX + 26,
            ARM64_REG_X27 => FIRST_GP_REGISTER_LOCAL_INDEX + 27,
            ARM64_REG_X28 => FIRST_GP_REGISTER_LOCAL_INDEX + 28,
            ARM64_REG_X29 => FIRST_GP_REGISTER_LOCAL_INDEX + 29,
            ARM64_REG_X30 => FIRST_GP_REGISTER_LOCAL_INDEX + 30,
            _ => todo!(),
        }
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
        unsafe { self.read_reg64(get_register_id(operand), false) }
    }

    fn write_reg64_from_operand(
        &self,
        operand: &arch::ArchOperand,
        value: by::BinaryenExpressionRef,
    ) -> by::BinaryenExpressionRef {
        unsafe {
            by::BinaryenLocalSet(
                self.module,
                self.get_reg_local_index(get_register_id(operand)),
                value,
            )
        }
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

    pub fn setup(&self) -> by::BinaryenExpressionRef {
        unsafe {
            by::BinaryenLocalSet(
                self.module,
                CARRY_FLAG_LOCAL_INDEX,
                by::BinaryenConst(self.module, by::BinaryenLiteralInt32(0)),
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
                    by::BinaryenBinary(
                        self.module,
                        by::BinaryenAddInt64(),
                        self.read_reg64_from_operand(&ops[1]),
                        self.get_i64_immediate_from_operand(&ops[2]),
                    ),
                    by::BinaryenLocalGet(self.module, CARRY_FLAG_LOCAL_INDEX, by::BinaryenInt32()),
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
                })
            }
            _ => unsafe { by::BinaryenNop(self.module) },
        }
    }
}
