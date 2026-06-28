use std::collections::HashMap;
use std::fmt;
use std::fmt::{write, Formatter};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use parser::ast::{BlockStatement, IDENTIFIER};

#[macro_use]
extern crate lazy_static;

use crate::environment::Env;

pub mod builtins;
pub mod environment;

pub type EvalError = String;
pub type BuiltinFunc = fn(Vec<Rc<Object>>) -> Rc<Object>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Object {
    Integer(i64),
    Boolean(bool),
    String(String),
    Array(Vec<Rc<Object>>),
    Hash(HashMap<Rc<Object>, Rc<Object>>),
    Null,
    ReturnValue(Rc<Object>),
    Function(Vec<IDENTIFIER>, BlockStatement, Env),
    Builtin(BuiltinFunc),
    Error(String),
    CompiledFunction(Rc<CompiledFunction>),
    ClosureObj(Closure),
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
                "[{}]",
                map.iter()
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
        }
    }
}

impl Object {
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
    pub instructions: Vec<u8>,
    pub num_locals: usize,
    pub num_parameters: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Closure {
    pub func: Rc<CompiledFunction>,
    pub free: Vec<Rc<Object>>,
}
