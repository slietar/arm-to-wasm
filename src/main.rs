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
    // 31 = Zero
    FormPCRelativeAddress {
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


fn main() {
    // let mut f = File::open("/bin/sh").unwrap();
    // let mut f = File::open("molcv").unwrap();
    // let mut f = File::open("/opt/homebrew/lib/python3.11/site-packages/numpy/random/_bounded_integers.cpython-311-darwin.so").unwrap();
    let mut f = File::open("simple-lib/target/debug/simple-lib").unwrap();

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
            } else if (instr >> 24) & 0b1001_1111 == 0b1001_0000 {
                // ADRP, p. 1611

                instructions.push(Instruction::FormPCRelativeAddress {
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
                    shift: hw << 4,
                    variant64bit,
                });
            } else if instr >> 26 == 0b10_0101 {
                // BL, p. 1653

                instructions.push(Instruction::BranchWithLink {
                    offset: (instr & 0b11_1111_1111_1111_1111_1111_1111) << 2,
                });
            } else if (instr >> 22) & 0b1_1111_1001 == 0b0_1010_0001 {
                // LDP, p. 1999

                instructions.push(Instruction::LoadPairOfRegisters {
                    addressing_mode: AddressingMode::parse((instr >> 23) & 0b11),
                    imm_offset: (instr >> 15) & 0b111_1111,
                    reg_base: Register((instr >> 5) & register_mask),
                    reg_transferred1: Register(instr & register_mask),
                    reg_transferred2: Register((instr >> 10) & register_mask),
                    variant64bit: (instr >> 31) > 0,
                });
            } else {
                eprintln!("unknown instruction");
                eprintln!("{:?}", instr >> 23);
            }
        }

        eprintln!("{:#?}", instructions);

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
