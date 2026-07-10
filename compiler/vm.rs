use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use byteorder::{BigEndian, ByteOrder};
use object::builtins::BuiltIns;

use object::Object::ClosureObj;
use object::{BoundMethodObject, BuiltinFunc, ClassObject, Closure, InstanceObject, Object};

use crate::compiler::Bytecode;
use crate::frame::Frame;
use crate::op_code::{cast_u8_to_opcode, Opcode};

const STACK_SIZE: usize = 2048;
pub const GLOBAL_SIZE: usize = 65536;
const MAX_FRAMES: usize = 1024;

pub struct VM {
    constants: Vec<Rc<Object>>,

    stack: Vec<Rc<Object>>,
    sp: usize, // stack pointer. Always point to the next value. Top of the stack is stack[sp -1]

    pub globals: Vec<Rc<Object>>,

    frames: Vec<Frame>,
    frame_index: usize,
}

impl VM {
    pub fn new(bytecode: Bytecode) -> VM {
        // it's rust, it's verbose. You can't just grow your vector size.
        let empty_frame = Frame::new(
            Closure {
                func: Rc::from(object::CompiledFunction {
                    name: String::new(),
                    instructions: vec![],
                    num_locals: 0,
                    num_parameters: 0,
                }),
                free: vec![],
            },
            0,
        );

        let main_fn = Rc::from(object::CompiledFunction {
            name: String::new(),
            instructions: bytecode.instructions.data,
            num_locals: 0,
            num_parameters: 0,
        });
        let main_closure = Closure {
            func: main_fn,
            free: vec![],
        };
        let main_frame = Frame::new(main_closure, 0);
        let mut frames = vec![empty_frame; MAX_FRAMES];
        frames[0] = main_frame;

        return VM {
            constants: bytecode.constants,
            stack: vec![Rc::new(Object::Null); STACK_SIZE],
            sp: 0,
            globals: vec![Rc::new(Object::Null); GLOBAL_SIZE],
            frames,
            frame_index: 1,
        };
    }

    pub fn new_with_global_store(bytecode: Bytecode, globals: Vec<Rc<Object>>) -> VM {
        let mut vm = VM::new(bytecode);
        vm.globals = globals;
        return vm;
    }

    pub fn run(&mut self) {
        let mut ip = 0;
        let mut ins: Vec<u8>;
        while self.current_frame().ip
            < self.current_frame().instructions().data.clone().len() as i32 - 1
        {
            self.current_frame().ip += 1;
            ip = self.current_frame().ip as usize;
            ins = self.current_frame().instructions().data.clone();

            let op: u8 = *ins.get(ip).unwrap();
            let opcode = cast_u8_to_opcode(op);

            match opcode {
                Opcode::OpConst => {
                    let const_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
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
                    self.current_frame().ip = pos as i32 - 1;
                }
                Opcode::OpJumpNotTruthy => {
                    let pos = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let condition = self.pop();
                    if !self.is_truthy(condition) {
                        self.current_frame().ip = pos as i32 - 1;
                    }
                }
                Opcode::OpNull => {
                    self.push(Rc::new(Object::Null));
                }
                Opcode::OpGetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.push(Rc::clone(&self.globals[global_index]));
                }
                Opcode::OpSetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.globals[global_index] = self.pop();
                }
                Opcode::OpArray => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let elements = self.build_array(self.sp - count, self.sp);
                    self.sp = self.sp - count;
                    self.push(Rc::new(Object::Array(elements)));
                }
                Opcode::OpHash => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let elements = self.build_hash(self.sp - count, self.sp);
                    self.sp = self.sp - count;
                    self.push(Rc::new(Object::Hash(elements)));
                }
                Opcode::OpIndex => {
                    let index = self.pop();
                    let left = self.pop();
                    self.execute_index_operation(left, index);
                }
                Opcode::OpReturnValue => {
                    let return_value = self.pop();
                    let frame = self.pop_frame();
                    self.sp = frame.base_pointer - 1;
                    self.push(return_value);
                }
                Opcode::OpReturn => {
                    let frame = self.pop_frame();
                    self.sp = frame.base_pointer - 1;
                    self.push(Rc::new(object::Object::Null));
                }
                Opcode::OpCall => {
                    let num_args = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    self.execute_call(num_args);
                }
                Opcode::OpSetLocal => {
                    let local_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let base = self.current_frame().base_pointer;
                    self.stack[base + local_index] = self.pop();
                }
                Opcode::OpGetLocal => {
                    let local_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let base = self.current_frame().base_pointer;
                    self.push(Rc::clone(&self.stack[base + local_index]));
                }
                Opcode::OpGetBuiltin => {
                    let built_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let definition = BuiltIns.get(built_index).unwrap().function;
                    self.push(Rc::new(Object::Builtin(definition)));
                }
                Opcode::OpClosure => {
                    let const_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    let num_free = ins[ip + 3] as usize;
                    self.current_frame().ip += 3;
                    self.push_closure(const_index, num_free);
                }
                Opcode::OpGetFree => {
                    let free_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let current_closure = self.current_frame().cl.clone();
                    self.push(current_closure.free[free_index].clone());
                }
                Opcode::OpCurrentClosure => {
                    let current_closure = self.current_frame().cl.clone();
                    self.push(Rc::new(Object::ClosureObj(current_closure)));
                }
                Opcode::OpClass => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    self.push(Rc::new(Object::Class(Rc::new(RefCell::new(ClassObject {
                        name,
                        constructor: None,
                        methods: HashMap::new(),
                    })))));
                }
                Opcode::OpMethod => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    let kind = ins[ip + 3];
                    self.current_frame().ip += 3;
                    let name = self.constant_string(name_index);
                    let method = self.pop();
                    let class = match &*self.stack[self.sp - 1] {
                        Object::Class(class) => Rc::clone(class),
                        value => panic!("cannot install method on {}", value),
                    };
                    if kind == 1 {
                        class.borrow_mut().constructor = Some(method);
                    } else {
                        class.borrow_mut().methods.insert(name, method);
                    }
                }
                Opcode::OpGetProperty => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    let receiver = self.pop();
                    let value = self.get_property(&receiver, &name);
                    self.push(value);
                }
                Opcode::OpSetProperty => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    let value = self.pop();
                    let receiver = self.pop();
                    self.set_property(&receiver, name, value);
                }
                Opcode::OpNew => {
                    let num_args = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    self.execute_new(num_args);
                }
            }
        }
    }

    fn execute_binary_operation(&mut self, opcode: Opcode) {
        let right = self.pop();
        let left = self.pop();
        match (left.as_ref(), right.as_ref()) {
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
        if opcode == Opcode::OpEqual || opcode == Opcode::OpNotEqual {
            let equal = left.as_ref() == right.as_ref();
            self.push(Rc::new(Object::Boolean(if opcode == Opcode::OpEqual {
                equal
            } else {
                !equal
            })));
            return;
        }
        match (left.as_ref(), right.as_ref()) {
            (Object::Integer(l), Object::Integer(r)) => {
                let result = match opcode {
                    Opcode::OpGreaterThan => l > r,
                    _ => panic!("Unknown opcode for comparing int"),
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
        match operand.as_ref() {
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
        match operand.as_ref() {
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
        match condition.as_ref() {
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
        match (left.as_ref(), index.as_ref()) {
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

    fn current_frame(&mut self) -> &mut Frame {
        &mut self.frames[self.frame_index - 1]
    }

    fn push_frame(&mut self, frame: Frame) {
        self.frames[self.frame_index] = frame;
        self.frame_index += 1;
    }

    fn pop_frame(&mut self) -> Frame {
        self.frame_index -= 1;
        return self.frames[self.frame_index].clone();
    }

    fn execute_call(&mut self, num_args: usize) {
        let callee = Rc::clone(&self.stack[self.sp - 1 - num_args]);
        match &*callee {
            Object::ClosureObj(cf) => {
                self.call_closure(cf.clone(), num_args);
            }
            Object::Builtin(bt) => {
                self.call_builtin(bt.clone(), num_args);
            }
            Object::BoundMethod(bound) => {
                self.call_bound_method(bound.clone(), num_args);
            }
            Object::Class(class) => {
                panic!("class {} must be constructed with new", class.borrow().name)
            }
            _ => {
                panic!("calling non-closure")
            }
        }
    }
    fn call_closure(&mut self, cl: Closure, num_args: usize) {
        if cl.func.num_parameters != num_args {
            panic!("wrong number of arguments: want={}, got={}", cl.func.num_parameters, num_args);
        }

        let frame = Frame::new(cl.clone(), self.sp - num_args);
        self.sp = frame.base_pointer + cl.func.num_locals;
        self.push_frame(frame);
    }

    fn call_builtin(&mut self, bt: BuiltinFunc, num_args: usize) {
        let args = self.stack[self.sp - num_args..self.sp].to_vec();
        let result = bt(args);
        self.sp = self.sp - num_args - 1;
        self.push(result);
    }

    fn push_closure(&mut self, const_index: usize, num_free: usize) {
        match &*self.constants[const_index] {
            Object::CompiledFunction(f) => {
                let mut free = Vec::with_capacity(num_free);
                for i in 0..num_free {
                    let f = self.stack[self.sp - num_free + i].clone();
                    free.push(f);
                }
                self.sp = self.sp - num_free;
                let closure = ClosureObj(Closure {
                    func: f.clone(),
                    free,
                });
                self.push(Rc::new(closure));
            }
            o => {
                panic!("not a function {}", o);
            }
        }
    }

    fn constant_string(&self, index: usize) -> String {
        match &*self.constants[index] {
            Object::String(value) => value.clone(),
            value => panic!("expected string constant, got {}", value),
        }
    }

    fn get_property(&self, receiver: &Rc<Object>, name: &str) -> Rc<Object> {
        let Object::Instance(instance) = &**receiver else {
            panic!("cannot read property '{}' of {}", name, receiver);
        };
        if let Some(value) = instance.borrow().fields.get(name).cloned() {
            return value;
        }
        let (class_name, method) = {
            let instance_object = instance.borrow();
            let class = instance_object.class.borrow();
            (class.name.clone(), class.methods.get(name).cloned())
        };
        match method {
            Some(method) => Rc::new(Object::BoundMethod(Rc::new(BoundMethodObject {
                receiver: Rc::clone(instance),
                method,
                name: name.to_string(),
            }))),
            None => panic!("property '{}' does not exist on {}", name, class_name),
        }
    }

    fn set_property(&self, receiver: &Rc<Object>, name: String, value: Rc<Object>) {
        let Object::Instance(instance) = &**receiver else {
            panic!("cannot set property '{}' of {}", name, receiver);
        };
        instance.borrow_mut().fields.insert(name, value);
    }

    fn execute_new(&mut self, num_args: usize) {
        let base = self.sp - num_args - 1;
        let class = match &*self.stack[base] {
            Object::Class(class) => Rc::clone(class),
            value => panic!("cannot construct {}", value),
        };
        let instance = Rc::new(RefCell::new(InstanceObject {
            class: Rc::clone(&class),
            fields: HashMap::new(),
        }));
        let instance_value = Rc::new(Object::Instance(instance));
        let constructor = class.borrow().constructor.clone();
        let Some(constructor) = constructor else {
            if num_args != 0 {
                panic!(
                    "wrong number of arguments for {}.constructor: want=0, got={}",
                    class.borrow().name,
                    num_args
                );
            }
            self.sp = base;
            self.push(instance_value);
            return;
        };

        let closure = match &*constructor {
            Object::ClosureObj(closure) => closure.clone(),
            value => panic!("constructor is not a closure: {}", value),
        };
        let expected = closure.func.num_parameters.saturating_sub(1);
        if expected != num_args {
            panic!(
                "wrong number of arguments for {}.constructor: want={}, got={}",
                class.borrow().name,
                expected,
                num_args
            );
        }
        self.rewrite_receiver_call(constructor, instance_value, num_args);
        self.call_closure(closure, num_args + 1);
    }

    fn call_bound_method(&mut self, bound: Rc<BoundMethodObject>, num_args: usize) {
        let closure = match &*bound.method {
            Object::ClosureObj(closure) => closure.clone(),
            value => panic!("bound method is not a closure: {}", value),
        };
        let expected = closure.func.num_parameters.saturating_sub(1);
        if expected != num_args {
            let class_name = bound.receiver.borrow().class.borrow().name.clone();
            panic!(
                "wrong number of arguments for {}.{}: want={}, got={}",
                class_name, bound.name, expected, num_args
            );
        }
        let receiver = Rc::new(Object::Instance(Rc::clone(&bound.receiver)));
        self.rewrite_receiver_call(Rc::clone(&bound.method), receiver, num_args);
        self.call_closure(closure, num_args + 1);
    }

    fn rewrite_receiver_call(
        &mut self,
        callable: Rc<Object>,
        receiver: Rc<Object>,
        num_args: usize,
    ) {
        let base = self.sp - num_args - 1;
        for index in (base + 1..self.sp).rev() {
            self.stack[index + 1] = Rc::clone(&self.stack[index]);
        }
        self.stack[base] = callable;
        self.stack[base + 1] = receiver;
        self.sp += 1;
    }
}
