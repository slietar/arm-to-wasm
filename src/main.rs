use std::io::{Read, Cursor};
use std::fs::File;
use mach_object::{OFile, MachCommand, LoadCommand};


#[derive(Debug)]
enum Instruction {
    // STP
    StorePairOfRegisters {
        imm: i32,
        rn: Register,
        rt1: Register,
        rt2: Register,
    },

    // ORR
    // 31 = Zero
    BitwiseOr {
        imm: u32,
        reg_dest: Register,
        reg_nonshifted: Register,
        reg_shifted: Register,
        shift: Shift,
        variant64bit: bool,
    },

    // ADD
    // 31 = Sp
    AddImmediate {
        imm: u32,
        reg_dest: Register,
        reg_source: Register,
        shifted: bool,
        variant64bit: bool,
    },

    // SBFM
    // 31 = Zero
    SignedBitfieldMove {
        imm_source: u32,
        imm_rotate: u32,
        reg_dest: Register,
        reg_source: Register,
        variant64bit: bool,
    },

    // ADRP
    FormPCRelativeAddress {
        imm_high: u32,
        imm_low: u32,
        reg_dest: Register,
    },

    // ADRP
    FormPCRelativeAddressToPage {
        imm_high: u32,
        imm_low: u32,
        reg_dest: Register,
    },

    // MOVZ
    // 31 = Zero
    MoveWideWithZero {
        reg_dest: Register,
        imm: u32,
        shift: u32,
        variant64bit: bool,
    },

    // BL
    BranchWithLink {
        offset: u32,
    },

    // LDP
    LoadPairOfRegisters {
        addressing_mode: AddressingMode,
        imm_offset: u32,
        reg_base: Register, // 31 = Sp
        reg_transferred1: Register,
        reg_transferred2: Register,
        variant64bit: bool,
    },

    // LDNP
    LoadPairOfRegistersNontemporalHint {
        imm_offset: u32,
        reg_base: Register, // 31 = Sp
        reg_transferred1: Register,
        reg_transferred2: Register,
        variant64bit: bool,
    },

    // LDNP SIMD&FP
    LoadPairOfRegistersFloatNontemporalHint {
        imm_offset: u32,
        reg_base: Register, // 31 = Sp
        reg_transferred1: Register,
        reg_transferred2: Register,
        variant: Variant,
    },

    // SVC
    SupervisorCall {
        imm: u32,
    },

    // UMLAL, UMLAL2
    UnsignedMultiplyAddLongVector {
        arr_dest: HalfArrangement,
        arr_source: FullArrangement,
        reg_dest: Register,
        reg_source1: Register,
        reg_source2: Register,
        upper: bool,
    },

    // UMLAL, UMLAL2
    UnsignedMultiplyAddLongByElement {
        arr_dest: HalfArrangement,
        arr_source: FullArrangement,
        index: u32,
        reg_dest: Register,
        reg_source1: Register,
        reg_source2: Register,
        upper: bool,
    },
}


// p. 322
// Wn => 0-30, 32 bits
// Xn => 0-30, 64 bits
// WSP, WZR => 31, 32 bits
// SP, XZR => 31, 64 bits
#[derive(Debug)]
struct Register(u32);


#[derive(Debug)]
enum AddressingMode {
    PostIndex,
    PreIndex,
    SignedOffset,
}

impl AddressingMode {
    fn parse(value: u32) -> Self {
        match value {
            0b01 => AddressingMode::PostIndex,
            0b10 => AddressingMode::SignedOffset,
            0b11 => AddressingMode::PreIndex,
            _ => unreachable!(),
        }
    }
}


#[derive(Debug)]
enum FullArrangement {
    B8,
    B16,
    H4,
    H8,
    S2,
    S4,
}

impl FullArrangement {
    fn parse(value: u32, q: u32) -> Self {
        match (value, q) {
            (0b00, 0b0) => FullArrangement::B8,
            (0b00, 0b1) => FullArrangement::B16,
            (0b01, 0b0) => FullArrangement::H4,
            (0b01, 0b1) => FullArrangement::H8,
            (0b10, 0b0) => FullArrangement::S2,
            (0b10, 0b1) => FullArrangement::S4,
            _ => unreachable!(),
        }
    }
}


#[derive(Debug)]
enum HalfArrangement {
    D2,
    H8,
    S4,
}

impl HalfArrangement {
    fn parse(value: u32) -> Self {
        match value {
            0b00 => HalfArrangement::H8,
            0b01 => HalfArrangement::S4,
            0b10 => HalfArrangement::D2,
            _ => unreachable!(),
        }
    }
}


#[derive(Debug)]
enum Shift {
    LSL,
    LSR,
    ASR,
    ROR
}

impl Shift {
    fn parse(value: u32) -> Self {
        match value {
            0b00 => Shift::LSL,
            0b01 => Shift::LSR,
            0b10 => Shift::ASR,
            0b11 => Shift::ROR,
            _ => unreachable!(),
        }
    }
}


#[derive(Debug)]
enum Variant {
    V32,
    V64,
    V128,
}


fn main() {
    // let mut f = File::open("/bin/sh").unwrap();
    // let mut f = File::open("molcv").unwrap();
    // let mut f = File::open("/opt/homebrew/lib/python3.11/site-packages/numpy/random/_bounded_integers.cpython-311-darwin.so").unwrap();
    // let mut f = File::open("simple-lib/target/debug/simple-lib").unwrap();
    let mut f = File::open("hello").unwrap();

    let mut buf = Vec::new();
    let size = f.read_to_end(&mut buf).unwrap();
    let mut cur = Cursor::new(&buf[..size]);

    let file = OFile::parse(&mut cur).unwrap();

    // println!("{:#?}", file);

    if let OFile::MachFile { ref header, ref commands } = file {
/*         for &MachCommand(ref cmd, cmdsize) in commands {
            // if let &LoadCommand::Segment64 { ref segname, ref sections, .. } = cmd {
            //     println!("segment: {}", segname);

            //     for ref sect in sections {
            //         println!("  section: {}", sect.sectname);
            //     }
            // }

            if let &LoadCommand::SymTab { nsyms, stroff, strsize, symoff } = cmd {
                // println!("{:#?}", cmd);

                // let x = &buf[stroff as usize..(stroff + 200) as usize];
                // eprintln!("{:?}", String::from_utf8_lossy(x));
                // eprintln!("{:?}", x);

                let mut c = Vec::new();

                let entry_size = 16;

                for entry_index in 0..nsyms {
                    let offset = (symoff + entry_index * entry_size) as usize;
                    let str_offset = (stroff + u32::from_le_bytes(buf[offset..(offset + 4)].try_into().unwrap())) as usize;
                    let str_length = buf[str_offset..].iter().position(|&r| r == b'\0').unwrap();
                    let name = unsafe { String::from_utf8_unchecked(buf[str_offset..(str_offset + str_length)].to_vec()) };
                    // let name = String::from_utf8_lossy(&buf[str_offset..(str_offset + str_length)]);

                    if !name.starts_with("__") {
                        // let x = &buf[(symoff + entry_index * entry_size) as usize..(symoff + (entry_index + 1) * entry_size) as usize];
                        // eprintln!("{:x?}", x);

                        println!("{}", name);
                    }

                    c.push(str_offset);
                }

                eprintln!("{:?}", strsize);
                eprintln!("{:?}", nsyms);
                eprintln!("{:?}", c.iter().max());
            }
        } */


        // let mut mem = [0u8; 5 * (1 << 30)];
        let mut mem = vec![0u8; 5 * (1 << 30)];

        for &MachCommand(ref cmd, _) in commands {
            match cmd {
                LoadCommand::Segment64 { segname, vmaddr, vmsize, fileoff, filesize, maxprot, initprot, flags, sections } => {
                    // eprintln!("segment: {}", segname);
                    // eprintln!("  ({} -> {})", vmaddr, vmsize);

                    for ref sect in sections {
                        // eprintln!("  section: {}", sect.sectname);
                        // eprintln!("    {}, {}", sect.addr, sect.size);
                        // eprintln!("    {}", sect.offset);

                        if sect.size > 0 && sect.offset > 0 {
                            mem[(sect.addr as usize)..(sect.addr as usize + sect.size as usize)].copy_from_slice(&buf[(sect.offset as usize)..(sect.offset as usize + sect.size as usize)]);
                        }
                    }
                },
                _ => {},
            }
        }


        let mut entry_pointer = None;

        for &MachCommand(ref cmd, _) in commands {
            match cmd {
                LoadCommand::EntryPoint { entryoff, stacksize } => {
                    entry_pointer = Some(entryoff);
                    // eprintln!("stack: {}", stacksize);
                    break;
                },
                _ => {},
            }
        }

        let pointer = *entry_pointer.unwrap() as usize + (1 << 32) + 0;
        // eprintln!("{:?}", pointer);


        // let opc = (instr >> 23) & 0b11111111;
        // eprintln!("{:?}", opc);

        let mut instructions = Vec::new();

        let register_mask = 0b11111;

        for instruction_index in 0..10 {
            let instr = u32::from_le_bytes(mem[(pointer + instruction_index * 4)..(pointer + (instruction_index + 1) * 4)].try_into().unwrap());


            if (instr >> 22) & 0b11111111 == 0b010100110 {
                // STP, p. 2332

                let imm7 = (instr >> 15) & 0b1111111;
                let variant64bit = (instr >> 31) > 0;

                instructions.push(Instruction::StorePairOfRegisters {
                    imm: (imm7 << 3) as i32,
                    rt1: Register(instr & register_mask),
                    rt2: Register((instr >> 10) & register_mask),
                    rn: Register((instr >> 5) & register_mask),
                });
            } else if (instr >> 21) & 0b1111111001 == 0b0101010000 {
                // ORR, p. 2141

                let variant64bit = (instr >> 31) > 0;
                let imm6 = (instr >> 10) & 0b111111;
                let shift = Shift::parse((instr >> 22) & 0b11);

                if !variant64bit {
                    assert_eq!(imm6 >> 5, 0);
                }

                instructions.push(Instruction::BitwiseOr {
                    imm: imm6,
                    reg_dest: Register(instr & register_mask),
                    reg_shifted: Register((instr >> 16) & register_mask),
                    reg_nonshifted: Register((instr >> 5) & register_mask),
                    shift,
                    variant64bit,
                });
            } else if (instr >> 23) & 0b1111_1111 == 0b0010_0010 {
                // ADD, p. 1597

                let variant64bit = (instr >> 31) > 0;

                instructions.push(Instruction::AddImmediate {
                    imm: (instr >> 10) & 0b11_1111_1111,
                    reg_dest: Register(instr & register_mask),
                    reg_source: Register((instr >> 5) & register_mask),
                    shifted: (instr >> 23) & 0b1 > 0,
                    variant64bit,
                });
            } else if (instr >> 23) & 0b1111_1111 == 0b0010_0110 {
                // SBFM, p. 2227

                let variant64bit = (instr >> 31) > 0;
                let imm_rotate = (instr >> 16) & 0b11111;
                let imm_source = (instr >> 10) & 0b11111;
                let n = (instr >> 22) & 0b1;

                if variant64bit {
                    assert!(n == 1);
                } else {
                    assert!(n == 0);
                    assert!(imm_rotate == 0);
                    assert!(imm_source == 0);
                }

                instructions.push(Instruction::SignedBitfieldMove {
                    imm_source,
                    imm_rotate,
                    reg_dest: Register(instr & register_mask),
                    reg_source: Register((instr >> 5) & register_mask),
                    variant64bit,
                });
            } else if (instr >> 24) & 0b1001_1111 == 0b0001_0000 {
                // ADR, p. 1610

                instructions.push(Instruction::FormPCRelativeAddress {
                    imm_high: (instr >> 5) & 0b111_1111_1111_1111_1111,
                    imm_low: (instr >> 29) & 0b11,
                    reg_dest: Register(instr & register_mask),
                });
            } else if (instr >> 24) & 0b1001_1111 == 0b1001_0000 {
                // ADRP, p. 1611

                instructions.push(Instruction::FormPCRelativeAddressToPage {
                    imm_high: (instr >> 5) & 0b111_1111_1111_1111_1111,
                    imm_low: (instr >> 29) & 0b11,
                    reg_dest: Register(instr & register_mask),
                });
            } else if (instr >> 23) & 0b1111_1111 == 0b1010_0101 {
                // MOVZ, p. 2113

                let variant64bit = (instr >> 31) > 0;
                let hw = (instr >> 21) & 0b11;

                if variant64bit {
                    assert_eq!(hw, 0);
                }

                instructions.push(Instruction::MoveWideWithZero {
                    reg_dest: Register(instr & register_mask),
                    imm: (instr >> 5) & 0b1111_1111_1111_1111,
                    shift: hw,
                    variant64bit,
                });
            } else if instr >> 26 == 0b10_0101 {
                // BL, p. 1653

                instructions.push(Instruction::BranchWithLink {
                    offset: (instr & 0b11_1111_1111_1111_1111_1111_1111) << 2,
                });
            } else if (instr >> 22) & 0b1_1111_1001 == 0b0_1010_0001 {
                // LDNP, p. 1997
                // LDP, p. 1999

                let mode = (instr >> 23) & 0b11;

                if mode == 0b00 {
                    instructions.push(Instruction::LoadPairOfRegistersNontemporalHint {
                        imm_offset: (instr >> 15) & 0b111_1111,
                        reg_base: Register((instr >> 5) & register_mask),
                        reg_transferred1: Register(instr & register_mask),
                        reg_transferred2: Register((instr >> 10) & register_mask),
                        variant64bit: (instr >> 31) > 0,
                    });
                } else {
                    instructions.push(Instruction::LoadPairOfRegisters {
                        addressing_mode: AddressingMode::parse((instr >> 23) & 0b11),
                        imm_offset: (instr >> 15) & 0b111_1111,
                        reg_base: Register((instr >> 5) & register_mask),
                        reg_transferred1: Register(instr & register_mask),
                        reg_transferred2: Register((instr >> 10) & register_mask),
                        variant64bit: (instr >> 31) > 0,
                    });
                }
            } else if (instr >> 22) & 0b1111_1111 == 0b1011_0001 {
                // LDNP SIMD&FP, p. 2927

                instructions.push(Instruction::LoadPairOfRegistersFloatNontemporalHint {
                    imm_offset: (instr >> 15) & 0b111_1111,
                    reg_base: Register((instr >> 5) & register_mask),
                    reg_transferred1: Register(instr & register_mask),
                    reg_transferred2: Register((instr >> 10) & register_mask),
                    variant: match instr >> 30 {
                        0b00 => Variant::V32,
                        0b01 => Variant::V64,
                        0b10 => Variant::V128,
                        _ => unreachable!(),
                    },
                });
            } else if instr & 0b1111_1111_1110_0000_0000_0000_0001_1111 == 0b1101_0100_0000_0000_0000_0000_0000_0001 {
                // SVC, p. 2411

                instructions.push(Instruction::SupervisorCall {
                    imm: (instr >> 5) & 0b1111_1111_1111_1111,
                });
            } else if (instr >> 10) & 0b10_1111_1100_1000_0011_1111 == 0b00_1011_1000_1000_0010_0000 {
                // UMLAL, UMLAL2 vector, p. 3330

                let q = (instr >> 30) & 0b1;

                instructions.push(Instruction::UnsignedMultiplyAddLongVector {
                    arr_dest: HalfArrangement::parse((instr >> 22) & 0b11),
                    arr_source: FullArrangement::parse((instr >> 10) & 0b11, q),
                    reg_dest: Register(instr & register_mask),
                    reg_source1: Register((instr >> 5) & register_mask),
                    reg_source2: Register((instr >> 16) & register_mask),
                    upper: q > 0,
                });
            } else if (instr >> 10) & 0b10_1111_1100_0000_0011_1101 == 0b00_1011_1100_0000_0000_1000 {
                // UMLAL, UMLAL2 by element, p. 3327

                let q = (instr >> 30) & 0b1;
                let size = (instr >> 22) & 0b11;
                let rm = (instr >> 16) & 0b1111;
                let m = (instr >> 20) & 0b1;
                let hl = (((instr >> 11) & 0b1) << 1) + ((instr >> 21) & 0b1);

                instructions.push(Instruction::UnsignedMultiplyAddLongByElement {
                    arr_dest: HalfArrangement::parse((instr >> 22) & 0b11),
                    arr_source: FullArrangement::parse((instr >> 10) & 0b11, q),
                    index: match size {
                        0b01 => hl,
                        0b10 => (hl << 1) + m,
                        _ => unreachable!(),
                    },
                    reg_dest: Register(instr & register_mask),
                    reg_source1: Register((instr >> 5) & register_mask),
                    reg_source2: match size {
                        0b01 => Register(rm),
                        0b10 => Register(rm + m << 5),
                        _ => unreachable!(),
                    },
                    upper: q > 0,
                });
            } else {
                eprintln!("unknown instruction");
                eprintln!("{:?}", instr);
            }
        }


        let mut registers = [0u64; 32];

        for instruction in instructions {
            match instruction {
                Instruction::MoveWideWithZero { reg_dest, imm, shift, variant64bit } => {
                    registers[reg_dest.0 as usize] = (imm as u64) << (shift << 4);
                },
                Instruction::FormPCRelativeAddress { imm_high, imm_low, reg_dest } => {
                    registers[reg_dest.0 as usize] = (imm_high as u64) << 12 + imm_low as u64;
                },
                _ => panic!("unknown instruction: {:?}", instruction),
            }
        }

        // eprintln!("{:#?}", instructions);

        // let op = instr >> 27;
        // let vr = (instr >> 23) & 0b1111;
        // let l = (instr >> 22) & 0b1;
        // let imm7 = (instr >> 15) & 0b1111111;
        // let rt2 = (instr >> 10) & 0b11111;
        // let rn = (instr >> 5) & 0b11111;
        // let rt = instr & 0b11111;

        // eprintln!("{:?}", op);
        // eprintln!("vr={}, l={}, imm7={}, rt2={}, rn={}, rt={}", vr, l, imm7, rt2, rn, rt);
    }
}
