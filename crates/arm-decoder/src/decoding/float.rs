use crate::{
    instructions::{
        BitfieldMoveMode, ConvertFPIntegerOp, FPMoveDirection, FPMoveFPSize, FPProcessingOp, FPToIntegerRoundingMode, Instruction, LogicalOp, SIMDTripleOp
    },
    structures::{Arrangement, FPSize, InstructionBytes, Shift, SizeVariant},
    utilities::equal_masked,
};

pub fn decode(bytes: InstructionBytes) -> Option<Instruction> {
    // Conversion between floating-point and integer
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Scalar-Floating-Point-and-Advanced-SIMD?lang=en#float2int
    if equal_masked(
        bytes.0,
        0b0101_1111_0010_0000_1111_1100_0000_0000,
        0b0001_1110_0010_0000_0000_0000_0000_0000,
    ) {
        let opcode = bytes.immediate_unsigned(16, 3);

        if opcode == 0b110 || opcode == 0b111 {
            return Some(Instruction::FPMove {
                destination: if opcode == 0b110 {
                    bytes.register(0, false)
                } else {
                    bytes.register_simd(0)
                },
                direction: if opcode == 0b110 {
                    FPMoveDirection::FPToInteger
                } else {
                    FPMoveDirection::IntegerToFP
                },
                fp_size: match (
                    bytes.immediate_unsigned(22, 2),
                    bytes.immediate_unsigned(19, 2),
                 ) {
                    (0b11, 0b00) => FPMoveFPSize::Half,
                    (0b00, 0b00) => FPMoveFPSize::Single,
                    (0b01, 0b00) => FPMoveFPSize::LowerDouble,
                    (0b01, 0b01) => FPMoveFPSize::UpperDouble,
                    _ => panic!(),
                },
                integer_size: bytes.variant(),
                operand: if opcode == 0b110 {
                    bytes.register_simd(5)
                } else {
                    bytes.register(5, false)
                }
            });
        }

        let (op, signed) = match (
            opcode,
            bytes.immediate_unsigned(19, 2)
         ) {
            (0b000, 0b00) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::Even,
            }, true),
            (0b001, 0b00) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::Even,
            }, false),
            (0b010, 0b00) => (ConvertFPIntegerOp::IntegerToFP, true),
            (0b011, 0b00) => (ConvertFPIntegerOp::IntegerToFP, false),
            (0b100, 0b00) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::Away,
            }, true),
            (0b101, 0b00) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::Away,
            }, false),
            (0b110, 0b00) => unreachable!(),
            (0b111, 0b00) => unreachable!(),
            (0b000, 0b01) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::PlusInfinity,
            }, true),
            (0b001, 0b01) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::PlusInfinity,
            }, false),
            (0b001, 0b10) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::MinusInfinity,
            }, false),
            (0b000, 0b10) => (ConvertFPIntegerOp::FPToInteger {
                rounding_mode: FPToIntegerRoundingMode::MinusInfinity,
            }, true),

            // FJCVTZS
            (0b110, 0b11) => todo!(),

            _ => todo!(),
        };

        return Some(Instruction::ConvertFPInteger {
            destination: if matches!(op, ConvertFPIntegerOp::FPToInteger { .. }) {
                bytes.register(0, false)
            } else {
                bytes.register_simd(0)
            },
            op,
            signed,
            fp_size: match bytes.immediate_unsigned(22, 2) {
                0b00 => FPSize::Single,
                0b01 => FPSize::Double,
                0b11 => FPSize::Half,
                _ => panic!(),
            },
            integer_size: bytes.variant(),
            operand: if matches!(op, ConvertFPIntegerOp::FPToInteger { .. }) {
                bytes.register_simd(5)
            } else {
                bytes.register(5, false)
            }
        });
    }

    // Floating-point data-processing (2 source)
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Scalar-Floating-Point-and-Advanced-SIMD?lang=en#floatdp2
    if equal_masked(
        bytes.0,
        0b1111_1111_0010_0000_0000_1100_0000_0000,
        0b0001_1110_0010_0000_0000_1000_0000_0000,
    ) {
        let size = match bytes.immediate_unsigned(22, 2) {
            0b00 => FPSize::Single,
            0b01 => FPSize::Double,
            0b11 => FPSize::Half,
            _ => panic!(),
        };

        let op = match bytes.immediate_unsigned(12, 4) {
            0b0000 => FPProcessingOp::Multiply,
            0b0001 => FPProcessingOp::Divide,
            0b0010 => FPProcessingOp::Add,
            0b0011 => FPProcessingOp::Subtract,
            0b0100 => FPProcessingOp::Maximum,
            0b0101 => FPProcessingOp::Minimum,
            0b0110 => FPProcessingOp::MaximumNumber,
            0b0111 => FPProcessingOp::MinimumNumber,
            0b1000 => FPProcessingOp::NegatedMultiply,
            _ => return None,
        };

        return Some(Instruction::FPProcessing {
            destination: bytes.register_simd(0),
            op,
            operand1: bytes.register_simd(5),
            operand2: bytes.register_simd(16),
            size,
        });
    }

    // Advanced SIMD three same
    // https://developer.arm.com/documentation/ddi0602/2026-03/Index-by-Encoding/Data-Processing----Scalar-Floating-Point-and-Advanced-SIMD?lang=en#asimdsame
    if equal_masked(
        bytes.0,
        0b1000_1111_0010_0000_0000_0100_0000_0000,
        0b0000_1110_0010_0000_0000_0100_0000_0000,
    ) {
        return Some(Instruction::Unknown);

        let is_scalar = bytes.bool(28);
        let u = bytes.bool(29);
        let size = bytes.immediate_unsigned(22, 2);
        let opcode = bytes.immediate_unsigned(11, 5);

        let op = match (u, size, opcode) {
            (false, _, 0b00000) => SIMDTripleOp::SignedHalvingAdd,
            (false, _, 0b00001) => SIMDTripleOp::SignedSaturatingAdd,
            (false, _, 0b00010) => SIMDTripleOp::SignedRoundingHalvingAdd,
            (false, 0b00, 0b00011) => SIMDTripleOp::BitwiseAnd,
            (false, 0b01, 0b00011) => SIMDTripleOp::BitwiseBitClear,
            (false, 0b10, 0b00011) => SIMDTripleOp::BitwiseOr,
            (false, 0b11, 0b00011) => SIMDTripleOp::BitwiseOrNot,
            (false, _, 0b00100) => SIMDTripleOp::SignedHalvingSubtract,
            (false, _, 0b00101) => SIMDTripleOp::SignedSaturatingSubtract,
            (false, _, 0b00110) => SIMDTripleOp::CompareSignedGT,
            (false, _, 0b00111) => SIMDTripleOp::CompareSignedGE,
            (false, _, 0b01000) => SIMDTripleOp::SignedShiftLeft,
            (false, _, 0b01001) => SIMDTripleOp::SignedSaturatingShiftLeft,
            (false, _, 0b01010) => SIMDTripleOp::SignedRoundingShiftLeft,
            (false, _, 0b01011) => SIMDTripleOp::SignedSaturatingRoundingShiftLeft,
            (false, _, 0b01100) => SIMDTripleOp::SignedMaximum,
            (false, _, 0b01101) => SIMDTripleOp::SignedMinimum,
            (false, _, 0b01110) => SIMDTripleOp::SignedAbsoluteDifference,
            (false, _, 0b01111) => SIMDTripleOp::SignedAbsoluteDifferenceAndAccumulate,
            (false, _, 0b10000) => SIMDTripleOp::Add,
            (false, _, 0b10001) => SIMDTripleOp::CompareBitwiseTestBitsNonzero,
            (false, _, 0b10010) => SIMDTripleOp::MultiplyAddAccumulate,
            (false, _, 0b10011) => SIMDTripleOp::Multiply,
            (false, _, 0b10100) => SIMDTripleOp::SignedMaximumPairwise,
            (false, _, 0b10101) => SIMDTripleOp::SignedMinimumPairwise,
            (false, _, 0b10110) => SIMDTripleOp::SignedSaturatingDoublingMultiplyReturningHighHalf,
            (false, _, 0b10111) => SIMDTripleOp::AddPairwise,
            (false, 0b00 | 0b01, 0b11000) => SIMDTripleOp::FPMaximumNumber,
            (false, 0b10 | 0b11, 0b11000) => SIMDTripleOp::FPMinimumNumber,
            (false, 0b00 | 0b01, 0b11001) => SIMDTripleOp::FPMultiplyAddAccumulate,
            (false, 0b10 | 0b11, 0b11001) => SIMDTripleOp::FPFusedMultiplySubtractFromAccumulator,
            (false, 0b00 | 0b01, 0b11010) => SIMDTripleOp::FPAdd,
            (false, 0b10 | 0b11, 0b11010) => SIMDTripleOp::FPSubtract,
            (false, 0b00 | 0b01, 0b11011) => SIMDTripleOp::FPMultiplyExtended,
            (false, 0b10 | 0b11, 0b11011) => SIMDTripleOp::FPAbsoluteMaximum,
            (false, 0b00 | 0b01, 0b11100) => SIMDTripleOp::FPCompareEqual,
            (false, 0b00, 0b11101) => SIMDTripleOp::FPFusedMultiplyAddLongToAccumulator,
            (false, 0b10, 0b11101) => SIMDTripleOp::FPFusedMultiplySubtractLongFromAccumulator,
            (false, 0b00 | 0b01, 0b11110) => SIMDTripleOp::FPMaximum,
            (false, 0b10 | 0b11, 0b11110) => SIMDTripleOp::FPMinimum,
            (false, 0b00 | 0b01, 0b11111) => SIMDTripleOp::FPReciprocalStep,
            (false, 0b10 | 0b11, 0b11111) => SIMDTripleOp::FPReciprocalSquareRootStep,

            (true, _, 0b00000) => SIMDTripleOp::UnsignedHalvingAdd,
            (true, _, 0b00001) => SIMDTripleOp::UnsignedSaturatingAdd,
            (true, _, 0b00010) => SIMDTripleOp::UnsignedRoundingHalvingAdd,
            (true, _, 0b00100) => SIMDTripleOp::UnsignedHalvingSubtract,
            (true, _, 0b00101) => SIMDTripleOp::UnsignedSaturatingSubtract,
            (true, _, 0b00110) => SIMDTripleOp::CompareUnsignedGT,
            (true, _, 0b00111) => SIMDTripleOp::CompareUnsignedGE,
            (true, _, 0b01000) => SIMDTripleOp::UnsignedShiftLeft,
            (true, _, 0b01001) => SIMDTripleOp::UnsignedSaturatingShiftLeft,
            (true, _, 0b01010) => SIMDTripleOp::UnsignedRoundingShiftLeft,
            (true, _, 0b01011) => SIMDTripleOp::UnsignedSaturatingRoundingShiftLeft,
            (true, _, 0b01100) => SIMDTripleOp::UnsignedMaximum,
            (true, _, 0b01101) => SIMDTripleOp::UnsignedMinimum,
            (true, _, 0b01110) => SIMDTripleOp::UnsignedAbsoluteDifference,
            (true, _, 0b01111) => SIMDTripleOp::UnsignedAbsoluteDifferenceAndAccumulate,
            (true, _, 0b10000) => SIMDTripleOp::Subtract,
            (true, _, 0b10001) => SIMDTripleOp::CompareEqual,
            (true, _, 0b10010) => SIMDTripleOp::MultiplySubtract,
            (true, _, 0b10011) => SIMDTripleOp::PolynomialMultiply,
            (true, _, 0b10100) => SIMDTripleOp::UnsignedMaximumPairwise,
            (true, _, 0b10101) => SIMDTripleOp::UnsignedMinimumPairwise,
            (true, _, 0b10110) => {
                SIMDTripleOp::SignedSaturatingRoundingDoublingMultiplyReturningHighHalf
            }
            (true, 0b00 | 0b01, 0b11000) => SIMDTripleOp::FPMaximumNumberPairwise,
            (true, 0b10 | 0b11, 0b11000) => SIMDTripleOp::FPMinimumNumberPairwise,
            (true, 0b00, 0b11001) => SIMDTripleOp::FPFusedMultiplyAddLongToAccumulatorUpperHalf,
            (true, 0b10, 0b11001) => {
                SIMDTripleOp::FPFusedMultiplySubtractLongFromAccumulatorUpperHalf
            }
            (true, 0b00 | 0b01, 0b11010) => SIMDTripleOp::FPAddPairwise,
            (true, 0b10 | 0b11, 0b11010) => SIMDTripleOp::FPAbsoluteDifference,
            (true, 0b00 | 0b01, 0b11011) => SIMDTripleOp::FPMultiply,
            (true, 0b10 | 0b11, 0b11011) => SIMDTripleOp::FPAbsoluteMinimum,
            (true, 0b00 | 0b01, 0b11100) => SIMDTripleOp::FPCompareGE,
            (true, 0b10 | 0b11, 0b11100) => SIMDTripleOp::FPCompareGT,
            (true, 0b00 | 0b01, 0b11101) => SIMDTripleOp::FPAbsoluteCompareGE,
            (true, 0b10 | 0b11, 0b11101) => SIMDTripleOp::FPAbsoluteCompareGT,
            (true, 0b00 | 0b01, 0b11110) => SIMDTripleOp::FPMaximumPairwise,
            (true, 0b10 | 0b11, 0b11110) => SIMDTripleOp::FPMinimumPairwise,
            (true, 0b00 | 0b01, 0b11111) => SIMDTripleOp::FPDivide,
            (true, 0b10 | 0b11, 0b11111) => SIMDTripleOp::FPScale,

            (true, 0b00, 0b00011) => SIMDTripleOp::BitwiseXor,
            (true, 0b01, 0b00011) => SIMDTripleOp::BitwiseSelect,
            (true, 0b10, 0b00011) => SIMDTripleOp::BitwiseInsertIfTrue,
            (true, 0b11, 0b00011) => SIMDTripleOp::BitwiseInsertIfFalse,

            _ => panic!(),
        };

        let arrangement = if is_scalar {
            match size {
                0b00 => Arrangement::B1,
                0b01 => Arrangement::H1,
                0b10 => Arrangement::S1,
                0b11 => Arrangement::D1,
                _ => unreachable!(),
            }
        } else {
            // eprintln!("Op: {:?}", op);

            match (size, bytes.bool(30)) {
                (0b00, false) => Arrangement::B8,
                (0b00, true) => Arrangement::B16,
                (0b01, false) => Arrangement::H4,
                (0b01, true) => Arrangement::H8,
                (0b10, false) => Arrangement::S2,
                (0b10, true) => Arrangement::S4,
                (0b11, false) => panic!(),
                (0b11, true) => Arrangement::D2,
                _ => unreachable!(),
            }
        };

        return Some(Instruction::SIMDTriple {
            arrangement,
            destination: bytes.register_simd(0),
            op,
            operand1: bytes.register_simd(5),
            operand2: bytes.register_simd(16),
        });
    }

    None
}
