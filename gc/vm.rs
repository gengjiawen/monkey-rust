use std::collections::HashMap;

use byteorder::{BigEndian, ByteOrder};
use compiler::compiler::Bytecode;
use compiler::op_code::{cast_u8_to_opcode, Opcode};
use object::builtins::BuiltIns;
use object::Object;

use crate::frame::Frame;
use crate::value::{
    alloc_value,
    call_builtin,
    export_object,
    get_value,
    import_object,
    GcClosure,
    HashKey,
    Value,
};
use crate::{GcHeap, GcRef};

const STACK_SIZE: usize = 2048;
pub const GLOBAL_SIZE: usize = 65536;
const MAX_FRAMES: usize = 1024;

enum CalleeKind {
    Closure(GcClosure),
    Builtin(object::BuiltinFunc),
}

pub struct GcVM {
    heap: GcHeap,
    constants: Vec<GcRef>,
    stack: Vec<GcRef>,
    sp: usize,
    globals: Vec<GcRef>,
    frames: Vec<Frame>,
    frame_index: usize,
    null: GcRef,
}

impl GcVM {
    pub fn new(bytecode: Bytecode) -> Self {
        let mut heap = GcHeap::new();
        let null = alloc_value(&mut heap, Value::Null);
        let constants = bytecode
            .constants
            .iter()
            .map(|constant| import_object(&mut heap, constant))
            .collect();

        let main_fn = alloc_value(
            &mut heap,
            Value::CompiledFunction(object::CompiledFunction {
                instructions: bytecode.instructions.data,
                num_locals: 0,
                num_parameters: 0,
            }),
        );
        let main_instructions = compiled_instructions(&heap, main_fn);
        let main_frame = Frame::new(
            GcClosure {
                func: main_fn,
                free: vec![],
            },
            main_instructions,
            0,
        );

        let empty_frame = Frame::new(
            GcClosure {
                func: main_fn,
                free: vec![],
            },
            vec![],
            0,
        );

        let mut frames = vec![empty_frame; MAX_FRAMES];
        frames[0] = main_frame;

        let stack = (0..STACK_SIZE).map(|_| heap.dup(null)).collect();
        let globals = (0..GLOBAL_SIZE).map(|_| heap.dup(null)).collect();

        GcVM {
            heap,
            constants,
            stack,
            sp: 0,
            globals,
            frames,
            frame_index: 1,
            null,
        }
    }

    pub fn heap(&self) -> &GcHeap {
        &self.heap
    }

    pub fn heap_mut(&mut self) -> &mut GcHeap {
        &mut self.heap
    }

    pub fn run(&mut self) {
        while self.current_frame().ip
            < self.current_frame().instructions.len() as i32 - 1
        {
            self.current_frame().ip += 1;
            let ip = self.current_frame().ip as usize;
            let ins = self.current_frame().instructions.clone();
            let op = *ins.get(ip).unwrap();
            let opcode = cast_u8_to_opcode(op);

            match opcode {
                Opcode::OpConst => {
                    let const_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.dup_and_push(self.constants[const_index]);
                }
                Opcode::OpAdd | Opcode::OpSub | Opcode::OpMul | Opcode::OpDiv => {
                    self.execute_binary_operation(opcode);
                }
                Opcode::OpPop => {
                    self.pop_discard();
                }
                Opcode::OpTrue => {
                    self.alloc_and_push(Value::Boolean(true));
                }
                Opcode::OpFalse => {
                    self.alloc_and_push(Value::Boolean(false));
                }
                Opcode::OpEqual | Opcode::OpNotEqual | Opcode::OpGreaterThan => {
                    self.execute_comparison(opcode);
                }
                Opcode::OpMinus => {
                    self.execute_minus_operation();
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
                    if !is_truthy(&self.heap, condition) {
                        self.current_frame().ip = pos as i32 - 1;
                    }
                    self.heap.free(condition);
                }
                Opcode::OpNull => {
                    self.dup_and_push(self.null);
                }
                Opcode::OpGetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.dup_and_push(self.globals[global_index]);
                }
                Opcode::OpSetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let value = self.pop();
                    self.heap.free(self.globals[global_index]);
                    self.globals[global_index] = value;
                }
                Opcode::OpArray => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let elements = self.build_array(self.sp - count, self.sp);
                    self.sp -= count;
                    self.alloc_and_push(Value::Array(elements));
                }
                Opcode::OpHash => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let elements = self.build_hash(self.sp - count, self.sp);
                    self.sp -= count;
                    self.alloc_and_push(Value::Hash(elements));
                }
                Opcode::OpIndex => {
                    let index = self.pop();
                    let left = self.pop();
                    self.execute_index_operation(left, index);
                    self.heap.free(index);
                    self.heap.free(left);
                }
                Opcode::OpReturnValue => {
                    let return_value = self.pop();
                    let frame = self.pop_frame();
                    self.sp = frame.base_pointer - 1;
                    self.push_raw(return_value);
                }
                Opcode::OpReturn => {
                    let frame = self.pop_frame();
                    self.sp = frame.base_pointer - 1;
                    self.dup_and_push(self.null);
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
                    let value = self.pop();
                    self.heap.free(self.stack[base + local_index]);
                    self.stack[base + local_index] = value;
                }
                Opcode::OpGetLocal => {
                    let local_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let base = self.current_frame().base_pointer;
                    self.dup_and_push(self.stack[base + local_index]);
                }
                Opcode::OpGetBuiltin => {
                    let built_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let definition = BuiltIns.get(built_index).unwrap().1;
                    self.alloc_and_push(Value::Builtin(definition));
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
                    let free_var = self.current_frame().cl.free[free_index];
                    self.dup_and_push(free_var);
                }
                Opcode::OpCurrentClosure => {
                    let current = self.current_frame().cl.clone();
                    self.alloc_and_push(Value::Closure(current));
                }
            }
        }
    }

    pub fn last_popped_stack_elm(&self) -> Option<GcRef> {
        self.stack.get(self.sp).copied()
    }

    pub fn export_last_result(&self) -> Option<Object> {
        self.last_popped_stack_elm()
            .map(|reference| export_object(&self.heap, reference))
    }

    fn alloc_and_push(&mut self, value: Value) {
        let reference = alloc_value(&mut self.heap, value);
        self.push_raw(reference);
    }

    fn dup_and_push(&mut self, reference: GcRef) {
        let duplicated = self.heap.dup(reference);
        self.push_raw(duplicated);
    }

    fn push_raw(&mut self, value: GcRef) {
        if self.sp >= STACK_SIZE {
            panic!("Stack overflow");
        }
        self.stack[self.sp] = value;
        self.sp += 1;
    }

    fn pop(&mut self) -> GcRef {
        let value = self.heap.dup(self.stack[self.sp - 1]);
        self.sp -= 1;
        value
    }

    fn pop_discard(&mut self) {
        self.sp -= 1;
    }

    fn execute_binary_operation(&mut self, opcode: Opcode) {
        let right = self.pop();
        let left = self.pop();
        match (get_value(&self.heap, left), get_value(&self.heap, right)) {
            (Value::Integer(l), Value::Integer(r)) => {
                let result = match opcode {
                    Opcode::OpAdd => l + r,
                    Opcode::OpSub => l - r,
                    Opcode::OpMul => l * r,
                    Opcode::OpDiv => l / r,
                    _ => panic!("Unknown opcode for int"),
                };
                self.alloc_and_push(Value::Integer(result));
            }
            (Value::String(l), Value::String(r)) => {
                let result = match opcode {
                    Opcode::OpAdd => l.to_string() + r,
                    _ => panic!("Unknown opcode for string"),
                };
                self.alloc_and_push(Value::String(result));
            }
            _ => panic!("unsupported binary operation for those types"),
        }
        self.heap.free(left);
        self.heap.free(right);
    }

    fn execute_comparison(&mut self, opcode: Opcode) {
        let right = self.pop();
        let left = self.pop();
        let result = match (get_value(&self.heap, left), get_value(&self.heap, right)) {
            (Value::Integer(l), Value::Integer(r)) => match opcode {
                Opcode::OpEqual => l == r,
                Opcode::OpNotEqual => l != r,
                Opcode::OpGreaterThan => l > r,
                _ => panic!("Unknown opcode for comparing int"),
            },
            (Value::Boolean(l), Value::Boolean(r)) => match opcode {
                Opcode::OpEqual => l == r,
                Opcode::OpNotEqual => l != r,
                _ => panic!("Unknown opcode for comparing boolean"),
            },
            _ => panic!("unsupported comparison for those types"),
        };
        self.alloc_and_push(Value::Boolean(result));
        self.heap.free(left);
        self.heap.free(right);
    }

    fn execute_minus_operation(&mut self) {
        let operand = self.pop();
        let negated = match get_value(&self.heap, operand) {
            Value::Integer(l) => -l,
            _ => panic!("unsupported types for negation"),
        };
        self.alloc_and_push(Value::Integer(negated));
        self.heap.free(operand);
    }

    fn execute_bang_operation(&mut self) {
        let operand = self.pop();
        let result = match get_value(&self.heap, operand) {
            Value::Boolean(l) => !l,
            _ => false,
        };
        self.alloc_and_push(Value::Boolean(result));
        self.heap.free(operand);
    }

    fn build_array(&mut self, start: usize, end: usize) -> Vec<GcRef> {
        let mut elements = Vec::with_capacity(end - start);
        for i in start..end {
            elements.push(self.heap.dup(self.stack[i]));
        }
        elements
    }

    fn build_hash(&mut self, start: usize, end: usize) -> HashMap<HashKey, GcRef> {
        let mut elements = HashMap::new();
        for i in (start..end).step_by(2) {
            let key_ref = self.stack[i];
            let key = HashKey::from_value(get_value(&self.heap, key_ref))
                .expect("hash key must be hashable");
            elements.insert(key, self.heap.dup(self.stack[i + 1]));
        }
        elements
    }

    fn execute_index_operation(&mut self, left: GcRef, index: GcRef) {
        let left_value = get_value(&self.heap, left).clone();
        let index_value = get_value(&self.heap, index).clone();
        match (&left_value, &index_value) {
            (Value::Array(array), Value::Integer(i)) => {
                self.execute_array_index(array, *i);
            }
            (Value::Hash(hash), _) => {
                self.execute_hash_index(hash, &index_value);
            }
            _ => panic!("unsupported index operation for those types"),
        }
    }

    fn execute_array_index(&mut self, array: &[GcRef], index: i64) {
        if index < array.len() as i64 && index >= 0 {
            self.dup_and_push(array[index as usize]);
        } else {
            self.dup_and_push(self.null);
        }
    }

    fn execute_hash_index(&mut self, hash: &HashMap<HashKey, GcRef>, index: &Value) {
        let key = HashKey::from_value(index).expect("unsupported hash index key");
        match hash.get(&key) {
            Some(value) => self.dup_and_push(*value),
            None => self.dup_and_push(self.null),
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
        self.frames[self.frame_index].clone()
    }

    fn execute_call(&mut self, num_args: usize) {
        let callee = self.stack[self.sp - 1 - num_args];
        match callee_kind(&self.heap, callee) {
            CalleeKind::Closure(closure) => self.call_closure(closure, num_args),
            CalleeKind::Builtin(builtin) => self.call_builtin(builtin, num_args),
        }
    }

    fn call_closure(&mut self, closure: GcClosure, num_args: usize) {
        let compiled = match get_value(&self.heap, closure.func) {
            Value::CompiledFunction(f) => f.clone(),
            _ => panic!("closure without compiled function"),
        };
        if compiled.num_parameters != num_args {
            panic!(
                "wrong number of arguments: want={}, got={}",
                compiled.num_parameters, num_args
            );
        }

        let frame = Frame::new(closure, compiled.instructions, self.sp - num_args);
        self.sp = frame.base_pointer + compiled.num_locals;
        self.push_frame(frame);
    }

    fn call_builtin(&mut self, builtin: object::BuiltinFunc, num_args: usize) {
        let mut args = Vec::with_capacity(num_args);
        for reference in &self.stack[self.sp - num_args..self.sp] {
            args.push(self.heap.dup(*reference));
        }
        let result = call_builtin(&mut self.heap, builtin, args);
        self.sp = self.sp - num_args - 1;
        self.push_raw(result);
    }

    fn push_closure(&mut self, const_index: usize, num_free: usize) {
        match get_value(&self.heap, self.constants[const_index]).clone() {
            Value::CompiledFunction(_) => {
                let mut free = Vec::with_capacity(num_free);
                for i in 0..num_free {
                    free.push(self.heap.dup(self.stack[self.sp - num_free + i]));
                }
                self.sp -= num_free;
                let func = self.heap.dup(self.constants[const_index]);
                self.alloc_and_push(Value::Closure(GcClosure { func, free }));
            }
            other => panic!("not a function {:?}", other),
        }
    }
}

fn is_truthy(heap: &GcHeap, condition: GcRef) -> bool {
    match get_value(heap, condition) {
        Value::Boolean(b) => *b,
        Value::Null => false,
        _ => true,
    }
}

fn callee_kind(heap: &GcHeap, reference: GcRef) -> CalleeKind {
    match get_value(heap, reference) {
        Value::Closure(closure) => CalleeKind::Closure(closure.clone()),
        Value::Builtin(builtin) => CalleeKind::Builtin(*builtin),
        _ => panic!("calling non-closure"),
    }
}

fn compiled_instructions(heap: &GcHeap, func: GcRef) -> Vec<u8> {
    match get_value(heap, func) {
        Value::CompiledFunction(f) => f.instructions.clone(),
        _ => panic!("expected compiled function"),
    }
}
