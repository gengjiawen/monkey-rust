use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;

use byteorder::{BigEndian, ByteOrder};

use object::Object;

use crate::compiler::Bytecode;
use crate::op_code::{cast_u8_to_opcode, Instructions, Opcode};

const STACK_SIZE: usize = 2048;
pub const GLOBAL_SIZE: usize = 65536;

pub struct VM {
    constants: Vec<Rc<Object>>,
    instructions: Instructions,

    stack: Vec<Rc<Object>>,
    sp: usize, // stack pointer. Always point to the next value. Top of the stack is stack[sp -1]

    pub globals: Vec<Rc<Object>>,
}

impl VM {
    pub fn new(bytecode: Bytecode) -> VM {
        return VM {
            constants: bytecode.constants,
            instructions: bytecode.instructions,
            stack: vec![Rc::new(Object::Null); STACK_SIZE],
            sp: 0,
            globals: vec![Rc::new(Object::Null); GLOBAL_SIZE],
        };
    }

    pub fn new_with_global_store(bytecode: Bytecode, globals: Vec<Rc<Object>>) -> VM {
        return VM {
            constants: bytecode.constants,
            instructions: bytecode.instructions,
            stack: vec![Rc::new(Object::Null); STACK_SIZE],
            sp: 0,
            globals,
        };
    }

    pub fn run(&mut self) {
        let mut ip = 0;
        let ins = self.instructions.data.clone();
        while ip < ins.len() {
            let op: u8 = *ins.get(ip).unwrap();
            let opcode = cast_u8_to_opcode(op);
            match opcode {
                Opcode::OpConst => {
                    let const_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    ip += 2;
                    self.push(Rc::clone(&self.constants[const_index]))
                }
                Opcode::OpAdd | Opcode::OpSub | Opcode::OpMul | Opcode::OpDiv => {
                    self.execute_binary_operation(opcode);
                }
                Opcode::OpPop => {
                    self.pop();
                }
                Opcode::OpTrue => {
                    self.push(Rc::new(Object::Boolean(true)));
                }
                Opcode::OpFalse => {
                    self.push(Rc::new(Object::Boolean(false)));
                }
                Opcode::OpEqual | Opcode::OpNotEqual | Opcode::OpGreaterThan => {
                    self.execute_comparison(opcode);
                }
                Opcode::OpMinus => {
                    self.execute_minus_operation(opcode);
                }
                Opcode::OpBang => {
                    self.execute_bang_operation();
                }
                Opcode::OpJump => {
                    let pos = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    ip = pos - 1;
                }
                Opcode::OpJumpNotTruthy => {
                    let pos = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    ip += 2;
                    let condition = self.pop();
                    if !self.is_truthy(condition) {
                        ip = pos - 1;
                    }
                }
                Opcode::OpNull => {
                    self.push(Rc::new(Object::Null));
                }
                Opcode::OpGetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    ip += 2;
                    self.push(Rc::clone(&self.globals[global_index]));
                }
                Opcode::OpSetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    ip += 2;
                    self.globals[global_index] = self.pop();
                }
                Opcode::OpArray => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    ip += 2;
                    let elements = self.build_array(self.sp - count, self.sp);
                    self.sp = self.sp - count;
                    self.push(Rc::new(Object::Array(elements)));
                }
                Opcode::OpHash => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    ip += 2;
                    let elements = self.build_hash(self.sp - count, self.sp);
                    self.sp = self.sp - count;
                    self.push(Rc::new(Object::Hash(elements)));
                }
                Opcode::OpIndex => {
                    let index = self.pop();
                    let left = self.pop();
                    self.execute_index_operation(left, index);
                }
            }
            ip += 1;
        }
    }

    fn execute_binary_operation(&mut self, opcode: Opcode) {
        let right = self.pop();
        let left = self.pop();
        match (left.borrow(), right.borrow()) {
            (Object::Integer(l), Object::Integer(r)) => {
                let result = match opcode {
                    Opcode::OpAdd => l + r,
                    Opcode::OpSub => l - r,
                    Opcode::OpMul => l * r,
                    Opcode::OpDiv => l / r,
                    _ => panic!("Unknown opcode for int"),
                };
                self.push(Rc::from(Object::Integer(result)));
            }
            (Object::String(l), Object::String(r)) => {
                let result = match opcode {
                    Opcode::OpAdd => l.to_string() + &r.to_string(),
                    _ => panic!("Unknown opcode for string"),
                };
                self.push(Rc::from(Object::String(result)));
            }
            _ => {
                panic!("unsupported add for those types")
            }
        }
    }

    fn execute_comparison(&mut self, opcode: Opcode) {
        let right = self.pop();
        let left = self.pop();
        match (left.borrow(), right.borrow()) {
            (Object::Integer(l), Object::Integer(r)) => {
                let result = match opcode {
                    Opcode::OpEqual => l == r,
                    Opcode::OpNotEqual => l != r,
                    Opcode::OpGreaterThan => l > r,
                    _ => panic!("Unknown opcode for comparing int"),
                };
                self.push(Rc::from(Object::Boolean(result)));
            }
            (Object::Boolean(l), Object::Boolean(r)) => {
                let result = match opcode {
                    Opcode::OpEqual => l == r,
                    Opcode::OpNotEqual => l != r,
                    _ => panic!("Unknown opcode for comparing boolean"),
                };
                self.push(Rc::from(Object::Boolean(result)));
            }
            _ => {
                panic!("unsupported comparison for those types")
            }
        }
    }

    fn execute_minus_operation(&mut self, opcode: Opcode) {
        let operand = self.pop();
        match operand.borrow() {
            Object::Integer(l) => {
                self.push(Rc::from(Object::Integer(-*l)));
            }
            _ => {
                panic!("unsupported types for negation {:?}", opcode)
            }
        }
    }
    fn execute_bang_operation(&mut self) {
        let operand = self.pop();
        match operand.borrow() {
            Object::Boolean(l) => {
                self.push(Rc::from(Object::Boolean(!*l)));
            }
            _ => {
                self.push(Rc::from(Object::Boolean(false)));
            }
        }
    }

    pub fn last_popped_stack_elm(&self) -> Option<Rc<Object>> {
        self.stack.get(self.sp).cloned()
    }

    fn pop(&mut self) -> Rc<Object> {
        let o = Rc::clone(&self.stack[self.sp - 1]);
        self.sp -= 1;
        return o;
    }

    fn push(&mut self, o: Rc<Object>) {
        if self.sp >= STACK_SIZE {
            panic!("Stack overflow");
        };
        self.stack[self.sp] = o;
        self.sp += 1;
    }
    fn is_truthy(&self, condition: Rc<Object>) -> bool {
        match condition.borrow() {
            Object::Boolean(b) => *b,
            Object::Null => false,
            _ => true,
        }
    }
    fn build_array(&self, start: usize, end: usize) -> Vec<Rc<Object>> {
        let mut elements = Vec::with_capacity(end - start);
        for i in start..end {
            elements.push(Rc::clone(&self.stack[i]));
        }
        return elements;
    }

    fn build_hash(&self, start: usize, end: usize) -> HashMap<Rc<Object>, Rc<Object>> {
        let mut elements = HashMap::new();
        for i in (start..end).step_by(2) {
            let key = Rc::clone(&self.stack[i]);
            let value = Rc::clone(&self.stack[i + 1]);
            elements.insert(key, value);
        }
        return elements;
    }

    fn execute_index_operation(&mut self, left: Rc<Object>, index: Rc<Object>) {
        match (left.borrow(), index.borrow()) {
            (Object::Array(l), Object::Integer(i)) => {
                self.execute_array_index(l, *i);
            }
            (Object::Hash(l), _) => {
                self.execute_hash_index(l, index);
            }
            _ => {
                panic!("unsupported index operation for those types")
            }
        }
    }

    fn execute_array_index(&mut self, array: &Vec<Rc<Object>>, index: i64) {
        if index < array.len() as i64 && index >= 0 {
            self.push(Rc::clone(&array[index as usize]));
        } else {
            self.push(Rc::new(Object::Null));
        }
    }

    fn execute_hash_index(&mut self, hash: &HashMap<Rc<Object>, Rc<Object>>, index: Rc<Object>) {
        match &*index {
            Object::Integer(_) | Object::Boolean(_) | Object::String(_) => match hash.get(&index) {
                Some(el) => {
                    self.push(Rc::clone(el));
                }
                None => {
                    self.push(Rc::new(Object::Null));
                }
            },
            _ => {
                panic!("unsupported hash index operation for those types {}", index)
            }
        }
    }
}
