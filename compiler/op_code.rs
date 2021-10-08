use std::collections::HashMap;

// why not type, see https://stackoverflow.com/a/35569079/1713757
pub struct Instructions {
    ins: Vec<u8>,
}

pub type OpCode = u8;

pub struct Definition {
    name: &'static str,
    operand_width: Vec<i32>,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Opcode {
    OpConst,
    OpAdd
}

lazy_static! {
    static ref DEFINITIONS: HashMap<Opcode, Definition> = {
        let mut m = HashMap::new();
        m.insert(Opcode::OpConst, Definition {name: "OpConst", operand_width: vec![1]});
        m.insert(Opcode::OpAdd, Definition {name: "OpAdd", operand_width: vec![0]});
        m
    };
}

impl Instructions {
    pub fn make_instructions(op: Opcode, operands: &Vec<usize>) Instructions {
        let mut instructions = Vec::new();
    }

    // prettify bytecodes
    pub fn string(&self) -> String {
        let mut ret = String::new();
        
        return ret;
    }

}