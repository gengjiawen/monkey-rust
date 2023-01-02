use crate::op_code::Instructions;
use object::CompiledFunction;

#[derive(Debug, Clone)]
pub struct Frame {
    func: CompiledFunction,
    pub ip: i32,
    pub base_pointer: usize,
}

impl Frame {
    pub fn new(func: CompiledFunction, base_pointer: usize) -> Self {
        Frame { func, ip: -1, base_pointer }
    }

    pub fn instructions(&self) -> Instructions {
        return Instructions { data: self.func.instructions.clone() };
    }
}
