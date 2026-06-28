use crate::value::GcClosure;
use compiler::op_code::Instructions;

#[derive(Debug, Clone)]
pub struct Frame {
    pub cl: GcClosure,
    pub ip: i32,
    pub base_pointer: usize,
    pub instructions: Vec<u8>,
}

impl Frame {
    pub fn new(closure: GcClosure, instructions: Vec<u8>, base_pointer: usize) -> Self {
        Frame {
            cl: closure,
            ip: -1,
            base_pointer,
            instructions,
        }
    }

    pub fn instruction_view(&self) -> Instructions {
        Instructions {
            data: self.instructions.clone(),
        }
    }
}
