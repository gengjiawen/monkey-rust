use std::collections::HashMap;

use byteorder::{BigEndian, ByteOrder};
use compiler::compiler::{Bytecode, DebugInfo};
use compiler::op_code::{cast_u8_to_opcode, Opcode};
use object::builtins::{BuiltIns, BuiltinId};
use object::Object;
use parser::lexer::token::Span;
use serde::Serialize;

use crate::frame::Frame;
use crate::report::{empty_value_kind_counts, GcCollectionReport};
use crate::value::{
    alloc_value, call_builtin, export_object, get_value, get_value_mut, import_object,
    try_export_object, value_to_string, GcBoundMethod, GcClass, GcClosure, GcInstance, HashKey,
    Value,
};
use crate::{GcHeap, GcRef};

const STACK_SIZE: usize = 2048;
pub const GLOBAL_SIZE: usize = 65536;
const MAX_FRAMES: usize = 1024;
pub const DEFAULT_INSTRUCTION_BUDGET: usize = 100_000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcRuntimeError {
    pub message: String,
    pub span: Option<Span>,
}

enum CalleeKind {
    Closure(GcClosure),
    Builtin(BuiltinId),
    BoundMethod(GcBoundMethod),
    Class(String),
    Other(String),
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
    last_popped: GcRef,
    main_debug_info: DebugInfo,
    function_debug_info: HashMap<GcRef, DebugInfo>,
}

impl GcVM {
    pub fn new(bytecode: Bytecode) -> Self {
        let Bytecode {
            instructions,
            constants: object_constants,
            debug_info: main_debug_info,
            function_debug_info: object_function_debug_info,
        } = bytecode;
        let mut heap = GcHeap::new();
        let null = alloc_value(&mut heap, Value::Null);
        let constants = object_constants
            .iter()
            .map(|constant| import_object(&mut heap, constant))
            .collect::<Vec<_>>();
        let function_debug_info = object_function_debug_info
            .into_iter()
            .filter_map(|(index, debug_info)| {
                constants
                    .get(index)
                    .copied()
                    .map(|reference| (reference, debug_info))
            })
            .collect();

        let main_fn = alloc_value(
            &mut heap,
            Value::CompiledFunction(object::CompiledFunction {
                name: String::new(),
                instructions: instructions.data,
                num_locals: 0,
                num_parameters: 0,
            }),
        );
        let main_instructions = compiled_instructions(&heap, main_fn);
        // Frames keep borrowed GcRefs. The initial main_fn allocation is the VM
        // root for these handles; placeholder frames do not take extra refs.
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
        let last_popped = heap.dup(null);

        GcVM {
            heap,
            constants,
            stack,
            sp: 0,
            globals,
            frames,
            frame_index: 1,
            null,
            last_popped,
            main_debug_info,
            function_debug_info,
        }
    }

    pub fn heap(&self) -> &GcHeap {
        &self.heap
    }

    pub fn heap_mut(&mut self) -> &mut GcHeap {
        &mut self.heap
    }

    pub fn collect_garbage(&mut self) -> GcCollectionReport {
        let before_kinds = self.heap.value_kinds_by_id();
        let before = self.heap.snapshot();
        let telemetry = self.heap.run_gc_with_stats();
        let after = self.heap.snapshot();
        let mut collected_by_value_kind = empty_value_kind_counts();
        for (id, kind) in before_kinds {
            if !self.heap.runtime().object_exists(id) {
                *collected_by_value_kind.entry(kind).or_default() += 1;
            }
        }
        GcCollectionReport {
            before,
            after,
            objects: telemetry.objects,
            phases: telemetry.phases,
            collected_by_value_kind,
        }
    }

    fn runtime_error(&self, message: impl Into<String>) -> GcRuntimeError {
        let frame = &self.frames[self.frame_index - 1];
        let debug_info = if self.frame_index == 1 {
            Some(&self.main_debug_info)
        } else {
            self.function_debug_info.get(&frame.cl.func)
        };
        let span = debug_info.and_then(|debug_info| {
            (frame.ip >= 0)
                .then(|| frame.ip as usize)
                .and_then(|pc| debug_info.span_for_pc(pc).cloned())
        });
        GcRuntimeError {
            message: message.into(),
            span,
        }
    }

    pub fn run(&mut self) {
        self.run_with_budget(usize::MAX)
            .expect("GC VM execution failed");
    }

    pub fn run_with_budget(&mut self, instruction_budget: usize) -> Result<(), GcRuntimeError> {
        let mut executed = 0;
        while self.current_frame().ip < self.current_frame().instructions.len() as i32 - 1 {
            self.current_frame().ip += 1;
            let ip = self.current_frame().ip as usize;
            if executed >= instruction_budget {
                return Err(self.runtime_error(format!(
                    "instruction limit exceeded (budget: {})",
                    instruction_budget
                )));
            }
            executed += 1;
            let ins = self.current_frame().instructions.clone();
            let op = *ins.get(ip).unwrap();
            let opcode = cast_u8_to_opcode(op);

            match opcode {
                Opcode::OpConst => {
                    let const_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.dup_and_push(self.constants[const_index])?;
                }
                Opcode::OpAdd | Opcode::OpSub | Opcode::OpMul | Opcode::OpDiv => {
                    self.execute_binary_operation(opcode)?;
                }
                Opcode::OpPop => {
                    self.pop_discard();
                }
                Opcode::OpTrue => {
                    self.alloc_and_push(Value::Boolean(true))?;
                }
                Opcode::OpFalse => {
                    self.alloc_and_push(Value::Boolean(false))?;
                }
                Opcode::OpEqual | Opcode::OpNotEqual | Opcode::OpGreaterThan => {
                    self.execute_comparison(opcode)?;
                }
                Opcode::OpMinus => {
                    self.execute_minus_operation()?;
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
                    let condition = self.pop_owned();
                    if !is_truthy(&self.heap, condition) {
                        self.current_frame().ip = pos as i32 - 1;
                    }
                    self.heap.free(condition);
                }
                Opcode::OpNull => {
                    self.dup_and_push(self.null)?;
                }
                Opcode::OpGetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    self.dup_and_push(self.globals[global_index])?;
                }
                Opcode::OpSetGlobal => {
                    let global_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let value = self.pop_owned();
                    self.heap.free(self.globals[global_index]);
                    self.globals[global_index] = value;
                }
                Opcode::OpArray => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let start = self.sp - count;
                    let elements = self.build_array(start, self.sp);
                    let array = alloc_value(&mut self.heap, Value::Array(elements));
                    self.clear_stack_range(start, self.sp);
                    self.sp = start;
                    self.push_raw(array)?;
                }
                Opcode::OpHash => {
                    let count = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let start = self.sp - count;
                    let elements = self.build_hash(start, self.sp)?;
                    let hash = alloc_value(&mut self.heap, Value::Hash(elements));
                    self.clear_stack_range(start, self.sp);
                    self.sp = start;
                    self.push_raw(hash)?;
                }
                Opcode::OpIndex => {
                    let index = self.pop_owned();
                    let left = self.pop_owned();
                    let result = self.execute_index_operation(left, index);
                    self.heap.free(index);
                    self.heap.free(left);
                    result?;
                }
                Opcode::OpReturnValue => {
                    let return_value = self.pop_owned();
                    if self.frame_index == 1 {
                        // A top-level return ends the program with this value
                        // as its result, matching the interpreter backend.
                        self.clear_stack_range(0, self.sp);
                        self.sp = 0;
                        self.heap.free(self.last_popped);
                        self.last_popped = return_value;
                        break;
                    }
                    let frame = self.pop_frame();
                    let new_sp = frame.base_pointer - 1;
                    self.clear_stack_range(new_sp, self.sp);
                    self.sp = new_sp;
                    self.push_raw(return_value)?;
                }
                Opcode::OpReturn => {
                    if self.frame_index == 1 {
                        self.clear_stack_range(0, self.sp);
                        self.sp = 0;
                        self.heap.free(self.last_popped);
                        self.last_popped = self.heap.dup(self.null);
                        break;
                    }
                    let frame = self.pop_frame();
                    let new_sp = frame.base_pointer - 1;
                    self.clear_stack_range(new_sp, self.sp);
                    self.sp = new_sp;
                    self.dup_and_push(self.null)?;
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
                    let value = self.pop_owned();
                    self.heap.free(self.stack[base + local_index]);
                    self.stack[base + local_index] = value;
                }
                Opcode::OpGetLocal => {
                    let local_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let base = self.current_frame().base_pointer;
                    self.dup_and_push(self.stack[base + local_index])?;
                }
                Opcode::OpGetBuiltin => {
                    let built_index = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    let definition = BuiltIns.get(built_index).unwrap();
                    self.alloc_and_push(Value::Builtin(definition.id))?;
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
                    let free_var = self.current_frame().cl.free[free_index];
                    self.dup_and_push(free_var)?;
                }
                Opcode::OpCurrentClosure => {
                    let current = self.current_frame().cl.clone();
                    self.alloc_and_push(Value::Closure(current))?;
                }
                Opcode::OpClass => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    self.alloc_and_push(Value::Class(GcClass {
                        name,
                        constructor: None,
                        methods: HashMap::new(),
                    }))?;
                }
                Opcode::OpMethod => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    let kind = ins[ip + 3];
                    self.current_frame().ip += 3;
                    let name = self.constant_string(name_index);
                    let method = self.pop_owned();
                    let class = self.stack[self.sp - 1];
                    let result = self.install_method(class, name, method, kind == 1);
                    self.heap.free(method);
                    result?;
                }
                Opcode::OpGetProperty => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    let receiver = self.pop_owned();
                    let value = self.get_property(receiver, &name);
                    self.heap.free(receiver);
                    self.push_raw(value?)?;
                }
                Opcode::OpSetProperty => {
                    let name_index = BigEndian::read_u16(&ins[ip + 1..ip + 3]) as usize;
                    self.current_frame().ip += 2;
                    let name = self.constant_string(name_index);
                    let value = self.pop_owned();
                    let receiver = self.pop_owned();
                    let result = self.set_property(receiver, name, value);
                    self.heap.free(value);
                    self.heap.free(receiver);
                    result?;
                }
                Opcode::OpNew => {
                    let num_args = ins[ip + 1] as usize;
                    self.current_frame().ip += 1;
                    self.execute_new(num_args)?;
                }
            }
        }
        Ok(())
    }

    pub fn last_popped_stack_elm(&self) -> Option<GcRef> {
        Some(self.last_popped)
    }

    pub fn export_last_result(&self) -> Option<Object> {
        self.last_popped_stack_elm()
            .map(|reference| export_object(&self.heap, reference))
    }

    pub fn try_export_last_result(&self) -> Result<Object, String> {
        try_export_object(&self.heap, self.last_popped)
    }

    pub fn last_result_string(&self) -> String {
        value_to_string(&self.heap, self.last_popped)
    }

    fn alloc_and_push(&mut self, value: Value) -> Result<(), GcRuntimeError> {
        let reference = alloc_value(&mut self.heap, value);
        self.push_raw(reference)
    }

    fn dup_and_push(&mut self, reference: GcRef) -> Result<(), GcRuntimeError> {
        let duplicated = self.heap.dup(reference);
        self.push_raw(duplicated)
    }

    fn push_raw(&mut self, value: GcRef) -> Result<(), GcRuntimeError> {
        if self.sp >= STACK_SIZE {
            let error = self.runtime_error("stack limit exceeded");
            self.heap.free(value);
            return Err(error);
        }
        let old = self.stack[self.sp];
        self.stack[self.sp] = value;
        self.heap.free(old);
        self.sp += 1;
        Ok(())
    }

    /// Move the top stack slot's owned reference to the caller.
    ///
    /// The caller must either free the returned ref or store it in another
    /// owning location. The vacated stack slot is reset to a null ref.
    fn pop_owned(&mut self) -> GcRef {
        self.sp -= 1;
        let value = self.stack[self.sp];
        self.stack[self.sp] = self.heap.dup(self.null);
        value
    }

    fn pop_discard(&mut self) {
        let value = self.pop_owned();
        self.heap.free(self.last_popped);
        self.last_popped = value;
    }

    fn clear_stack_range(&mut self, start: usize, end: usize) {
        for index in start..end {
            let old = self.stack[index];
            self.stack[index] = self.heap.dup(self.null);
            self.heap.free(old);
        }
    }

    fn execute_binary_operation(&mut self, opcode: Opcode) -> Result<(), GcRuntimeError> {
        let right = self.pop_owned();
        let left = self.pop_owned();
        let left_value = get_value(&self.heap, left).clone();
        let right_value = get_value(&self.heap, right).clone();
        let result = match (&left_value, &right_value) {
            (Value::Integer(l), Value::Integer(r)) => match opcode {
                Opcode::OpAdd => Ok(Value::Integer(l + r)),
                Opcode::OpSub => Ok(Value::Integer(l - r)),
                Opcode::OpMul => Ok(Value::Integer(l * r)),
                Opcode::OpDiv if *r != 0 => l
                    .checked_div(*r)
                    .map(Value::Integer)
                    .ok_or_else(|| "integer overflow in division".to_string()),
                Opcode::OpDiv => Err("division by zero".to_string()),
                _ => unreachable!(),
            },
            (Value::String(l), Value::String(r)) if opcode == Opcode::OpAdd => {
                Ok(Value::String(l.to_string() + r))
            }
            _ => Err(format!(
                "unsupported binary operation for {} and {}",
                value_to_string(&self.heap, left),
                value_to_string(&self.heap, right)
            )),
        };
        self.heap.free(left);
        self.heap.free(right);
        match result {
            Ok(value) => self.alloc_and_push(value),
            Err(message) => Err(self.runtime_error(message)),
        }
    }

    fn execute_comparison(&mut self, opcode: Opcode) -> Result<(), GcRuntimeError> {
        let right = self.pop_owned();
        let left = self.pop_owned();
        let result = match (get_value(&self.heap, left), get_value(&self.heap, right)) {
            (Value::Integer(l), Value::Integer(r)) => match opcode {
                Opcode::OpEqual => Some(l == r),
                Opcode::OpNotEqual => Some(l != r),
                Opcode::OpGreaterThan => Some(l > r),
                _ => unreachable!(),
            },
            (Value::Boolean(l), Value::Boolean(r)) => match opcode {
                Opcode::OpEqual => Some(l == r),
                Opcode::OpNotEqual => Some(l != r),
                _ => None,
            },
            (Value::String(l), Value::String(r)) => match opcode {
                Opcode::OpEqual => Some(l == r),
                Opcode::OpNotEqual => Some(l != r),
                _ => None,
            },
            (Value::Null, Value::Null) => match opcode {
                Opcode::OpEqual => Some(true),
                Opcode::OpNotEqual => Some(false),
                _ => None,
            },
            (Value::Class(_), Value::Class(_))
            | (Value::Instance(_), Value::Instance(_))
            | (Value::BoundMethod(_), Value::BoundMethod(_)) => match opcode {
                Opcode::OpEqual => Some(left == right),
                Opcode::OpNotEqual => Some(left != right),
                _ => None,
            },
            _ => None,
        };
        let message = if result.is_none() {
            Some(format!(
                "unsupported comparison for {} and {}",
                value_to_string(&self.heap, left),
                value_to_string(&self.heap, right)
            ))
        } else {
            None
        };
        self.heap.free(left);
        self.heap.free(right);
        if let Some(result) = result {
            self.alloc_and_push(Value::Boolean(result))
        } else {
            Err(self.runtime_error(message.unwrap()))
        }
    }

    fn execute_minus_operation(&mut self) -> Result<(), GcRuntimeError> {
        let operand = self.pop_owned();
        let negated = match get_value(&self.heap, operand) {
            Value::Integer(value) => Some(-value),
            _ => None,
        };
        let message = negated.is_none().then(|| {
            format!("unsupported type for negation: {}", value_to_string(&self.heap, operand))
        });
        self.heap.free(operand);
        if let Some(negated) = negated {
            self.alloc_and_push(Value::Integer(negated))
        } else {
            Err(self.runtime_error(message.unwrap()))
        }
    }

    fn execute_bang_operation(&mut self) -> Result<(), GcRuntimeError> {
        let operand = self.pop_owned();
        let result = match get_value(&self.heap, operand) {
            Value::Boolean(l) => !l,
            _ => false,
        };
        self.heap.free(operand);
        self.alloc_and_push(Value::Boolean(result))
    }

    fn build_array(&mut self, start: usize, end: usize) -> Vec<GcRef> {
        let mut elements = Vec::with_capacity(end - start);
        for i in start..end {
            elements.push(self.stack[i]);
        }
        elements
    }

    fn build_hash(
        &mut self,
        start: usize,
        end: usize,
    ) -> Result<HashMap<HashKey, GcRef>, GcRuntimeError> {
        let mut elements = HashMap::new();
        for i in (start..end).step_by(2) {
            let key_ref = self.stack[i];
            let key = HashKey::from_value(get_value(&self.heap, key_ref)).ok_or_else(|| {
                self.runtime_error(format!(
                    "hash key must be hashable, got {}",
                    value_to_string(&self.heap, key_ref)
                ))
            })?;
            elements.insert(key, self.stack[i + 1]);
        }
        Ok(elements)
    }

    fn execute_index_operation(&mut self, left: GcRef, index: GcRef) -> Result<(), GcRuntimeError> {
        let left_value = get_value(&self.heap, left).clone();
        let index_value = get_value(&self.heap, index).clone();
        match (&left_value, &index_value) {
            (Value::Array(array), Value::Integer(i)) => self.execute_array_index(array, *i),
            (Value::Hash(hash), _) => self.execute_hash_index(hash, &index_value),
            _ => Err(self.runtime_error(format!(
                "unsupported index operation for {} and {}",
                value_to_string(&self.heap, left),
                value_to_string(&self.heap, index)
            ))),
        }
    }

    fn execute_array_index(&mut self, array: &[GcRef], index: i64) -> Result<(), GcRuntimeError> {
        if index < array.len() as i64 && index >= 0 {
            self.dup_and_push(array[index as usize])
        } else {
            self.dup_and_push(self.null)
        }
    }

    fn execute_hash_index(
        &mut self,
        hash: &HashMap<HashKey, GcRef>,
        index: &Value,
    ) -> Result<(), GcRuntimeError> {
        let key = HashKey::from_value(index)
            .ok_or_else(|| self.runtime_error("unsupported hash index key"))?;
        match hash.get(&key) {
            Some(value) => self.dup_and_push(*value),
            None => self.dup_and_push(self.null),
        }
    }

    fn current_frame(&mut self) -> &mut Frame {
        &mut self.frames[self.frame_index - 1]
    }

    fn push_frame(&mut self, frame: Frame) -> Result<(), GcRuntimeError> {
        if self.frame_index >= MAX_FRAMES {
            return Err(self.runtime_error("frame limit exceeded"));
        }
        self.frames[self.frame_index] = frame;
        self.frame_index += 1;
        Ok(())
    }

    fn pop_frame(&mut self) -> Frame {
        self.frame_index -= 1;
        self.frames[self.frame_index].clone()
    }

    fn execute_call(&mut self, num_args: usize) -> Result<(), GcRuntimeError> {
        let callee = self.stack[self.sp - 1 - num_args];
        match callee_kind(&self.heap, callee) {
            CalleeKind::Closure(closure) => self.call_closure(closure, num_args),
            CalleeKind::Builtin(builtin) => self.call_builtin(builtin, num_args),
            CalleeKind::BoundMethod(bound) => self.call_bound_method(bound, num_args),
            CalleeKind::Class(name) => {
                Err(self.runtime_error(format!("class {} must be constructed with new", name)))
            }
            CalleeKind::Other(value) => Err(self.runtime_error(format!("cannot call {}", value))),
        }
    }

    fn call_closure(&mut self, closure: GcClosure, num_args: usize) -> Result<(), GcRuntimeError> {
        let compiled = match get_value(&self.heap, closure.func) {
            Value::CompiledFunction(f) => f.clone(),
            _ => return Err(self.runtime_error("closure without compiled function")),
        };
        if compiled.num_parameters != num_args {
            return Err(self.runtime_error(format!(
                "wrong number of arguments: want={}, got={}",
                compiled.num_parameters, num_args
            )));
        }

        let frame = Frame::new(closure, compiled.instructions, self.sp - num_args);
        let next_sp = frame.base_pointer + compiled.num_locals;
        if next_sp > STACK_SIZE {
            return Err(self.runtime_error("stack limit exceeded"));
        }
        self.sp = next_sp;
        self.push_frame(frame)
    }

    fn call_builtin(&mut self, builtin: BuiltinId, num_args: usize) -> Result<(), GcRuntimeError> {
        let base = self.sp - num_args - 1;
        let args = self.stack[self.sp - num_args..self.sp].to_vec();
        let result = call_builtin(&mut self.heap, builtin, &args, self.null);
        self.clear_stack_range(base, self.sp);
        self.sp = base;
        self.push_raw(result)
    }

    fn push_closure(&mut self, const_index: usize, num_free: usize) -> Result<(), GcRuntimeError> {
        match get_value(&self.heap, self.constants[const_index]).clone() {
            Value::CompiledFunction(_) => {
                let start = self.sp - num_free;
                let mut free = Vec::with_capacity(num_free);
                for i in 0..num_free {
                    free.push(self.stack[start + i]);
                }
                let func = self.constants[const_index];
                let closure = alloc_value(
                    &mut self.heap,
                    Value::Closure(GcClosure {
                        func,
                        free,
                    }),
                );
                self.clear_stack_range(start, self.sp);
                self.sp = start;
                self.push_raw(closure)
            }
            other => panic!("not a function {:?}", other),
        }
    }

    fn constant_string(&self, index: usize) -> String {
        match get_value(&self.heap, self.constants[index]) {
            Value::String(value) => value.clone(),
            value => panic!("expected string constant, got {}", value),
        }
    }

    fn install_method(
        &mut self,
        class: GcRef,
        name: String,
        method: GcRef,
        constructor: bool,
    ) -> Result<(), GcRuntimeError> {
        if !matches!(get_value(&self.heap, class), Value::Class(_)) {
            return Err(self.runtime_error(format!(
                "cannot install method on {}",
                value_to_string(&self.heap, class)
            )));
        }
        let owned_method = self.heap.dup(method);
        let old_method = match get_value_mut(&mut self.heap, class) {
            Value::Class(class) => {
                if constructor {
                    class.constructor.replace(owned_method)
                } else {
                    class.methods.insert(name, owned_method)
                }
            }
            _ => unreachable!(),
        };
        if let Some(old_method) = old_method {
            self.heap.free(old_method);
        }
        Ok(())
    }

    fn get_property(&mut self, receiver: GcRef, name: &str) -> Result<GcRef, GcRuntimeError> {
        let (class, field) = match get_value(&self.heap, receiver) {
            Value::Instance(instance) => (instance.class, instance.fields.get(name).copied()),
            _ => {
                return Err(self.runtime_error(format!(
                    "cannot read property '{}' of {}",
                    name,
                    value_to_string(&self.heap, receiver)
                )))
            }
        };
        if let Some(field) = field {
            return Ok(self.heap.dup(field));
        }

        let (class_name, method) = match get_value(&self.heap, class) {
            Value::Class(class) => (class.name.clone(), class.methods.get(name).copied()),
            _ => return Err(self.runtime_error("instance has invalid class")),
        };
        match method {
            Some(method) => Ok(alloc_value(
                &mut self.heap,
                Value::BoundMethod(GcBoundMethod {
                    receiver,
                    method,
                    name: name.to_string(),
                }),
            )),
            None => {
                Err(self
                    .runtime_error(format!("property '{}' does not exist on {}", name, class_name)))
            }
        }
    }

    fn set_property(
        &mut self,
        receiver: GcRef,
        name: String,
        value: GcRef,
    ) -> Result<(), GcRuntimeError> {
        if !matches!(get_value(&self.heap, receiver), Value::Instance(_)) {
            return Err(self.runtime_error(format!(
                "cannot set property '{}' of {}",
                name,
                value_to_string(&self.heap, receiver)
            )));
        }
        let owned_value = self.heap.dup(value);
        let old_value = match get_value_mut(&mut self.heap, receiver) {
            Value::Instance(instance) => instance.fields.insert(name, owned_value),
            _ => unreachable!(),
        };
        if let Some(old_value) = old_value {
            self.heap.free(old_value);
        }
        Ok(())
    }

    fn execute_new(&mut self, num_args: usize) -> Result<(), GcRuntimeError> {
        let base = self.sp - num_args - 1;
        let class_reference = self.stack[base];
        let (class_name, constructor) = match get_value(&self.heap, class_reference) {
            Value::Class(class) => (class.name.clone(), class.constructor),
            _ => {
                return Err(self.runtime_error(format!(
                    "cannot construct {}",
                    value_to_string(&self.heap, class_reference)
                )))
            }
        };

        let Some(constructor) = constructor else {
            if num_args != 0 {
                return Err(self.runtime_error(format!(
                    "wrong number of arguments for {}.constructor: want=0, got={}",
                    class_name, num_args
                )));
            }
            let instance = alloc_value(
                &mut self.heap,
                Value::Instance(GcInstance {
                    class: class_reference,
                    fields: HashMap::new(),
                }),
            );
            self.clear_stack_range(base, self.sp);
            self.sp = base;
            return self.push_raw(instance);
        };

        let closure = match get_value(&self.heap, constructor) {
            Value::Closure(closure) => closure.clone(),
            _ => return Err(self.runtime_error("constructor is not a closure")),
        };
        let compiled = match get_value(&self.heap, closure.func) {
            Value::CompiledFunction(function) => function.clone(),
            _ => return Err(self.runtime_error("constructor closure has invalid function")),
        };
        let expected = compiled.num_parameters.saturating_sub(1);
        if expected != num_args {
            return Err(self.runtime_error(format!(
                "wrong number of arguments for {}.constructor: want={}, got={}",
                class_name, expected, num_args
            )));
        }

        let instance = alloc_value(
            &mut self.heap,
            Value::Instance(GcInstance {
                class: class_reference,
                fields: HashMap::new(),
            }),
        );
        self.rewrite_receiver_call(constructor, instance, num_args)?;
        self.call_closure(closure, num_args + 1)
    }

    fn call_bound_method(
        &mut self,
        bound: GcBoundMethod,
        num_args: usize,
    ) -> Result<(), GcRuntimeError> {
        let closure = match get_value(&self.heap, bound.method) {
            Value::Closure(closure) => closure.clone(),
            _ => return Err(self.runtime_error("bound method is not a closure")),
        };
        let compiled = match get_value(&self.heap, closure.func) {
            Value::CompiledFunction(function) => function.clone(),
            _ => return Err(self.runtime_error("method closure has invalid function")),
        };
        let expected = compiled.num_parameters.saturating_sub(1);
        if expected != num_args {
            let class_name = match get_value(&self.heap, bound.receiver) {
                Value::Instance(instance) => match get_value(&self.heap, instance.class) {
                    Value::Class(class) => class.name.clone(),
                    _ => "<invalid class>".to_string(),
                },
                _ => "<invalid receiver>".to_string(),
            };
            return Err(self.runtime_error(format!(
                "wrong number of arguments for {}.{}: want={}, got={}",
                class_name, bound.name, expected, num_args
            )));
        }
        let receiver = self.heap.dup(bound.receiver);
        self.rewrite_receiver_call(bound.method, receiver, num_args)?;
        self.call_closure(closure, num_args + 1)
    }

    /// Takes ownership of `receiver` and frees it if the stack cannot hold
    /// the rewritten call layout.
    fn rewrite_receiver_call(
        &mut self,
        callable: GcRef,
        receiver: GcRef,
        num_args: usize,
    ) -> Result<(), GcRuntimeError> {
        let base = self.sp - num_args - 1;
        if base + num_args + 2 > STACK_SIZE {
            let error = self.runtime_error("stack limit exceeded");
            self.heap.free(receiver);
            return Err(error);
        }
        let callable = self.heap.dup(callable);
        let borrowed_arguments = self.stack[self.sp - num_args..self.sp].to_vec();
        let arguments = borrowed_arguments
            .into_iter()
            .map(|argument| self.heap.dup(argument))
            .collect::<Vec<_>>();
        self.clear_stack_range(base, self.sp);
        self.sp = base;
        self.push_raw(callable)?;
        self.push_raw(receiver)?;
        for argument in arguments {
            self.push_raw(argument)?;
        }
        Ok(())
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
        Value::BoundMethod(bound) => CalleeKind::BoundMethod(bound.clone()),
        Value::Class(class) => CalleeKind::Class(class.name.clone()),
        _ => CalleeKind::Other(value_to_string(heap, reference)),
    }
}

fn compiled_instructions(heap: &GcHeap, func: GcRef) -> Vec<u8> {
    match get_value(heap, func) {
        Value::CompiledFunction(f) => f.instructions.clone(),
        _ => panic!("expected compiled function"),
    }
}
