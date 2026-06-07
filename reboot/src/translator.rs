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
                self.module,
                self.get_reg_local_index(reg_id),
                by::BinaryenInt64(),
            )
        };

        if self.is_half_register(reg_id) {
            // Extract the lower 32 bits
            unsafe { by::BinaryenUnary(self.module, by::BinaryenWrapInt64(), full_register_expr) }
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
                    self.module,
                    by::BinaryenOrInt64(),
                    by::BinaryenUnary(self.module, by::BinaryenExtendUInt32(), value),
                    by::BinaryenBinary(
                        self.module,
                        by::BinaryenAndInt64(),
                        by::BinaryenLocalGet(self.module, local_index, by::BinaryenInt64()),
                        by::BinaryenConst(
                            self.module,
                            by::BinaryenLiteralInt64(u64::cast_signed(0xffff_ffff_0000_0000u64)),
                        ),
                    ),
                )
            }
        } else {
            value
        };

        unsafe { by::BinaryenLocalSet(self.module, self.get_reg_local_index(register_id), expr) }
    }

    fn get_i64_immediate_from_operand(
        &self,
        operand: &arch::ArchOperand,
        mode_32bit: bool,
    ) -> by::BinaryenExpressionRef {
        if let arch::arm64::Arm64OperandType::Imm(imm) = get_arm_operand(operand).op_type {
            let value = if mode_32bit {
                unsafe { by::BinaryenLiteralInt32(std::mem::transmute(imm as u32)) }
            } else {
                unsafe { by::BinaryenLiteralInt64(imm) }
            };

            unsafe { by::BinaryenConst(self.module, value) }
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
                self.get_i64_immediate_from_operand(operand, mode_32bit)
            }
            _ => unimplemented!(),
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
            "add" => {
                let mode_32bit = self.is_half_register(get_register_id(&ops[0]));

                self.write_register_from_operand(&ops[0], unsafe {
                    by::BinaryenBinary(
                        self.module,
                        by::BinaryenAddInt64(),
                        self.read_register_from_operand(&ops[1]),
                        self.get_i64_immediate_from_operand(&ops[2], mode_32bit),
                    )

                    // For ADC
                    // by::BinaryenBinary(
                    //     self.module,
                    //     by::BinaryenAddInt64(),
                    //     by::BinaryenBinary(
                    //         self.module,
                    //         by::BinaryenAddInt64(),
                    //         self.read_reg64_from_operand(&ops[1]),
                    //         self.get_i64_immediate_from_operand(&ops[2]),
                    //     ),
                    //     by::BinaryenLocalGet(self.module, CARRY_FLAG_LOCAL_INDEX, by::BinaryenInt32()),
                    // )
                })
            }
            "adrp" => self.write_register_from_operand(&ops[0], unsafe {
                by::BinaryenConst(self.module, by::BinaryenLiteralInt64(0xDEADBEEF))
            }),
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
            "sub" => {
                // let p = arch_detail.arm64().unwrap();
                // eprintln!("Is this a SUB with carry? {}", p.update_flags());

                let mode_32bit = self.is_half_register(get_register_id(&ops[0]));

                self.write_register_from_operand(&ops[0], unsafe {
                    by::BinaryenBinary(
                        self.module,
                        by::BinaryenSubInt64(),
                        self.read_register_from_operand(&ops[1]),
                        self.get_i64_immediate_from_operand(&ops[2], mode_32bit),
                    )
                })
            }
            _ => unsafe { by::BinaryenNop(self.module) },
        }
    }
}
