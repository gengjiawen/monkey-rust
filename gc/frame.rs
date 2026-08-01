use crate::value::GcClosure;
use compiler::op_code::Instructions;

#[derive(Debug, Clone)]
pub struct Frame {
    /// Borrowed closure refs. Frames do not own or free these handles; the
    /// callee stack slot, main function root, or closure object keeps them live.
    pub cl: GcClosure,
    pub ip: i32,
    pub base_pointer: usize,
    pub instructions: Vec<u8>,
    /// One flag per local slot: has the slot been written since the frame was
    /// pushed? Parameters (including a method's `this`) arrive initialized;
    /// `let` slots hold a prefilled null the debugger must not present as a
    /// user value until `OpSetLocal` marks them.
    pub initialized: Vec<bool>,
}

impl Frame {
    pub fn new(
        closure: GcClosure,
        instructions: Vec<u8>,
        base_pointer: usize,
        num_locals: usize,
        num_parameters: usize,
    ) -> Self {
        Frame {
            cl: closure,
            ip: -1,
            base_pointer,
            instructions,
            initialized: (0..num_locals).map(|slot| slot < num_parameters).collect(),
        }
    }

    pub fn instruction_view(&self) -> Instructions {
        Instructions {
            data: self.instructions.clone(),
        }
    }
}
