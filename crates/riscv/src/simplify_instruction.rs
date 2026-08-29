use raki::{BaseIOpcode, COpcode, Instruction, OpcodeKind};

/// Zero register (`x0`).
const ZERO: usize = 0;
/// Return-address register (`ra` / `x1`).
const RA: usize = 1;
/// Stack pointer register (`sp` / `x2`).
const SP: usize = 2;

/// If `instr` is a compressed (C extension) instruction, returns the
/// equivalent full-size (32-bit) instruction. Returns `None` for
/// non-compressed instructions.
pub fn expand_compressed(instr: &Instruction) -> Option<Instruction> {
    let OpcodeKind::C(copc) = &instr.opc else {
        return None;
    };

    let (opc, rd, rs1, rs2, imm) = match copc {
        // Quadrant 0
        COpcode::ADDI4SPN => (
            BaseIOpcode::ADDI,
            instr.rd,
            Some(SP),
            None,
            instr.imm,
        ),
        COpcode::LW => (BaseIOpcode::LW, instr.rd, instr.rs1, None, instr.imm),
        COpcode::LD => (BaseIOpcode::LD, instr.rd, instr.rs1, None, instr.imm),
        COpcode::SW => (BaseIOpcode::SW, None, instr.rs1, instr.rs2, instr.imm),
        COpcode::SD => (BaseIOpcode::SD, None, instr.rs1, instr.rs2, instr.imm),

        // Quadrant 1
        COpcode::NOP => (
            BaseIOpcode::ADDI,
            Some(ZERO),
            Some(ZERO),
            None,
            Some(0),
        ),
        COpcode::ADDI => (BaseIOpcode::ADDI, instr.rd, instr.rs1, None, instr.imm),
        COpcode::ADDIW => (BaseIOpcode::ADDIW, instr.rd, instr.rs1, None, instr.imm),
        COpcode::JAL => (BaseIOpcode::JAL, Some(RA), None, None, instr.imm),
        COpcode::LI => (
            BaseIOpcode::ADDI,
            instr.rd,
            Some(ZERO),
            None,
            instr.imm,
        ),
        COpcode::ADDI16SP => (
            BaseIOpcode::ADDI,
            Some(SP),
            Some(SP),
            None,
            instr.imm,
        ),
        COpcode::LUI => (BaseIOpcode::LUI, instr.rd, None, None, instr.imm),
        COpcode::SRLI => (BaseIOpcode::SRLI, instr.rd, instr.rd, None, instr.imm),
        COpcode::SRAI => (BaseIOpcode::SRAI, instr.rd, instr.rd, None, instr.imm),
        COpcode::ANDI => (BaseIOpcode::ANDI, instr.rd, instr.rd, None, instr.imm),
        COpcode::SUB => (BaseIOpcode::SUB, instr.rd, instr.rd, instr.rs2, None),
        COpcode::XOR => (BaseIOpcode::XOR, instr.rd, instr.rd, instr.rs2, None),
        COpcode::OR => (BaseIOpcode::OR, instr.rd, instr.rd, instr.rs2, None),
        COpcode::AND => (BaseIOpcode::AND, instr.rd, instr.rd, instr.rs2, None),
        COpcode::SUBW => (BaseIOpcode::SUBW, instr.rd, instr.rd, instr.rs2, None),
        COpcode::ADDW => (BaseIOpcode::ADDW, instr.rd, instr.rd, instr.rs2, None),
        COpcode::J => (BaseIOpcode::JAL, Some(ZERO), None, None, instr.imm),
        COpcode::BEQZ => (BaseIOpcode::BEQ, None, instr.rs1, Some(ZERO), instr.imm),
        COpcode::BNEZ => (BaseIOpcode::BNE, None, instr.rs1, Some(ZERO), instr.imm),

        // Quadrant 2
        COpcode::SLLI => (BaseIOpcode::SLLI, instr.rd, instr.rd, None, instr.imm),
        COpcode::LWSP => (BaseIOpcode::LW, instr.rd, Some(SP), None, instr.imm),
        COpcode::LDSP => (BaseIOpcode::LD, instr.rd, Some(SP), None, instr.imm),
        COpcode::JR => (BaseIOpcode::JALR, Some(ZERO), instr.rs1, None, Some(0)),
        COpcode::MV => (
            BaseIOpcode::ADD,
            instr.rd,
            Some(ZERO),
            instr.rs2,
            None,
        ),
        COpcode::EBREAK => (BaseIOpcode::EBREAK, None, None, None, None),
        COpcode::JALR => (BaseIOpcode::JALR, Some(RA), instr.rs1, None, Some(0)),
        COpcode::ADD => (BaseIOpcode::ADD, instr.rd, instr.rd, instr.rs2, None),
        COpcode::SWSP => (BaseIOpcode::SW, None, Some(SP), instr.rs2, instr.imm),
        COpcode::SDSP => (BaseIOpcode::SD, None, Some(SP), instr.rs2, instr.imm),
    };

    let opc = OpcodeKind::BaseI(opc);
    let inst_format = opc.get_format();

    Some(Instruction {
        opc,
        rd,
        rs1,
        rs2,
        imm,
        inst_format,
        is_compressed: false,
    })
}
