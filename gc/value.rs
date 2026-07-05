use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::header::GcObjectType;
use crate::{GcHeap, GcObject, GcRef};
use object::{BuiltinFunc, Closure, CompiledFunction, Object};

/// Runtime value stored in the GC heap. Mirrors `object::Object` but uses `GcRef` edges.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i64),
    Boolean(bool),
    String(String),
    Array(Vec<GcRef>),
    Hash(HashMap<HashKey, GcRef>),
    Null,
    Error(String),
    CompiledFunction(CompiledFunction),
    Closure(GcClosure),
    Builtin(BuiltinFunc),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcClosure {
    pub func: GcRef,
    pub free: Vec<GcRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HashKey {
    Integer(i64),
    Boolean(bool),
    String(String),
}

pub struct ValueCell {
    pub value: Value,
}

impl GcObject for ValueCell {
    fn trace(&self, visit: &mut dyn FnMut(crate::GcId)) {
        self.value.trace(&mut |reference| visit(reference.0));
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Value {
    pub fn trace(&self, visit: &mut dyn FnMut(GcRef)) {
        match self {
            Value::Array(items) => {
                for item in items {
                    visit(*item);
                }
            }
            Value::Hash(map) => {
                for value in map.values() {
                    visit(*value);
                }
            }
            Value::Closure(closure) => {
                visit(closure.func);
                for free in &closure.free {
                    visit(*free);
                }
            }
            _ => {}
        }
    }

    pub fn with_owned_edges(self, heap: &mut GcHeap) -> Self {
        match self {
            Value::Array(items) => Value::Array(items.into_iter().map(|r| heap.dup(r)).collect()),
            Value::Hash(map) => {
                Value::Hash(map.into_iter().map(|(k, v)| (k, heap.dup(v))).collect())
            }
            Value::Closure(mut closure) => {
                closure.func = heap.dup(closure.func);
                closure.free = closure.free.into_iter().map(|r| heap.dup(r)).collect();
                Value::Closure(closure)
            }
            other => other,
        }
    }

    pub fn edge_refs(&self) -> Vec<GcRef> {
        let mut refs = Vec::new();
        self.trace(&mut |reference| refs.push(reference));
        refs
    }
}

impl HashKey {
    pub fn from_object(object: &Object) -> Option<HashKey> {
        match object {
            Object::Integer(i) => Some(HashKey::Integer(*i)),
            Object::Boolean(b) => Some(HashKey::Boolean(*b)),
            Object::String(s) => Some(HashKey::String(s.clone())),
            _ => None,
        }
    }

    pub fn from_value(value: &Value) -> Option<HashKey> {
        match value {
            Value::Integer(i) => Some(HashKey::Integer(*i)),
            Value::Boolean(b) => Some(HashKey::Boolean(*b)),
            Value::String(s) => Some(HashKey::String(s.clone())),
            _ => None,
        }
    }

    pub fn to_object(&self) -> Object {
        match self {
            HashKey::Integer(i) => Object::Integer(*i),
            HashKey::Boolean(b) => Object::Boolean(*b),
            HashKey::String(s) => Object::String(s.clone()),
        }
    }
}

pub fn alloc_value(heap: &mut GcHeap, value: Value) -> GcRef {
    let value = value.with_owned_edges(heap);
    heap.alloc(
        ValueCell {
            value,
        },
        GcObjectType::MonkeyObject,
    )
}

pub fn get_value<'a>(heap: &'a GcHeap, reference: GcRef) -> &'a Value {
    &heap
        .runtime()
        .object_downcast::<ValueCell>(reference.0)
        .expect("invalid value reference")
        .value
}

pub fn value_to_string(heap: &GcHeap, reference: GcRef) -> String {
    format_value(heap, get_value(heap, reference))
}

fn format_value(heap: &GcHeap, value: &Value) -> String {
    match value {
        Value::Integer(i) => i.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Error(e) => e.clone(),
        Value::Array(items) => {
            let parts = items
                .iter()
                .map(|item| value_to_string(heap, *item))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", parts)
        }
        Value::Hash(map) => {
            let parts = map
                .iter()
                .map(|(k, v)| format!("{}: {}", format_hash_key(k), value_to_string(heap, *v)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{}}}", parts)
        }
        Value::CompiledFunction(_) => "[compiled function]".to_string(),
        Value::Closure(_) => "[closure function]".to_string(),
        Value::Builtin(_) => "[builtin function]".to_string(),
    }
}

fn format_hash_key(key: &HashKey) -> String {
    match key {
        HashKey::Integer(i) => i.to_string(),
        HashKey::Boolean(b) => b.to_string(),
        HashKey::String(s) => s.clone(),
    }
}

pub fn import_object(heap: &mut GcHeap, object: &Object) -> GcRef {
    let value = match object {
        Object::Integer(i) => Value::Integer(*i),
        Object::Boolean(b) => Value::Boolean(*b),
        Object::String(s) => Value::String(s.clone()),
        Object::Null => Value::Null,
        Object::Error(e) => Value::Error(e.clone()),
        Object::Array(items) => {
            Value::Array(items.iter().map(|item| import_object(heap, item)).collect())
        }
        Object::Hash(map) => Value::Hash(
            map.iter()
                .map(|(k, v)| {
                    (
                        HashKey::from_object(k).expect("hash key must be hashable"),
                        import_object(heap, v),
                    )
                })
                .collect(),
        ),
        Object::CompiledFunction(f) => Value::CompiledFunction(CompiledFunction {
            instructions: f.instructions.clone(),
            num_locals: f.num_locals,
            num_parameters: f.num_parameters,
        }),
        Object::ClosureObj(closure) => Value::Closure(GcClosure {
            func: import_object(heap, &Object::CompiledFunction(Rc::clone(&closure.func))),
            free: closure
                .free
                .iter()
                .map(|item| import_object(heap, item))
                .collect(),
        }),
        Object::Builtin(b) => Value::Builtin(*b),
        Object::ReturnValue(inner) => return import_object(heap, inner),
        Object::Function(_, _, _) => {
            panic!("interpreter functions cannot be imported into the GC VM")
        }
    };
    let edge_refs = value.edge_refs();
    let reference = alloc_value(heap, value);
    for edge in edge_refs {
        heap.free(edge);
    }
    reference
}

pub fn export_object(heap: &GcHeap, reference: GcRef) -> Object {
    match get_value(heap, reference) {
        Value::Integer(i) => Object::Integer(*i),
        Value::Boolean(b) => Object::Boolean(*b),
        Value::String(s) => Object::String(s.clone()),
        Value::Null => Object::Null,
        Value::Error(e) => Object::Error(e.clone()),
        Value::Array(items) => Object::Array(
            items
                .iter()
                .map(|item| Rc::new(export_object(heap, *item)))
                .collect(),
        ),
        Value::Hash(map) => Object::Hash(
            map.iter()
                .map(|(k, v)| (Rc::new(k.to_object()), Rc::new(export_object(heap, *v))))
                .collect(),
        ),
        Value::CompiledFunction(f) => Object::CompiledFunction(Rc::new(f.clone())),
        Value::Closure(closure) => {
            let func = match get_value(heap, closure.func) {
                Value::CompiledFunction(f) => Rc::new(f.clone()),
                _ => panic!("closure func must be compiled function"),
            };
            Object::ClosureObj(Closure {
                func,
                free: closure
                    .free
                    .iter()
                    .map(|item| Rc::new(export_object(heap, *item)))
                    .collect(),
            })
        }
        Value::Builtin(b) => Object::Builtin(*b),
    }
}

pub fn call_builtin(heap: &mut GcHeap, builtin: BuiltinFunc, args: Vec<GcRef>) -> GcRef {
    let rc_args = args
        .iter()
        .map(|reference| Rc::new(export_object(heap, *reference)))
        .collect();
    let result = builtin(rc_args);
    import_object(heap, &result)
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
