use std::collections::HashMap;

use byteorder;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};

use strum::{EnumCount, EnumIter};

// why not type, see https://stackoverflow.com/a/35569079/1713757
#[derive(Hash, Eq, Debug, Clone, PartialEq, PartialOrd)]
pub struct Instructions {
    pub data: Vec<u8>,
}

pub struct OpcodeDefinition {
    name: &'static str,
    operand_width: Vec<i32>,
}

#[repr(u8)]
#[derive(Debug, Hash, Eq, Clone, Copy, PartialEq, EnumCount, EnumIter)]
pub enum Opcode {
    OpConst,
    OpAdd,
    OpPop,
    OpSub,
    OpMul,
    OpDiv,
    OpTrue,
    OpFalse,
    OpEqual,
    OpNotEqual,
    OpGreaterThan,
    OpMinus,
    OpBang,
    OpJumpNotTruthy,
    OpJump,
    OpNull,
    OpGetGlobal,
    OpSetGlobal,
}

lazy_static! {
    pub static ref DEFINITIONS: HashMap<Opcode, OpcodeDefinition> = {
        let mut m = HashMap::new();
        m.insert(Opcode::OpConst, OpcodeDefinition { name: "OpConst", operand_width: vec![2] });
        m.insert(Opcode::OpAdd, OpcodeDefinition { name: "OpAdd", operand_width: vec![] });
        m.insert(Opcode::OpPop, OpcodeDefinition { name: "OpPop", operand_width: vec![] });
        m.insert(Opcode::OpSub, OpcodeDefinition { name: "OpSub", operand_width: vec![] });
        m.insert(Opcode::OpMul, OpcodeDefinition { name: "OpMul", operand_width: vec![] });
        m.insert(Opcode::OpDiv, OpcodeDefinition { name: "OpDiv", operand_width: vec![] });
        m.insert(Opcode::OpTrue, OpcodeDefinition { name: "OpTrue", operand_width: vec![] });
        m.insert(Opcode::OpFalse, OpcodeDefinition { name: "OpFalse", operand_width: vec![] });
        m.insert(Opcode::OpEqual, OpcodeDefinition { name: "OpEqual", operand_width: vec![] });
        m.insert(
            Opcode::OpNotEqual,
            OpcodeDefinition { name: "OpNotEqual", operand_width: vec![] },
        );
        m.insert(
            Opcode::OpGreaterThan,
            OpcodeDefinition { name: "OpGreatThan", operand_width: vec![] },
        );
        m.insert(Opcode::OpMinus, OpcodeDefinition { name: "OpMinus", operand_width: vec![] });
        m.insert(Opcode::OpBang, OpcodeDefinition { name: "OpBang", operand_width: vec![] });
        m.insert(
            Opcode::OpJumpNotTruthy,
            OpcodeDefinition { name: "OpJumpNotTruthy", operand_width: vec![2] },
        );
        m.insert(Opcode::OpJump, OpcodeDefinition { name: "OpJump", operand_width: vec![2] });
        m.insert(Opcode::OpNull, OpcodeDefinition { name: "OpNull", operand_width: vec![] });
        m.insert(
            Opcode::OpGetGlobal,
            OpcodeDefinition { name: "OpGetGlobal", operand_width: vec![2] },
        );
        m.insert(
            Opcode::OpSetGlobal,
            OpcodeDefinition { name: "OpSetGlobal", operand_width: vec![2] },
        );
        m
    };
}

pub fn make_instructions(op: Opcode, operands: &Vec<usize>) -> Instructions {
    let mut instructions = Vec::new();
    instructions.push(op as u8);
    let widths = &DEFINITIONS.get(&op).unwrap().operand_width;

    for (o, w) in operands.into_iter().zip(widths) {
        match w {
            2 => {
                instructions.write_u16::<BigEndian>(*o as u16).unwrap();
            }
            1 => {
                instructions.write_u8(*o as u8).unwrap();
            }
            _ => {
                panic!("unsupported operand width {}", w)
            }
        }
    }

    return Instructions { data: instructions };
}

pub fn read_operands(def: &OpcodeDefinition, ins: &[u8]) -> (Vec<usize>, usize) {
    let mut operands = Vec::with_capacity(def.operand_width.len());
    let mut offset = 0;

    for w in &def.operand_width {
        match w {
            2 => {
                operands.push(BigEndian::read_u16(&ins[offset..offset + 2]) as usize);
                offset = offset + 2;
            }
            1 => {
                operands.push(ins[offset] as usize);
                offset = offset + 1;
            }
            0 => {}
            _ => {
                panic!("unsupported operand width {} for read", w)
            }
        }
    }

    return (operands, offset);
}

pub fn concat_instructions(expected: &Vec<Instructions>) -> Instructions {
    let mut out = Instructions { data: vec![] };

    for instruction in expected {
        out = out.merge_instructions(instruction)
    }

    return out;
}

pub fn cast_u8_to_opcode(op: u8) -> Opcode {
    // https://stackoverflow.com/a/42382144/1713757
    return unsafe { ::std::mem::transmute(op) };
}

impl Instructions {
    // prettify bytecodes
    pub fn string(&self) -> String {
        let mut ret = String::new();
        let mut i = 0;
        while i < self.data.len() {
            let op: u8 = *self.data.get(i).unwrap();
            let opcode = cast_u8_to_opcode(op);

            let definition = DEFINITIONS.get(&opcode).unwrap();
            let (operands, read_size) = read_operands(definition, &self.data[i + 1..]);
            ret.push_str(&format!("{:04} {}\n", i, Self::fmt_instructions(definition, &operands)));
            i = i + 1 + read_size;
        }

        return ret;
    }

    fn fmt_instructions(def: &OpcodeDefinition, operands: &Vec<usize>) -> String {
        match def.operand_width.len() {
            2 => format!("{} {} {}", def.name, operands[0], operands[1]),
            1 => format!("{} {}", def.name, operands[0]),
            0 => format!("{}", def.name),
            _ => {
                panic!("unsupported operand width {}", def.operand_width.len());
            }
        }
    }

    pub fn merge_instructions(&self, other: &Instructions) -> Instructions {
        let ins = vec![self, other];
        // Maybe extend_from_slice, but I have not make it work
        // https://stackoverflow.com/a/69578632/1713757
        return Instructions {
            data: ins
                .iter()
                .fold(vec![], |sum, &i| [sum.as_slice(), i.data.as_slice()].concat()),
        };
    }
}
