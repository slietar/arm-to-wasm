use std::io::{Read, Cursor};
use std::fs::File;
use mach_object::{OFile, MachCommand, LoadCommand};


#[derive(Debug)]
enum Instruction {
    StorePairOfRegisters {
        imm: i32,
        rn: Register,
        rt1: Register,
        rt2: Register,
    },

    // 31 = Zero
    BitwiseOr {
        imm: u32,
        reg_dest: Register,
        reg_nonshifted: Register,
        reg_shifted: Register,
        shift: Shift,
    },

    AddImmediate {
        imm: u32,
        reg_dest: Register,
        reg_source: Register,
        shifted: bool,
    },
}


#[derive(Debug)]
struct Register {
    // p. 322

    // Wn => 0-30, 32 bits
    // Xn => 0-30, 64 bits
    // WSP => 31, 32 bits
    // SP => 31, 64 bits
    id: u32,
    half: bool,
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

        for instruction_index in 0..4 {
            let instr = u32::from_le_bytes(mem[(pointer + instruction_index * 4)..(pointer + (instruction_index + 1) * 4)].try_into().unwrap());


            if (instr >> 22) & 0b11111111 == 0b010100110 {
                // STP, p. 2332

                let imm7 = (instr >> 15) & 0b1111111;
                let rt2 = (instr >> 10) & register_mask;
                let rn = (instr >> 5) & register_mask;
                let rt = instr & register_mask;
                let variant64bit = (instr >> 31) > 0;

                instructions.push(Instruction::StorePairOfRegisters {
                    imm: (imm7 << 3) as i32,
                    rt1: Register { id: rt, half: !variant64bit },
                    rt2: Register { id: rt2, half: !variant64bit },
                    rn: Register { id: rn, half: !variant64bit },
                });
            } else if (instr >> 21) & 0b1111111001 == 0b0101010000 {
                // ORR, p. 2141

                let variant64bit = (instr >> 31) > 0;
                let rm = (instr >> 16) & register_mask;
                let imm6 = (instr >> 10) & 0b111111;
                let rn = (instr >> 5) & register_mask;
                let rt = instr & register_mask;
                let shift = Shift::parse((instr >> 22) & 0b11);

                if !variant64bit {
                    assert_eq!(imm6 >> 5, 0);
                }

                instructions.push(Instruction::BitwiseOr {
                    imm: imm6,
                    reg_dest: Register { id: rt, half: !variant64bit },
                    reg_shifted: Register { id: rm, half: !variant64bit },
                    reg_nonshifted: Register { id: rn, half: !variant64bit },
                    shift,
                });
            } else if (instr >> 23) & 0b11111111 == 0b00100010 {

            } else {
                eprintln!("unknown instruction");
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
