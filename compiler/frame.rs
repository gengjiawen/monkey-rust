use crate::op_code::Instructions;
use object::Closure;

#[derive(Debug, Clone)]
pub struct Frame {
    pub cl: Closure,
    pub ip: i32,
    pub base_pointer: usize,
}

impl Frame {
    pub fn new(func: Closure, base_pointer: usize) -> Self {
        Frame {
            cl: func,
            ip: -1,
            base_pointer,
        }
    }

    pub fn instructions(&self) -> Instructions {
        return Instructions {
            data: self.cl.func.instructions.clone(),
        };
    }
}
