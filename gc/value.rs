use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Write as _;
use std::rc::Rc;

use object::builtins::{BuiltIns, BuiltinId};
use object::{Closure, CompiledFunction, Object};
use serde::Serialize;

use crate::header::GcObjectType;
use crate::{GcHeap, GcObject, GcRef};

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
    Builtin(BuiltinId),
    Class(GcClass),
    Instance(GcInstance),
    BoundMethod(GcBoundMethod),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcClosure {
    pub func: GcRef,
    pub free: Vec<GcRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcClass {
    pub name: String,
    pub constructor: Option<GcRef>,
    pub methods: HashMap<String, GcRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcInstance {
    pub class: GcRef,
    pub fields: HashMap<String, GcRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcBoundMethod {
    pub receiver: GcRef,
    pub method: GcRef,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueKind {
    Class,
    Instance,
    BoundMethod,
    Closure,
    Array,
    Hash,
    Integer,
    Boolean,
    String,
    Null,
    Error,
    CompiledFunction,
    Builtin,
    Other,
}

/// Typed heap-edge relation used by teaching diagnostics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EdgeRelation {
    ArrayElement {
        index: usize,
    },
    HashValue {
        #[serde(rename = "keyKind")]
        key_kind: HashKeyKind,
        key: String,
    },
    ClosureFunction,
    ClosureFree {
        index: usize,
    },
    ClassConstructor,
    ClassMethod {
        name: String,
    },
    InstanceClass,
    InstanceField {
        name: String,
    },
    BoundMethodReceiver,
    BoundMethodFunction,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HashKeyKind {
    Integer,
    Boolean,
    String,
}

pub const MAX_HASH_KEY_LABEL_LEN: usize = 64;

pub fn format_hash_key_label(key: &HashKey) -> String {
    match key {
        HashKey::Integer(value) => value.to_string(),
        HashKey::Boolean(value) => value.to_string(),
        HashKey::String(value) => escape_and_truncate_key(value),
    }
}

fn escape_and_truncate_key(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len().min(MAX_HASH_KEY_LABEL_LEN) + 3);
    for ch in value.chars() {
        let mut char_buffer = [0; 4];
        let mut control_buffer = String::new();
        let encoded = match ch {
            '\\' => "\\\\",
            '"' => "\\\"",
            '\n' => "\\n",
            '\r' => "\\r",
            '\t' => "\\t",
            c if c.is_control() => {
                write!(&mut control_buffer, "\\u{:04x}", c as u32)
                    .expect("writing to a String cannot fail");
                control_buffer.as_str()
            }
            c => c.encode_utf8(&mut char_buffer),
        };
        if escaped.len() + encoded.len() > MAX_HASH_KEY_LABEL_LEN {
            escaped.push('…');
            break;
        }
        escaped.push_str(encoded);
    }
    escaped
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HashKey {
    Integer(i64),
    Boolean(bool),
    String(String),
}

impl HashKey {
    pub fn kind(&self) -> HashKeyKind {
        match self {
            HashKey::Integer(_) => HashKeyKind::Integer,
            HashKey::Boolean(_) => HashKeyKind::Boolean,
            HashKey::String(_) => HashKeyKind::String,
        }
    }
}

pub struct ValueCell {
    pub value: Value,
}

impl GcObject for ValueCell {
    fn trace(&self, visit: &mut dyn FnMut(crate::GcId)) {
        self.value.trace(&mut |reference| visit(reference.0));
    }
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Class(_) => ValueKind::Class,
            Value::Instance(_) => ValueKind::Instance,
            Value::BoundMethod(_) => ValueKind::BoundMethod,
            Value::Closure(_) => ValueKind::Closure,
            Value::Array(_) => ValueKind::Array,
            Value::Hash(_) => ValueKind::Hash,
            Value::Integer(_) => ValueKind::Integer,
            Value::Boolean(_) => ValueKind::Boolean,
            Value::String(_) => ValueKind::String,
            Value::Null => ValueKind::Null,
            Value::Error(_) => ValueKind::Error,
            Value::CompiledFunction(_) => ValueKind::CompiledFunction,
            Value::Builtin(_) => ValueKind::Builtin,
        }
    }

    /// Visit heap edges with typed structural relations for diagnostics.
    ///
    /// Hash keys, class methods, and instance fields are visited in stable
    /// sorted order so reports stay deterministic. The collector's `trace`
    /// path remains allocation-free and visits only edge targets.
    pub fn visit_edges(&self, mut visit: impl FnMut(EdgeRelation, GcRef)) {
        match self {
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    visit(
                        EdgeRelation::ArrayElement {
                            index,
                        },
                        *item,
                    );
                }
            }
            Value::Hash(map) => {
                let mut entries = map.iter().collect::<Vec<_>>();
                entries.sort_by(|(left, _), (right, _)| left.cmp(right));
                for (key, value) in entries {
                    visit(
                        EdgeRelation::HashValue {
                            key_kind: key.kind(),
                            key: format_hash_key_label(key),
                        },
                        *value,
                    );
                }
            }
            Value::Closure(closure) => {
                visit(EdgeRelation::ClosureFunction, closure.func);
                for (index, free) in closure.free.iter().enumerate() {
                    visit(
                        EdgeRelation::ClosureFree {
                            index,
                        },
                        *free,
                    );
                }
            }
            Value::Class(class) => {
                if let Some(constructor) = class.constructor {
                    visit(EdgeRelation::ClassConstructor, constructor);
                }
                let mut methods = class.methods.iter().collect::<Vec<_>>();
                methods.sort_by(|(left, _), (right, _)| left.cmp(right));
                for (name, method) in methods {
                    visit(
                        EdgeRelation::ClassMethod {
                            name: name.clone(),
                        },
                        *method,
                    );
                }
            }
            Value::Instance(instance) => {
                visit(EdgeRelation::InstanceClass, instance.class);
                let mut fields = instance.fields.iter().collect::<Vec<_>>();
                fields.sort_by(|(left, _), (right, _)| left.cmp(right));
                for (name, value) in fields {
                    visit(
                        EdgeRelation::InstanceField {
                            name: name.clone(),
                        },
                        *value,
                    );
                }
            }
            Value::BoundMethod(method) => {
                visit(EdgeRelation::BoundMethodReceiver, method.receiver);
                visit(EdgeRelation::BoundMethodFunction, method.method);
            }
            // Leaf variants own no GcRef. No catch-all: adding a Value variant
            // must fail to compile until its edges are classified here.
            Value::Integer(_)
            | Value::Boolean(_)
            | Value::String(_)
            | Value::Null
            | Value::Error(_)
            | Value::CompiledFunction(_)
            | Value::Builtin(_) => {}
        }
    }

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
            Value::Class(class) => {
                if let Some(constructor) = class.constructor {
                    visit(constructor);
                }
                for method in class.methods.values() {
                    visit(*method);
                }
            }
            Value::Instance(instance) => {
                visit(instance.class);
                for field in instance.fields.values() {
                    visit(*field);
                }
            }
            Value::BoundMethod(method) => {
                visit(method.receiver);
                visit(method.method);
            }
            // Leaf variants own no GcRef. No catch-all: adding a Value variant
            // must fail to compile until its edges are classified here.
            Value::Integer(_)
            | Value::Boolean(_)
            | Value::String(_)
            | Value::Null
            | Value::Error(_)
            | Value::CompiledFunction(_)
            | Value::Builtin(_) => {}
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
            Value::Class(mut class) => {
                class.constructor = class.constructor.map(|r| heap.dup(r));
                class.methods = class
                    .methods
                    .into_iter()
                    .map(|(name, method)| (name, heap.dup(method)))
                    .collect();
                Value::Class(class)
            }
            Value::Instance(mut instance) => {
                instance.class = heap.dup(instance.class);
                instance.fields = instance
                    .fields
                    .into_iter()
                    .map(|(name, value)| (name, heap.dup(value)))
                    .collect();
                Value::Instance(instance)
            }
            Value::BoundMethod(mut method) => {
                method.receiver = heap.dup(method.receiver);
                method.method = heap.dup(method.method);
                Value::BoundMethod(method)
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

pub fn get_value(heap: &GcHeap, reference: GcRef) -> &Value {
    &heap
        .runtime()
        .object_downcast::<ValueCell>(reference.0)
        .expect("invalid value reference")
        .value
}

pub fn get_value_mut(heap: &mut GcHeap, reference: GcRef) -> &mut Value {
    &mut heap
        .runtime_mut()
        .object_downcast_mut::<ValueCell>(reference.0)
        .expect("invalid value reference")
        .value
}

pub fn value_to_string(heap: &GcHeap, reference: GcRef) -> String {
    format_reference(heap, reference, &mut HashSet::new())
}

fn format_reference(heap: &GcHeap, reference: GcRef, visited: &mut HashSet<usize>) -> String {
    if !visited.insert(reference.0) {
        return format!("[cycle #{}]", reference.0);
    }
    let formatted = format_value(heap, get_value(heap, reference), visited);
    visited.remove(&reference.0);
    formatted
}

fn format_value(heap: &GcHeap, value: &Value, visited: &mut HashSet<usize>) -> String {
    match value {
        Value::Integer(i) => i.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Error(e) => e.clone(),
        Value::Array(items) => {
            let parts = items
                .iter()
                .map(|item| format_reference(heap, *item, visited))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", parts)
        }
        Value::Hash(map) => {
            let parts = map
                .iter()
                .map(|(k, v)| {
                    format!("{}: {}", format_hash_key(k), format_reference(heap, *v, visited))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{}}}", parts)
        }
        Value::CompiledFunction(_) => "[compiled function]".to_string(),
        Value::Closure(_) => "[closure function]".to_string(),
        Value::Builtin(_) => "[builtin function]".to_string(),
        Value::Class(class) => format!("[class {}]", class.name),
        Value::Instance(instance) => {
            format!("[object {}]", class_name(heap, instance.class))
        }
        Value::BoundMethod(method) => {
            format!("[bound method {}.{}]", instance_class_name(heap, method.receiver), method.name)
        }
    }
}

fn class_name(heap: &GcHeap, class: GcRef) -> String {
    match get_value(heap, class) {
        Value::Class(class) => class.name.clone(),
        _ => "<invalid class>".to_string(),
    }
}

fn instance_class_name(heap: &GcHeap, instance: GcRef) -> String {
    match get_value(heap, instance) {
        Value::Instance(instance) => class_name(heap, instance.class),
        _ => "<invalid receiver>".to_string(),
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
            name: f.name.clone(),
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
        Object::Builtin(function) => {
            let definition = BuiltIns
                .iter()
                .find(|definition| std::ptr::fn_addr_eq(definition.function, *function))
                .expect("unknown builtin function");
            Value::Builtin(definition.id)
        }
        Object::ReturnValue(inner) => return import_object(heap, inner),
        Object::Function(_, _, _) => {
            panic!("interpreter functions cannot be imported into the GC VM")
        }
        Object::Class(_) | Object::Instance(_) | Object::BoundMethod(_) => {
            panic!("graph values cannot be imported into the GC VM")
        }
    };
    let edge_refs = value.edge_refs();
    let reference = alloc_value(heap, value);
    for edge in edge_refs {
        heap.free(edge);
    }
    reference
}

pub fn try_export_object(heap: &GcHeap, reference: GcRef) -> Result<Object, String> {
    match get_value(heap, reference) {
        Value::Integer(i) => Ok(Object::Integer(*i)),
        Value::Boolean(b) => Ok(Object::Boolean(*b)),
        Value::String(s) => Ok(Object::String(s.clone())),
        Value::Null => Ok(Object::Null),
        Value::Error(e) => Ok(Object::Error(e.clone())),
        Value::Array(items) => {
            let mut exported = Vec::with_capacity(items.len());
            for item in items {
                exported.push(Rc::new(try_export_object(heap, *item)?));
            }
            Ok(Object::Array(exported))
        }
        Value::Hash(map) => {
            let mut exported = HashMap::with_capacity(map.len());
            for (key, value) in map {
                exported
                    .insert(Rc::new(key.to_object()), Rc::new(try_export_object(heap, *value)?));
            }
            Ok(Object::Hash(exported))
        }
        Value::CompiledFunction(f) => Ok(Object::CompiledFunction(Rc::new(f.clone()))),
        Value::Closure(closure) => {
            let func = match get_value(heap, closure.func) {
                Value::CompiledFunction(f) => Rc::new(f.clone()),
                _ => return Err("closure func must be compiled function".to_string()),
            };
            let mut free = Vec::with_capacity(closure.free.len());
            for item in &closure.free {
                free.push(Rc::new(try_export_object(heap, *item)?));
            }
            Ok(Object::ClosureObj(Closure {
                func,
                free,
            }))
        }
        Value::Builtin(id) => {
            let definition = BuiltIns
                .iter()
                .find(|definition| definition.id == *id)
                .ok_or_else(|| "unknown builtin id".to_string())?;
            Ok(Object::Builtin(definition.function))
        }
        Value::Class(_) | Value::Instance(_) | Value::BoundMethod(_) => {
            Err("GC graph values cannot be exported as object::Object".to_string())
        }
    }
}

pub fn export_object(heap: &GcHeap, reference: GcRef) -> Object {
    try_export_object(heap, reference).expect("value cannot be exported")
}

pub fn call_builtin(heap: &mut GcHeap, builtin: BuiltinId, args: &[GcRef], null: GcRef) -> GcRef {
    match builtin {
        BuiltinId::Len => {
            if args.len() != 1 {
                return alloc_value(
                    heap,
                    Value::Error(format!("builtin len expected 1 argument, got {}", args.len())),
                );
            }
            match get_value(heap, args[0]) {
                Value::String(value) => alloc_value(heap, Value::Integer(value.len() as i64)),
                Value::Array(value) => alloc_value(heap, Value::Integer(value.len() as i64)),
                _ => alloc_value(
                    heap,
                    Value::Error(format!(
                        "builtin len not supported for for type {}",
                        value_to_string(heap, args[0])
                    )),
                ),
            }
        }
        BuiltinId::Puts => {
            for argument in args {
                println!("{}", value_to_string(heap, *argument));
            }
            heap.dup(null)
        }
        BuiltinId::First | BuiltinId::Last | BuiltinId::Rest => {
            let name = match builtin {
                BuiltinId::First => "first",
                BuiltinId::Last => "last",
                BuiltinId::Rest => "rest",
                _ => unreachable!(),
            };
            if args.len() != 1 {
                return alloc_value(
                    heap,
                    Value::Error(format!(
                        "builtin {} expected 1 argument, got {}",
                        name,
                        args.len()
                    )),
                );
            }
            let items = match get_value(heap, args[0]) {
                Value::Array(items) => items.clone(),
                _ => {
                    return alloc_value(
                        heap,
                        Value::Error(format!(
                            "builtin {} not supported for for type {}",
                            name,
                            value_to_string(heap, args[0])
                        )),
                    )
                }
            };
            match builtin {
                BuiltinId::First => items
                    .first()
                    .map(|item| heap.dup(*item))
                    .unwrap_or_else(|| heap.dup(null)),
                BuiltinId::Last => items
                    .last()
                    .map(|item| heap.dup(*item))
                    .unwrap_or_else(|| heap.dup(null)),
                BuiltinId::Rest => {
                    if items.is_empty() {
                        heap.dup(null)
                    } else {
                        alloc_value(heap, Value::Array(items[1..].to_vec()))
                    }
                }
                _ => unreachable!(),
            }
        }
        BuiltinId::Push => {
            if args.len() != 2 {
                return alloc_value(
                    heap,
                    Value::Error(format!("builtin push expected 2 arguments, got {}", args.len())),
                );
            }
            let mut items = match get_value(heap, args[0]) {
                Value::Array(items) => items.clone(),
                _ => {
                    return alloc_value(
                        heap,
                        Value::Error(format!(
                            "builtin push not supported for for type {}",
                            value_to_string(heap, args[0])
                        )),
                    )
                }
            };
            items.push(args[1]);
            alloc_value(heap, Value::Array(items))
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
