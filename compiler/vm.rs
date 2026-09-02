use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use byteorder::{BigEndian, ByteOrder};
use object::builtins::BuiltIns;

use object::Object::ClosureObj;
use object::{BoundMethodObject, BuiltinFunc, ClassObject, Closure, InstanceObject, Object};

use crate::compiler::Bytecode;
use crate::frame::Frame;
use crate::op_code::Opcode;

const STACK_SIZE: usize = 2048;
pub const GLOBAL_SIZE: usize = 65536;
const MAX_FRAMES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmRuntimeErrorKind {
    Arithmetic,
    Call,
    Index,
    Property,
    Stack,
    Type,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmRuntimeError {
    pub kind: VmRuntimeErrorKind,
    pub message: String,
}

impl VmRuntimeError {
    fn new(kind: VmRuntimeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for VmRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for VmRuntimeError {}

type VmResult<T> = Result<T, VmRuntimeError>;

pub struct VM {
    constants: Vec<Rc<Object>>,

    stack: Vec<Rc<Object>>,
    sp: usize, // stack pointer. Always point to the next value. Top of the stack is stack[sp -1]

    pub globals: Vec<Rc<Object>>,

    frames: Vec<Frame>,
    frame_index: usize,
    last_error: Option<VmRuntimeError>,
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

        let null = Rc::new(Object::Null);
        return VM {
            constants: bytecode.constants,
            stack: vec![Rc::clone(&null); STACK_SIZE],
            sp: 0,
            globals: vec![null; GLOBAL_SIZE],
            frames,
            frame_index: 1,
            last_error: None,
        };
    }

    pub fn new_with_global_store(bytecode: Bytecode, globals: Vec<Rc<Object>>) -> VM {
        let mut vm = VM::new(bytecode);
        vm.globals = globals;
        return vm;
    }

    /// Run bytecode while retaining the error for callers of the original API.
    /// New code should prefer [`Self::run_checked`] so failures cannot be ignored.
    pub fn run(&mut self) {
        let _ = self.run_checked();
    }

    pub fn run_checked(&mut self) -> VmResult<()> {
        self.last_error = None;
        let result = self.run_inner();
        if let Err(error) = &result {
            self.last_error = Some(error.clone());
        }
        result
    }

    pub fn last_error(&self) -> Option<&VmRuntimeError> {
        self.last_error.as_ref()
    }

    fn run_inner(&mut self) -> VmResult<()> {
        let mut ip: usize;
        let mut ins: Vec<u8>;
        while self.current_frame().ip
            < self.current_frame().instructions().data.clone().len() as i32 - 1
        {
            self.current_frame().ip += 1;
            ip = self.current_frame().ip as usize;
            ins = self.current_frame().instructions().data.clone();

            let op: u8 = *ins.get(ip).unwrap();
            let opcode = Opcode::from_repr(op).expect("unknown opcode in compiled bytecode");

            match opcode {
                Opcode::OpConst => {
                    let const_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.push(Rc::clone(&self.constants[const_index]))?;
                }
                Opcode::OpAdd | Opcode::OpSub | Opcode::OpMul | Opcode::OpDiv => {
                    self.execute_binary_operation(opcode)?;
                }
                Opcode::OpPop => {
                    self.pop();
                }
                Opcode::OpTrue => {
                    self.push(Rc::new(Object::Boolean(true)))?;
                }
                Opcode::OpFalse => {
                    self.push(Rc::new(Object::Boolean(false)))?;
                }
                Opcode::OpEqual
                | Opcode::OpNotEqual
                | Opcode::OpGreaterThan
                | Opcode::OpLessThan => {
                    self.execute_comparison(opcode)?;
                }
                Opcode::OpMinus => {
                    self.execute_minus_operation(opcode)?;
                }
                Opcode::OpBang => {
                    self.execute_bang_operation()?;
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
                    self.push(Rc::new(Object::Null))?;
                }
                Opcode::OpGetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.push(Rc::clone(&self.globals[global_index]))?;
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
                    self.sp -= count;
                    self.push(Rc::new(Object::Array(elements)))?;
                }
                Opcode::OpHash => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    #[allow(clippy::mutable_key_type)]
                    let elements = self.build_hash(self.sp - count, self.sp)?;
                    self.sp -= count;
                    self.push(Rc::new(Object::Hash(elements)))?;
                }
                Opcode::OpIndex => {
                    let index = self.pop();
                    let left = self.pop();
                    self.execute_index_operation(left, index)?;
                }
                Opcode::OpReturnValue => {
                    let return_value = self.pop();
                    if self.frame_index == 1 {
                        // A top-level return ends the program with this value
                        // as its result, matching the interpreter backend.
                        self.stack[0] = return_value;
                        self.sp = 0;
                        break;
                    }
                    let frame = self.pop_frame();
                    self.sp = frame.base_pointer - 1;
                    self.push(return_value)?;
                }
                Opcode::OpReturn => {
                    if self.frame_index == 1 {
                        self.stack[0] = Rc::new(object::Object::Null);
                        self.sp = 0;
                        break;
                    }
                    let frame = self.pop_frame();
                    self.sp = frame.base_pointer - 1;
                    self.push(Rc::new(object::Object::Null))?;
                }
                Opcode::OpCall => {
                    let num_args = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    self.execute_call(num_args)?;
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
                    self.push(Rc::clone(&self.stack[base + local_index]))?;
                }
                Opcode::OpGetBuiltin => {
                    let built_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let definition = BuiltIns.get(built_index).unwrap().function;
                    self.push(Rc::new(Object::Builtin(definition)))?;
                }
                Opcode::OpClosure => {
                    let const_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    let num_free = ins[ip + 3] as usize;
                    self.current_frame().ip += 3;
                    self.push_closure(const_index, num_free)?;
                }
                Opcode::OpGetFree => {
                    let free_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let current_closure = self.current_frame().cl.clone();
                    self.push(current_closure.free[free_index].clone())?;
                }
                Opcode::OpCurrentClosure => {
                    let current_closure = self.current_frame().cl.clone();
                    self.push(Rc::new(Object::ClosureObj(current_closure)))?;
                }
                Opcode::OpClass => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    self.push(Rc::new(Object::Class(Rc::new(RefCell::new(ClassObject {
                        name,
                        constructor: None,
                        methods: HashMap::new(),
                    })))))?;
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
                    let value = self.get_property(&receiver, &name)?;
                    self.push(value)?;
                }
                Opcode::OpSetProperty => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    let value = self.pop();
                    let receiver = self.pop();
                    self.set_property(&receiver, name, value)?;
                }
                Opcode::OpNew => {
                    let num_args = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    self.execute_new(num_args)?;
                }
                Opcode::OpDebugger => {
                    // Recording debugger snapshots is the GC VM's job; here
                    // the statement is a no-op with no stack effect.
                }
            }
        }
        Ok(())
    }

    fn execute_binary_operation(&mut self, opcode: Opcode) -> VmResult<()> {
        let right = self.pop();
        let left = self.pop();
        match (left.as_ref(), right.as_ref()) {
            (Object::Integer(l), Object::Integer(r)) => {
                let result = match opcode {
                    Opcode::OpAdd => l.checked_add(*r).ok_or_else(|| {
                        VmRuntimeError::new(
                            VmRuntimeErrorKind::Arithmetic,
                            "integer overflow in addition",
                        )
                    }),
                    Opcode::OpSub => l.checked_sub(*r).ok_or_else(|| {
                        VmRuntimeError::new(
                            VmRuntimeErrorKind::Arithmetic,
                            "integer overflow in subtraction",
                        )
                    }),
                    Opcode::OpMul => l.checked_mul(*r).ok_or_else(|| {
                        VmRuntimeError::new(
                            VmRuntimeErrorKind::Arithmetic,
                            "integer overflow in multiplication",
                        )
                    }),
                    Opcode::OpDiv if *r == 0 => {
                        Err(VmRuntimeError::new(VmRuntimeErrorKind::Arithmetic, "division by zero"))
                    }
                    Opcode::OpDiv => l.checked_div(*r).ok_or_else(|| {
                        VmRuntimeError::new(
                            VmRuntimeErrorKind::Arithmetic,
                            "integer overflow in division",
                        )
                    }),
                    _ => unreachable!("compiler emitted non-binary opcode"),
                }?;
                self.push(Rc::from(Object::Integer(result)))
            }
            (Object::String(l), Object::String(r)) if opcode == Opcode::OpAdd => {
                self.push(Rc::from(Object::String(l.to_string() + r)))
            }
            _ => Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Type,
                format!("unsupported binary operation for {} and {}", left, right),
            )),
        }
    }

    fn execute_comparison(&mut self, opcode: Opcode) -> VmResult<()> {
        let right = self.pop();
        let left = self.pop();
        if opcode == Opcode::OpEqual || opcode == Opcode::OpNotEqual {
            let equal = left.as_ref() == right.as_ref();
            return self.push(Rc::new(Object::Boolean(if opcode == Opcode::OpEqual {
                equal
            } else {
                !equal
            })));
        }
        match (left.as_ref(), right.as_ref()) {
            (Object::Integer(l), Object::Integer(r)) => {
                let result = match opcode {
                    Opcode::OpGreaterThan => l > r,
                    Opcode::OpLessThan => l < r,
                    _ => unreachable!("compiler emitted non-comparison opcode"),
                };
                self.push(Rc::from(Object::Boolean(result)))
            }
            _ => Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Type,
                format!("unsupported comparison for {} and {}", left, right),
            )),
        }
    }

    fn execute_minus_operation(&mut self, opcode: Opcode) -> VmResult<()> {
        let operand = self.pop();
        match operand.as_ref() {
            Object::Integer(value) => value
                .checked_neg()
                .ok_or_else(|| {
                    VmRuntimeError::new(
                        VmRuntimeErrorKind::Arithmetic,
                        "integer overflow in negation",
                    )
                })
                .and_then(|value| self.push(Rc::from(Object::Integer(value)))),
            _ => Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Type,
                format!("unsupported type for negation {:?}: {}", opcode, operand),
            )),
        }
    }

    fn execute_bang_operation(&mut self) -> VmResult<()> {
        let operand = self.pop();
        match operand.as_ref() {
            Object::Boolean(l) => self.push(Rc::from(Object::Boolean(!*l))),
            _ => self.push(Rc::from(Object::Boolean(false))),
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

    fn push(&mut self, o: Rc<Object>) -> VmResult<()> {
        if self.sp >= STACK_SIZE {
            return Err(VmRuntimeError::new(VmRuntimeErrorKind::Stack, "stack limit exceeded"));
        }
        self.stack[self.sp] = o;
        self.sp += 1;
        Ok(())
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

    // Object's Hash impl only covers Integer/Boolean/String, which have no
    // interior mutability, so the keys are effectively immutable.
    #[allow(clippy::mutable_key_type)]
    fn build_hash(&self, start: usize, end: usize) -> VmResult<HashMap<Rc<Object>, Rc<Object>>> {
        let mut elements = HashMap::new();
        for i in (start..end).step_by(2) {
            let key = Rc::clone(&self.stack[i]);
            if !key.is_hashable() {
                return Err(VmRuntimeError::new(
                    VmRuntimeErrorKind::Index,
                    format!("hash key must be hashable, got {}", key),
                ));
            }
            let value = Rc::clone(&self.stack[i + 1]);
            elements.insert(key, value);
        }
        Ok(elements)
    }

    fn execute_index_operation(&mut self, left: Rc<Object>, index: Rc<Object>) -> VmResult<()> {
        match (left.as_ref(), index.as_ref()) {
            (Object::Array(l), Object::Integer(i)) => self.execute_array_index(l, *i),
            (Object::Hash(l), _) => self.execute_hash_index(l, index),
            _ => Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Index,
                format!("unsupported index operation for {} with {}", left, index),
            )),
        }
    }

    fn execute_array_index(&mut self, array: &[Rc<Object>], index: i64) -> VmResult<()> {
        if index < array.len() as i64 && index >= 0 {
            self.push(Rc::clone(&array[index as usize]))
        } else {
            self.push(Rc::new(Object::Null))
        }
    }

    #[allow(clippy::mutable_key_type)]
    fn execute_hash_index(
        &mut self,
        hash: &HashMap<Rc<Object>, Rc<Object>>,
        index: Rc<Object>,
    ) -> VmResult<()> {
        match &*index {
            Object::Integer(_) | Object::Boolean(_) | Object::String(_) => match hash.get(&index) {
                Some(el) => self.push(Rc::clone(el)),
                None => self.push(Rc::new(Object::Null)),
            },
            _ => Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Index,
                format!("unsupported hash index key {}", index),
            )),
        }
    }

    fn current_frame(&mut self) -> &mut Frame {
        &mut self.frames[self.frame_index - 1]
    }

    fn push_frame(&mut self, frame: Frame) -> VmResult<()> {
        if self.frame_index >= MAX_FRAMES {
            return Err(VmRuntimeError::new(VmRuntimeErrorKind::Stack, "frame limit exceeded"));
        }
        self.frames[self.frame_index] = frame;
        self.frame_index += 1;
        Ok(())
    }

    fn pop_frame(&mut self) -> Frame {
        self.frame_index -= 1;
        return self.frames[self.frame_index].clone();
    }

    fn execute_call(&mut self, num_args: usize) -> VmResult<()> {
        let callee = Rc::clone(&self.stack[self.sp - 1 - num_args]);
        match &*callee {
            Object::ClosureObj(cf) => self.call_closure(cf.clone(), num_args),
            Object::Builtin(bt) => self.call_builtin(*bt, num_args),
            Object::BoundMethod(bound) => self.call_bound_method(bound.clone(), num_args),
            Object::Class(class) => Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Call,
                format!("class {} must be constructed with new", class.borrow().name),
            )),
            _ => Err(VmRuntimeError::new(VmRuntimeErrorKind::Call, "calling non-closure")),
        }
    }

    fn call_closure(&mut self, cl: Closure, num_args: usize) -> VmResult<()> {
        if cl.func.num_parameters != num_args {
            return Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Call,
                format!(
                    "wrong number of arguments: want={}, got={}",
                    cl.func.num_parameters, num_args
                ),
            ));
        }

        let frame = Frame::new(cl.clone(), self.sp - num_args);
        let next_sp = frame
            .base_pointer
            .checked_add(cl.func.num_locals)
            .filter(|next_sp| *next_sp <= STACK_SIZE)
            .ok_or_else(|| {
                VmRuntimeError::new(VmRuntimeErrorKind::Stack, "stack limit exceeded")
            })?;
        self.push_frame(frame)?;
        self.sp = next_sp;
        Ok(())
    }

    fn call_builtin(&mut self, bt: BuiltinFunc, num_args: usize) -> VmResult<()> {
        let args = self.stack[self.sp - num_args..self.sp].to_vec();
        let result = bt(args);
        self.sp = self.sp - num_args - 1;
        // Builtins report failures as Object::Error values. Every other
        // runtime failure here is an Err, so lift them the way the interpreter
        // does; otherwise the message keeps flowing as an ordinary value and
        // ends up as an operand, e.g. `len(1) + 1`.
        if let Object::Error(message) = &*result {
            return Err(VmRuntimeError::new(VmRuntimeErrorKind::Call, message.clone()));
        }
        self.push(result)
    }

    fn push_closure(&mut self, const_index: usize, num_free: usize) -> VmResult<()> {
        match &*self.constants[const_index] {
            Object::CompiledFunction(f) => {
                let mut free = Vec::with_capacity(num_free);
                for i in 0..num_free {
                    let f = self.stack[self.sp - num_free + i].clone();
                    free.push(f);
                }
                self.sp -= num_free;
                let closure = ClosureObj(Closure {
                    func: f.clone(),
                    free,
                });
                self.push(Rc::new(closure))
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

    fn get_property(&self, receiver: &Rc<Object>, name: &str) -> VmResult<Rc<Object>> {
        let Object::Instance(instance) = &**receiver else {
            return Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Property,
                format!("cannot read property '{}' of {}", name, receiver),
            ));
        };
        if let Some(value) = instance.borrow().fields.get(name).cloned() {
            return Ok(value);
        }
        let (class_name, method) = {
            let instance_object = instance.borrow();
            let class = instance_object.class.borrow();
            (class.name.clone(), class.methods.get(name).cloned())
        };
        match method {
            Some(method) => Ok(Rc::new(Object::BoundMethod(Rc::new(BoundMethodObject {
                receiver: Rc::clone(instance),
                method,
                name: name.to_string(),
            })))),
            None => Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Property,
                format!("property '{}' does not exist on {}", name, class_name),
            )),
        }
    }

    fn set_property(&self, receiver: &Rc<Object>, name: String, value: Rc<Object>) -> VmResult<()> {
        let Object::Instance(instance) = &**receiver else {
            return Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Property,
                format!("cannot set property '{}' of {}", name, receiver),
            ));
        };
        instance.borrow_mut().fields.insert(name, value);
        Ok(())
    }

    fn execute_new(&mut self, num_args: usize) -> VmResult<()> {
        let base = self.sp - num_args - 1;
        let class = match &*self.stack[base] {
            Object::Class(class) => Rc::clone(class),
            value => {
                return Err(VmRuntimeError::new(
                    VmRuntimeErrorKind::Call,
                    format!("cannot construct {}", value),
                ))
            }
        };
        let instance = Rc::new(RefCell::new(InstanceObject {
            class: Rc::clone(&class),
            fields: HashMap::new(),
        }));
        let instance_value = Rc::new(Object::Instance(instance));
        let constructor = class.borrow().constructor.clone();
        let Some(constructor) = constructor else {
            if num_args != 0 {
                return Err(VmRuntimeError::new(
                    VmRuntimeErrorKind::Call,
                    format!(
                        "wrong number of arguments for {}.constructor: want=0, got={}",
                        class.borrow().name,
                        num_args
                    ),
                ));
            }
            self.sp = base;
            self.push(instance_value)?;
            return Ok(());
        };

        let closure = match &*constructor {
            Object::ClosureObj(closure) => closure.clone(),
            value => panic!("constructor is not a closure: {}", value),
        };
        let expected = closure.func.num_parameters.saturating_sub(1);
        if expected != num_args {
            return Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Call,
                format!(
                    "wrong number of arguments for {}.constructor: want={}, got={}",
                    class.borrow().name,
                    expected,
                    num_args
                ),
            ));
        }
        self.rewrite_receiver_call(constructor, instance_value, num_args)?;
        self.call_closure(closure, num_args + 1)
    }

    fn call_bound_method(&mut self, bound: Rc<BoundMethodObject>, num_args: usize) -> VmResult<()> {
        let closure = match &*bound.method {
            Object::ClosureObj(closure) => closure.clone(),
            value => panic!("bound method is not a closure: {}", value),
        };
        let expected = closure.func.num_parameters.saturating_sub(1);
        if expected != num_args {
            let class_name = bound.receiver.borrow().class.borrow().name.clone();
            return Err(VmRuntimeError::new(
                VmRuntimeErrorKind::Call,
                format!(
                    "wrong number of arguments for {}.{}: want={}, got={}",
                    class_name, bound.name, expected, num_args
                ),
            ));
        }
        let receiver = Rc::new(Object::Instance(Rc::clone(&bound.receiver)));
        self.rewrite_receiver_call(Rc::clone(&bound.method), receiver, num_args)?;
        self.call_closure(closure, num_args + 1)
    }

    fn rewrite_receiver_call(
        &mut self,
        callable: Rc<Object>,
        receiver: Rc<Object>,
        num_args: usize,
    ) -> VmResult<()> {
        if self.sp >= STACK_SIZE {
            return Err(VmRuntimeError::new(VmRuntimeErrorKind::Stack, "stack limit exceeded"));
        }
        let base = self.sp - num_args - 1;
        for index in (base + 1..self.sp).rev() {
            self.stack[index + 1] = Rc::clone(&self.stack[index]);
        }
        self.stack[base] = callable;
        self.stack[base + 1] = receiver;
        self.sp += 1;
        Ok(())
    }
}
