use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use parser::ast::{BlockStatement, Param};

#[macro_use]
extern crate lazy_static;

use crate::environment::Env;

pub mod builtins;
pub mod environment;

pub type EvalError = String;
pub type BuiltinFunc = fn(Vec<Rc<Object>>) -> Rc<Object>;

pub type ClassRef = Rc<RefCell<ClassObject>>;
pub type InstanceRef = Rc<RefCell<InstanceObject>>;

#[derive(Clone)]
pub enum Object {
    Integer(i64),
    Boolean(bool),
    String(String),
    Array(Vec<Rc<Object>>),
    Hash(HashMap<Rc<Object>, Rc<Object>>),
    Null,
    ReturnValue(Rc<Object>),
    Function(Vec<Param>, BlockStatement, Env),
    Builtin(BuiltinFunc),
    Error(String),
    CompiledFunction(Rc<CompiledFunction>),
    ClosureObj(Closure),
    Class(ClassRef),
    Instance(InstanceRef),
    BoundMethod(Rc<BoundMethodObject>),
}

#[derive(Clone)]
pub struct ClassObject {
    pub name: String,
    pub constructor: Option<Rc<Object>>,
    pub methods: HashMap<String, Rc<Object>>,
}

#[derive(Clone)]
pub struct InstanceObject {
    pub class: ClassRef,
    pub fields: HashMap<String, Rc<Object>>,
}

#[derive(Clone)]
pub struct BoundMethodObject {
    pub receiver: InstanceRef,
    pub method: Rc<Object>,
    pub name: String,
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Object::Integer(i) => write!(f, "{}", i),
            Object::Boolean(b) => write!(f, "{}", b),
            Object::String(s) => write!(f, "{}", s),
            Object::Null => write!(f, "null"),
            Object::ReturnValue(expr) => write!(f, "{}", expr),
            Object::Function(params, body, _env) => {
                let func_params = params
                    .iter()
                    .map(|stmt| stmt.to_string())
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "fn({}) {{ {} }}", func_params, body)
            }
            Object::Builtin(_) => write!(f, "[builtin function]"),
            Object::Error(e) => write!(f, "{}", e),
            Object::Array(e) => write!(
                f,
                "[{}]",
                e.iter()
                    .map(|o| o.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Object::Hash(map) => write!(
                f,
                "{{{}}}",
                sorted_hash_entries(map.iter())
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Object::CompiledFunction(_) => {
                write!(f, "[compiled function]")
            }
            Object::ClosureObj(_) => {
                write!(f, "[closure function]")
            }
            Object::Class(class) => write!(f, "[class {}]", class.borrow().name),
            Object::Instance(instance) => {
                write!(f, "[object {}]", instance.borrow().class.borrow().name)
            }
            Object::BoundMethod(method) => {
                let class_name = method.receiver.borrow().class.borrow().name.clone();
                write!(f, "[bound method {}.{}]", class_name, method.name)
            }
        }
    }
}

/// Hash entries in the canonical order every backend renders them in:
/// `(key type rank, canonical key bytes)` with integer=0, boolean=1, string=2
/// (arm64 backend design §10.2). `HashMap` iteration order is unspecified and
/// varies run to run, so a display that walked the map directly would print
/// the same hash differently on two runs of the same program.
fn sorted_hash_entries<'a>(
    map: impl Iterator<Item = (&'a Rc<Object>, &'a Rc<Object>)>,
) -> Vec<(&'a Rc<Object>, &'a Rc<Object>)> {
    let mut entries = map.collect::<Vec<_>>();
    entries.sort_by_key(|(key, _)| (hash_key_rank(key), hash_key_canonical_bytes(key)));
    entries
}

fn hash_key_rank(key: &Object) -> u8 {
    match key {
        Object::Integer(_) => 0,
        Object::Boolean(_) => 1,
        Object::String(_) => 2,
        // Unreachable for hashes the runtimes build: `is_hashable` rejects
        // every other variant before it can become a key.
        _ => 3,
    }
}

fn hash_key_canonical_bytes(key: &Object) -> Vec<u8> {
    match key {
        Object::Integer(raw) => raw.to_string().into_bytes(),
        Object::Boolean(raw) => raw.to_string().into_bytes(),
        Object::String(raw) => raw.clone().into_bytes(),
        other => other.to_string().into_bytes(),
    }
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Object::Integer(value) => f.debug_tuple("Integer").field(value).finish(),
            Object::Boolean(value) => f.debug_tuple("Boolean").field(value).finish(),
            Object::String(value) => f.debug_tuple("String").field(value).finish(),
            Object::Array(value) => f.debug_tuple("Array").field(value).finish(),
            Object::Hash(value) => f.debug_tuple("Hash").field(value).finish(),
            Object::Null => write!(f, "Null"),
            Object::ReturnValue(value) => f.debug_tuple("ReturnValue").field(value).finish(),
            Object::Function(params, body, _) => f
                .debug_struct("Function")
                .field("params", params)
                .field("body", body)
                .finish_non_exhaustive(),
            Object::Builtin(_) => write!(f, "Builtin([function])"),
            Object::Error(value) => f.debug_tuple("Error").field(value).finish(),
            Object::CompiledFunction(value) => {
                f.debug_tuple("CompiledFunction").field(value).finish()
            }
            Object::ClosureObj(value) => f.debug_tuple("ClosureObj").field(value).finish(),
            Object::Class(_) | Object::Instance(_) | Object::BoundMethod(_) => {
                write!(f, "{}", self)
            }
        }
    }
}

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Object::Integer(left), Object::Integer(right)) => left == right,
            (Object::Boolean(left), Object::Boolean(right)) => left == right,
            (Object::String(left), Object::String(right)) => left == right,
            (Object::Array(left), Object::Array(right)) => left == right,
            (Object::Hash(left), Object::Hash(right)) => left == right,
            (Object::Null, Object::Null) => true,
            (Object::ReturnValue(left), Object::ReturnValue(right)) => left == right,
            (
                Object::Function(left_params, left_body, left_env),
                Object::Function(right_params, right_body, right_env),
            ) => {
                left_params == right_params
                    && left_body == right_body
                    && Rc::ptr_eq(left_env, right_env)
            }
            (Object::Builtin(left), Object::Builtin(right)) => std::ptr::fn_addr_eq(*left, *right),
            (Object::Error(left), Object::Error(right)) => left == right,
            (Object::CompiledFunction(left), Object::CompiledFunction(right)) => left == right,
            (Object::ClosureObj(left), Object::ClosureObj(right)) => left == right,
            (Object::Class(left), Object::Class(right)) => Rc::ptr_eq(left, right),
            (Object::Instance(left), Object::Instance(right)) => Rc::ptr_eq(left, right),
            (Object::BoundMethod(left), Object::BoundMethod(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

impl Eq for Object {}

impl Object {
    /// Frozen truthiness (arm64 backend design §10.1): only `false` and `null`
    /// are falsy, and `!v` is exactly `!v.is_truthy()`. Every backend routes
    /// both `if` and `!` through this one definition.
    pub fn is_truthy(&self) -> bool {
        match self {
            Object::Boolean(value) => return *value,
            Object::Null => return false,
            _ => return true,
        }
    }

    pub fn is_hashable(&self) -> bool {
        match self {
            Object::Integer(_) | Object::Boolean(_) | Object::String(_) => return true,
            _ => return false,
        }
    }
}

impl Hash for Object {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Object::Integer(i) => i.hash(state),
            Object::Boolean(b) => b.hash(state),
            Object::String(s) => s.hash(state),
            t => panic!("can't hashable for {}", t),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompiledFunction {
    pub name: String,
    pub instructions: Vec<u8>,
    pub num_locals: usize,
    pub num_parameters: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Closure {
    pub func: Rc<CompiledFunction>,
    pub free: Vec<Rc<Object>>,
}
