//! Storage/execution agnostic runtime semantics (design §5.2, §8, §10.1).
//!
//! Everything here is shared between the native runtime (`runtime.rs`) and
//! the wasm simulator: tagged value encoding, the frozen semantics matrix
//! (checked `i64` arithmetic, equality, truthiness, indexing, builtins),
//! the canonical observer encoding, and call/construct dispatch. Failures
//! never exit the process at this layer; they are returned as stable
//! [`RuntimeErrorKind`] categories.

use std::collections::HashMap;

use object::builtins::{BuiltIns, BuiltinId};

use crate::runtime_backend::{CodeHandle, ValueStore};

/// 64-bit tagged value (design §5.2).
pub type Value = u64;

pub const FALSE_VALUE: Value = 0b0011;
pub const TRUE_VALUE: Value = 0b0111;
pub const NULL_VALUE: Value = 0b1011;

pub const PTR_TAG_MASK: u64 = 0b111;
pub const HEAP_TAG: u64 = 0b001;
pub const BUILTIN_TAG: u64 = 0b101;

/// SMIs cover `[-2^62, 2^62 - 1]`; the rest of the `i64` range is boxed.
pub const SMI_MIN: i64 = -(1 << 62);
pub const SMI_MAX: i64 = (1 << 62) - 1;

/// Hard cap shared with the calling convention: a closure body receives its
/// arguments in `x1..x7` (design §2.2, §7).
pub const MAX_INVOKE_ARGS: usize = 7;

pub fn is_smi(value: Value) -> bool {
    value & 1 == 0
}

pub fn smi_to_i64(value: Value) -> i64 {
    (value as i64) >> 1
}

pub fn i64_fits_smi(raw: i64) -> bool {
    (SMI_MIN..=SMI_MAX).contains(&raw)
}

pub fn smi_from_i64(raw: i64) -> Value {
    debug_assert!(i64_fits_smi(raw));
    (raw << 1) as u64
}

pub fn is_heap(value: Value) -> bool {
    value & PTR_TAG_MASK == HEAP_TAG
}

pub fn is_builtin(value: Value) -> bool {
    value & PTR_TAG_MASK == BUILTIN_TAG
}

pub fn bool_value(value: bool) -> Value {
    if value {
        TRUE_VALUE
    } else {
        FALSE_VALUE
    }
}

/// Frozen builtin numbering used by the immediate encoding
/// `(ordinal << 3) | 0b101`. `print` shares `BuiltinId::Puts` and therefore
/// the same ordinal (see `object/builtins.rs`).
pub fn builtin_ordinal(id: BuiltinId) -> u64 {
    match id {
        BuiltinId::Len => 0,
        BuiltinId::Puts => 1,
        BuiltinId::First => 2,
        BuiltinId::Last => 3,
        BuiltinId::Rest => 4,
        BuiltinId::Push => 5,
    }
}

pub fn builtin_from_ordinal(ordinal: u64) -> Option<BuiltinId> {
    match ordinal {
        0 => Some(BuiltinId::Len),
        1 => Some(BuiltinId::Puts),
        2 => Some(BuiltinId::First),
        3 => Some(BuiltinId::Last),
        4 => Some(BuiltinId::Rest),
        5 => Some(BuiltinId::Push),
        _ => None,
    }
}

pub fn builtin_value(id: BuiltinId) -> Value {
    (builtin_ordinal(id) << 3) | BUILTIN_TAG
}

/// Canonical name used by the observer protocol (`print` normalizes to
/// `puts`).
pub fn builtin_canonical_name(id: BuiltinId) -> &'static str {
    match id {
        BuiltinId::Len => "len",
        BuiltinId::Puts => "puts",
        BuiltinId::First => "first",
        BuiltinId::Last => "last",
        BuiltinId::Rest => "rest",
        BuiltinId::Push => "push",
    }
}

/// Builtin id for a `SymbolScope::Builtin` symbol index (the index into
/// `object::builtins::BuiltIns`, where `print` aliases `puts`).
pub fn builtin_id_for_symbol_index(index: usize) -> Option<BuiltinId> {
    BuiltIns.get(index).map(|definition| definition.id)
}

/// Stable error categories, frozen ABI between `.s` and the runtime
/// (design §8). Tests compare kinds, never messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum RuntimeErrorKind {
    InternalError = 0,
    TypeError = 1,
    ArityError = 2,
    NotCallable = 3,
    NotConstructable = 4,
    MissingProperty = 5,
    InvalidHashKey = 6,
    DivisionByZero = 7,
    IntegerOverflow = 8,
    ResourceLimit = 9,
}

impl RuntimeErrorKind {
    pub fn name(self) -> &'static str {
        match self {
            RuntimeErrorKind::InternalError => "InternalError",
            RuntimeErrorKind::TypeError => "TypeError",
            RuntimeErrorKind::ArityError => "ArityError",
            RuntimeErrorKind::NotCallable => "NotCallable",
            RuntimeErrorKind::NotConstructable => "NotConstructable",
            RuntimeErrorKind::MissingProperty => "MissingProperty",
            RuntimeErrorKind::InvalidHashKey => "InvalidHashKey",
            RuntimeErrorKind::DivisionByZero => "DivisionByZero",
            RuntimeErrorKind::IntegerOverflow => "IntegerOverflow",
            RuntimeErrorKind::ResourceLimit => "ResourceLimit",
        }
    }

    pub fn from_u64(kind: u64) -> Option<RuntimeErrorKind> {
        match kind {
            0 => Some(RuntimeErrorKind::InternalError),
            1 => Some(RuntimeErrorKind::TypeError),
            2 => Some(RuntimeErrorKind::ArityError),
            3 => Some(RuntimeErrorKind::NotCallable),
            4 => Some(RuntimeErrorKind::NotConstructable),
            5 => Some(RuntimeErrorKind::MissingProperty),
            6 => Some(RuntimeErrorKind::InvalidHashKey),
            7 => Some(RuntimeErrorKind::DivisionByZero),
            8 => Some(RuntimeErrorKind::IntegerOverflow),
            9 => Some(RuntimeErrorKind::ResourceLimit),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFailure {
    pub kind: RuntimeErrorKind,
    pub message: String,
}

pub type RuntimeResult<T> = Result<T, RuntimeFailure>;

fn fail<T>(kind: RuntimeErrorKind, message: impl Into<String>) -> RuntimeResult<T> {
    Err(RuntimeFailure {
        kind,
        message: message.into(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum HashKey {
    Integer(i64),
    Boolean(bool),
    Str(String),
}

impl HashKey {
    /// Rank + canonical bytes ordering used by displays and the observer
    /// (integer=0, boolean=1, string=2; design §10.2).
    fn rank(&self) -> u8 {
        match self {
            HashKey::Integer(_) => 0,
            HashKey::Boolean(_) => 1,
            HashKey::Str(_) => 2,
        }
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        match self {
            HashKey::Integer(raw) => raw.to_string().into_bytes(),
            HashKey::Boolean(raw) => raw.to_string().into_bytes(),
            HashKey::Str(raw) => raw.clone().into_bytes(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClosureData {
    pub code: CodeHandle,
    pub num_parameters: u64,
    pub free: Vec<Value>,
}

#[derive(Clone, Debug)]
pub struct ClassData {
    pub name: String,
    pub methods: HashMap<String, Value>,
    pub constructor: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct InstanceData {
    pub class: Value,
    pub fields: HashMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct BoundMethodData {
    pub receiver: Value,
    pub method: Value,
    pub name: String,
}

/// Heap object layout is fully opaque to generated assembly (design §5.2),
/// which is what lets it be plain Rust data.
#[derive(Clone, Debug)]
pub enum HeapObject {
    BoxedInt(i64),
    Str(String),
    Array(Vec<Value>),
    Hash(HashMap<HashKey, Value>),
    Closure(ClosureData),
    Class(ClassData),
    Instance(InstanceData),
    BoundMethod(BoundMethodData),
}

fn get_obj<S: ValueStore>(store: &S, value: Value) -> RuntimeResult<&HeapObject> {
    match store.try_get(value) {
        Some(object) => Ok(object),
        None => fail(RuntimeErrorKind::InternalError, "invalid heap reference"),
    }
}

/// Raw integer behind either representation (SMI or boxed).
pub fn int_value<S: ValueStore>(store: &S, value: Value) -> Option<i64> {
    if is_smi(value) {
        return Some(smi_to_i64(value));
    }
    match store.try_get(value) {
        Some(HeapObject::BoxedInt(raw)) => Some(*raw),
        _ => None,
    }
}

/// SMI when it fits, boxed integer otherwise; results shrink back to SMI
/// whenever possible.
pub fn make_int<S: ValueStore>(store: &mut S, raw: i64) -> Value {
    if i64_fits_smi(raw) {
        smi_from_i64(raw)
    } else {
        store.alloc(HeapObject::BoxedInt(raw))
    }
}

/// Frozen truthiness: only `false` and `null` are falsy (design §10.1).
pub fn truthy(value: Value) -> bool {
    value != FALSE_VALUE && value != NULL_VALUE
}

pub fn string_from_utf8<S: ValueStore>(store: &mut S, bytes: &[u8]) -> RuntimeResult<Value> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(store.alloc(HeapObject::Str(text.to_string()))),
        Err(_) => fail(RuntimeErrorKind::InternalError, "string literal is not valid UTF-8"),
    }
}

pub fn array_from_values<S: ValueStore>(store: &mut S, values: &[Value]) -> Value {
    store.alloc(HeapObject::Array(values.to_vec()))
}

pub fn hash_key<S: ValueStore>(store: &S, value: Value) -> Option<HashKey> {
    if let Some(raw) = int_value(store, value) {
        return Some(HashKey::Integer(raw));
    }
    match value {
        TRUE_VALUE => Some(HashKey::Boolean(true)),
        FALSE_VALUE => Some(HashKey::Boolean(false)),
        _ => match store.try_get(value) {
            Some(HeapObject::Str(text)) => Some(HashKey::Str(text.clone())),
            _ => None,
        },
    }
}

/// `pairs` is `k0,v0,k1,v1,…`; later duplicates win, like the VMs.
pub fn hash_from_pairs<S: ValueStore>(store: &mut S, pairs: &[Value]) -> RuntimeResult<Value> {
    debug_assert_eq!(pairs.len() % 2, 0);
    let mut entries = HashMap::new();
    for pair in pairs.chunks_exact(2) {
        let key = match hash_key(store, pair[0]) {
            Some(key) => key,
            None => {
                let shown = display(store, pair[0])?;
                return fail(
                    RuntimeErrorKind::InvalidHashKey,
                    format!("hash key must be hashable, got {}", shown),
                );
            }
        };
        entries.insert(key, pair[1]);
    }
    Ok(store.alloc(HeapObject::Hash(entries)))
}

pub fn closure_new<S: ValueStore>(
    store: &mut S,
    code: CodeHandle,
    num_parameters: u64,
    free: &[Value],
) -> RuntimeResult<Value> {
    if num_parameters as usize > MAX_INVOKE_ARGS {
        return fail(
            RuntimeErrorKind::ResourceLimit,
            format!("closure cannot take more than {} parameters", MAX_INVOKE_ARGS),
        );
    }
    Ok(store.alloc(HeapObject::Closure(ClosureData {
        code,
        num_parameters,
        free: free.to_vec(),
    })))
}

/// v1's only free-variable access path (design §7).
pub fn get_free<S: ValueStore>(store: &S, closure: Value, index: u64) -> RuntimeResult<Value> {
    match get_obj(store, closure)? {
        HeapObject::Closure(data) => match data.free.get(index as usize) {
            Some(value) => Ok(*value),
            None => fail(RuntimeErrorKind::InternalError, "free variable index out of range"),
        },
        _ => fail(RuntimeErrorKind::InternalError, "rt_get_free on a non-closure"),
    }
}

pub fn class_new<S: ValueStore>(store: &mut S, name: &str) -> Value {
    store.alloc(HeapObject::Class(ClassData {
        name: name.to_string(),
        methods: HashMap::new(),
        constructor: None,
    }))
}

pub fn class_add_method<S: ValueStore>(
    store: &mut S,
    class: Value,
    name: &str,
    method: Value,
    is_constructor: bool,
) -> RuntimeResult<()> {
    if !matches!(get_obj(store, method)?, HeapObject::Closure(_)) {
        return fail(RuntimeErrorKind::InternalError, "class method must be a closure");
    }
    match store.try_get_mut(class) {
        Some(HeapObject::Class(data)) => {
            if is_constructor {
                data.constructor = Some(method);
            } else {
                data.methods.insert(name.to_string(), method);
            }
            Ok(())
        }
        _ => fail(RuntimeErrorKind::InternalError, "cannot install a method on a non-class"),
    }
}

/// Field first, then a freshly bound method; missing → `MissingProperty`
/// (design §8, matching the VMs' error surface).
pub fn get_property<S: ValueStore>(
    store: &mut S,
    object: Value,
    name: &str,
) -> RuntimeResult<Value> {
    let (class, field) = match store.try_get(object) {
        Some(HeapObject::Instance(instance)) => {
            (instance.class, instance.fields.get(name).copied())
        }
        _ => {
            let shown = display(store, object)?;
            return fail(
                RuntimeErrorKind::TypeError,
                format!("cannot read property '{}' of {}", name, shown),
            );
        }
    };
    if let Some(field) = field {
        return Ok(field);
    }
    let (class_name, method) = match get_obj(store, class)? {
        HeapObject::Class(data) => (data.name.clone(), data.methods.get(name).copied()),
        _ => return fail(RuntimeErrorKind::InternalError, "instance has an invalid class"),
    };
    match method {
        Some(method) => Ok(store.alloc(HeapObject::BoundMethod(BoundMethodData {
            receiver: object,
            method,
            name: name.to_string(),
        }))),
        None => fail(
            RuntimeErrorKind::MissingProperty,
            format!("property '{}' does not exist on {}", name, class_name),
        ),
    }
}

pub fn set_property<S: ValueStore>(
    store: &mut S,
    object: Value,
    name: &str,
    value: Value,
) -> RuntimeResult<()> {
    match store.try_get_mut(object) {
        Some(HeapObject::Instance(instance)) => {
            instance.fields.insert(name.to_string(), value);
            Ok(())
        }
        _ => {
            let shown = display(store, object)?;
            fail(
                RuntimeErrorKind::TypeError,
                format!("cannot set property '{}' of {}", name, shown),
            )
        }
    }
}

/// Array out-of-bounds and hash missing key are `null`; wrong container or
/// index types are `TypeError`; unhashable hash keys are `InvalidHashKey`
/// (design §10.1).
pub fn index<S: ValueStore>(store: &S, object: Value, index: Value) -> RuntimeResult<Value> {
    if is_heap(object) {
        match get_obj(store, object)? {
            HeapObject::Array(elements) => {
                if let Some(position) = int_value(store, index) {
                    if position >= 0 && (position as usize) < elements.len() {
                        return Ok(elements[position as usize]);
                    }
                    return Ok(NULL_VALUE);
                }
            }
            HeapObject::Hash(entries) => {
                let key = match hash_key(store, index) {
                    Some(key) => key,
                    None => {
                        return fail(RuntimeErrorKind::InvalidHashKey, "unsupported hash index key")
                    }
                };
                return Ok(entries.get(&key).copied().unwrap_or(NULL_VALUE));
            }
            _ => {}
        }
    }
    let object_shown = display(store, object)?;
    let index_shown = display(store, index)?;
    fail(
        RuntimeErrorKind::TypeError,
        format!("unsupported index operation for {} and {}", object_shown, index_shown),
    )
}

/// Frozen equality matrix (design §10.1): integers by raw value, scalars by
/// value, builtins by id, aggregates recursively, identity types by object
/// identity, differing types compare unequal.
pub fn eq_values<S: ValueStore>(store: &S, left: Value, right: Value) -> RuntimeResult<bool> {
    let left_int = int_value(store, left);
    let right_int = int_value(store, right);
    if let (Some(l), Some(r)) = (left_int, right_int) {
        return Ok(l == r);
    }
    if left_int.is_some() != right_int.is_some() {
        return Ok(false);
    }
    if !is_heap(left) || !is_heap(right) {
        // true/false/null singletons and builtin immediates: bit equality is
        // value (respectively id) equality; mixed immediate/heap is unequal.
        return Ok(left == right);
    }
    match (get_obj(store, left)?, get_obj(store, right)?) {
        (HeapObject::Str(l), HeapObject::Str(r)) => Ok(l == r),
        (HeapObject::Array(l), HeapObject::Array(r)) => {
            if l.len() != r.len() {
                return Ok(false);
            }
            for (l_element, r_element) in l.iter().zip(r.iter()) {
                if !eq_values(store, *l_element, *r_element)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (HeapObject::Hash(l), HeapObject::Hash(r)) => {
            if l.len() != r.len() {
                return Ok(false);
            }
            for (key, l_value) in l.iter() {
                match r.get(key) {
                    Some(r_value) => {
                        if !eq_values(store, *l_value, *r_value)? {
                            return Ok(false);
                        }
                    }
                    None => return Ok(false),
                }
            }
            Ok(true)
        }
        (HeapObject::Closure(_), HeapObject::Closure(_))
        | (HeapObject::Class(_), HeapObject::Class(_))
        | (HeapObject::Instance(_), HeapObject::Instance(_))
        | (HeapObject::BoundMethod(_), HeapObject::BoundMethod(_)) => Ok(left == right),
        _ => Ok(false),
    }
}

/// `>` accepts integers only (design §10.1).
pub fn gt<S: ValueStore>(store: &S, left: Value, right: Value) -> RuntimeResult<Value> {
    if let (Some(l), Some(r)) = (int_value(store, left), int_value(store, right)) {
        return Ok(bool_value(l > r));
    }
    let left_shown = display(store, left)?;
    let right_shown = display(store, right)?;
    fail(
        RuntimeErrorKind::TypeError,
        format!("unsupported comparison for {} and {}", left_shown, right_shown),
    )
}

fn checked_arith<S: ValueStore>(
    store: &mut S,
    left: Value,
    right: Value,
    operation: &str,
    apply: impl Fn(i64, i64) -> Option<i64>,
) -> RuntimeResult<Value> {
    if let (Some(l), Some(r)) = (int_value(store, left), int_value(store, right)) {
        return match apply(l, r) {
            Some(raw) => Ok(make_int(store, raw)),
            None => fail(
                RuntimeErrorKind::IntegerOverflow,
                format!("integer overflow in {}", operation),
            ),
        };
    }
    let left_shown = display(store, left)?;
    let right_shown = display(store, right)?;
    fail(
        RuntimeErrorKind::TypeError,
        format!("unsupported binary operation for {} and {}", left_shown, right_shown),
    )
}

/// Checked addition; also string concatenation (design §8).
pub fn add<S: ValueStore>(store: &mut S, left: Value, right: Value) -> RuntimeResult<Value> {
    if int_value(store, left).is_none() {
        if let (Some(HeapObject::Str(l)), Some(HeapObject::Str(r))) =
            (store.try_get(left), store.try_get(right))
        {
            let combined = format!("{}{}", l, r);
            return Ok(store.alloc(HeapObject::Str(combined)));
        }
    }
    checked_arith(store, left, right, "addition", i64::checked_add)
}

pub fn sub<S: ValueStore>(store: &mut S, left: Value, right: Value) -> RuntimeResult<Value> {
    checked_arith(store, left, right, "subtraction", i64::checked_sub)
}

pub fn mul<S: ValueStore>(store: &mut S, left: Value, right: Value) -> RuntimeResult<Value> {
    checked_arith(store, left, right, "multiplication", i64::checked_mul)
}

/// Division separates `DivisionByZero` first, then truncates toward zero via
/// checked `i64` division (`i64::MIN / -1` → `IntegerOverflow`).
pub fn div<S: ValueStore>(store: &mut S, left: Value, right: Value) -> RuntimeResult<Value> {
    if let (Some(_), Some(0)) = (int_value(store, left), int_value(store, right)) {
        return fail(RuntimeErrorKind::DivisionByZero, "division by zero");
    }
    checked_arith(store, left, right, "division", i64::checked_div)
}

pub fn minus<S: ValueStore>(store: &mut S, value: Value) -> RuntimeResult<Value> {
    if let Some(raw) = int_value(store, value) {
        return match raw.checked_neg() {
            Some(negated) => Ok(make_int(store, negated)),
            None => fail(RuntimeErrorKind::IntegerOverflow, "integer overflow in negation"),
        };
    }
    let shown = display(store, value)?;
    fail(RuntimeErrorKind::TypeError, format!("unsupported type for negation: {}", shown))
}

/// `!v` is strictly the logical inverse of truthiness (design §10.1).
pub fn bang(value: Value) -> Value {
    bool_value(!truthy(value))
}

/// Hash entries in canonical order: `(key type rank, canonical key bytes)`.
fn sorted_hash_entries(entries: &HashMap<HashKey, Value>) -> Vec<(&HashKey, Value)> {
    let mut sorted: Vec<(&HashKey, Value)> =
        entries.iter().map(|(key, value)| (key, *value)).collect();
    sorted.sort_by(|(a, _), (b, _)| {
        (a.rank(), a.canonical_bytes()).cmp(&(b.rank(), b.canonical_bytes()))
    });
    sorted
}

fn key_display(key: &HashKey) -> String {
    match key {
        HashKey::Integer(raw) => raw.to_string(),
        HashKey::Boolean(raw) => raw.to_string(),
        HashKey::Str(raw) => raw.clone(),
    }
}

fn instance_class_name<S: ValueStore>(store: &S, instance: Value) -> RuntimeResult<String> {
    match get_obj(store, instance)? {
        HeapObject::Instance(data) => match get_obj(store, data.class)? {
            HeapObject::Class(class) => Ok(class.name.clone()),
            _ => fail(RuntimeErrorKind::InternalError, "instance has an invalid class"),
        },
        _ => fail(RuntimeErrorKind::InternalError, "expected an instance"),
    }
}

/// Shared language display (design §10.2): what `puts` prints.
pub fn display<S: ValueStore>(store: &S, value: Value) -> RuntimeResult<String> {
    if let Some(raw) = int_value(store, value) {
        return Ok(raw.to_string());
    }
    match value {
        TRUE_VALUE => return Ok("true".to_string()),
        FALSE_VALUE => return Ok("false".to_string()),
        NULL_VALUE => return Ok("null".to_string()),
        _ => {}
    }
    if is_builtin(value) {
        return Ok("[builtin function]".to_string());
    }
    match get_obj(store, value)? {
        HeapObject::BoxedInt(_) => unreachable!("handled by int_value"),
        HeapObject::Str(text) => Ok(text.clone()),
        HeapObject::Array(elements) => {
            let mut rendered = Vec::with_capacity(elements.len());
            for element in elements {
                rendered.push(display(store, *element)?);
            }
            Ok(format!("[{}]", rendered.join(", ")))
        }
        HeapObject::Hash(entries) => {
            let mut rendered = Vec::with_capacity(entries.len());
            for (key, entry_value) in sorted_hash_entries(entries) {
                rendered.push(format!("{}: {}", key_display(key), display(store, entry_value)?));
            }
            Ok(format!("{{{}}}", rendered.join(", ")))
        }
        HeapObject::Closure(_) => Ok("[function]".to_string()),
        HeapObject::Class(data) => Ok(format!("[class {}]", data.name)),
        HeapObject::Instance(_) => Ok(format!("[object {}]", instance_class_name(store, value)?)),
        HeapObject::BoundMethod(data) => Ok(format!(
            "[bound method {}.{}]",
            instance_class_name(store, data.receiver)?,
            data.name
        )),
    }
}

fn json_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + 2);
    for character in text.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                escaped.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => escaped.push(c),
        }
    }
    escaped
}

/// Canonical tagged JSON encoding for the observer protocol (design §10.2).
pub fn canonical_value<S: ValueStore>(store: &S, value: Value) -> RuntimeResult<String> {
    if let Some(raw) = int_value(store, value) {
        return Ok(format!("{{\"type\":\"integer\",\"value\":\"{}\"}}", raw));
    }
    match value {
        TRUE_VALUE => return Ok("{\"type\":\"boolean\",\"value\":true}".to_string()),
        FALSE_VALUE => return Ok("{\"type\":\"boolean\",\"value\":false}".to_string()),
        NULL_VALUE => return Ok("{\"type\":\"null\"}".to_string()),
        _ => {}
    }
    if is_builtin(value) {
        let id = match builtin_from_ordinal(value >> 3) {
            Some(id) => id,
            None => return fail(RuntimeErrorKind::InternalError, "invalid builtin encoding"),
        };
        return Ok(format!("{{\"type\":\"builtin\",\"id\":\"{}\"}}", builtin_canonical_name(id)));
    }
    match get_obj(store, value)? {
        HeapObject::BoxedInt(_) => unreachable!("handled by int_value"),
        HeapObject::Str(text) => {
            Ok(format!("{{\"type\":\"string\",\"value\":\"{}\"}}", json_escape(text)))
        }
        HeapObject::Array(elements) => {
            let mut rendered = Vec::with_capacity(elements.len());
            for element in elements {
                rendered.push(canonical_value(store, *element)?);
            }
            Ok(format!("{{\"type\":\"array\",\"elements\":[{}]}}", rendered.join(",")))
        }
        HeapObject::Hash(entries) => {
            let mut rendered = Vec::with_capacity(entries.len());
            for (key, entry_value) in sorted_hash_entries(entries) {
                let key_json = match key {
                    HashKey::Integer(raw) => {
                        format!("{{\"type\":\"integer\",\"value\":\"{}\"}}", raw)
                    }
                    HashKey::Boolean(raw) => {
                        format!("{{\"type\":\"boolean\",\"value\":{}}}", raw)
                    }
                    HashKey::Str(raw) => {
                        format!("{{\"type\":\"string\",\"value\":\"{}\"}}", json_escape(raw))
                    }
                };
                rendered.push(format!(
                    "{{\"key\":{},\"value\":{}}}",
                    key_json,
                    canonical_value(store, entry_value)?
                ));
            }
            Ok(format!("{{\"type\":\"hash\",\"entries\":[{}]}}", rendered.join(",")))
        }
        HeapObject::Closure(_) => Ok("{\"type\":\"function\"}".to_string()),
        HeapObject::Class(data) => {
            Ok(format!("{{\"type\":\"class\",\"name\":\"{}\"}}", json_escape(&data.name)))
        }
        HeapObject::Instance(_) => Ok(format!(
            "{{\"type\":\"instance\",\"class\":\"{}\"}}",
            json_escape(&instance_class_name(store, value)?)
        )),
        HeapObject::BoundMethod(data) => Ok(format!(
            "{{\"type\":\"bound_method\",\"class\":\"{}\",\"method\":\"{}\"}}",
            json_escape(&instance_class_name(store, data.receiver)?),
            json_escape(&data.name)
        )),
    }
}

/// Where `puts`/`print` bytes go: stdout natively, a buffer in the simulator
/// and in tests (design §8.1).
pub trait OutputSink {
    fn write_line(&mut self, line: &str);
}

/// Test/simulator sink collecting raw stdout bytes.
pub struct BufferSink {
    pub bytes: Vec<u8>,
}

impl Default for BufferSink {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferSink {
    pub fn new() -> BufferSink {
        BufferSink {
            bytes: vec![],
        }
    }
}

impl OutputSink for BufferSink {
    fn write_line(&mut self, line: &str) {
        self.bytes.extend_from_slice(line.as_bytes());
        self.bytes.push(b'\n');
    }
}

/// Builtins run inside the runtime (no separate FFI symbols); arity/type
/// problems are terminating errors, never `Error` values (design §8).
pub fn call_builtin<S: ValueStore>(
    store: &mut S,
    sink: &mut dyn OutputSink,
    id: BuiltinId,
    args: &[Value],
) -> RuntimeResult<Value> {
    let expect_arity = |count: usize| -> RuntimeResult<()> {
        if args.len() != count {
            return fail(
                RuntimeErrorKind::ArityError,
                format!(
                    "builtin {} expected {} argument{}, got {}",
                    builtin_canonical_name(id),
                    count,
                    if count == 1 { "" } else { "s" },
                    args.len()
                ),
            );
        }
        Ok(())
    };
    match id {
        BuiltinId::Len => {
            expect_arity(1)?;
            let length = match store.try_get(args[0]) {
                Some(HeapObject::Str(text)) => text.len() as i64,
                Some(HeapObject::Array(elements)) => elements.len() as i64,
                _ => {
                    let shown = display(store, args[0])?;
                    return fail(
                        RuntimeErrorKind::TypeError,
                        format!("builtin len not supported for type {}", shown),
                    );
                }
            };
            Ok(make_int(store, length))
        }
        BuiltinId::Puts => {
            for argument in args {
                let line = display(store, *argument)?;
                sink.write_line(&line);
            }
            Ok(NULL_VALUE)
        }
        BuiltinId::First | BuiltinId::Last => {
            expect_arity(1)?;
            match store.try_get(args[0]) {
                Some(HeapObject::Array(elements)) => {
                    let element =
                        if id == BuiltinId::First { elements.first() } else { elements.last() };
                    Ok(element.copied().unwrap_or(NULL_VALUE))
                }
                _ => {
                    let shown = display(store, args[0])?;
                    fail(
                        RuntimeErrorKind::TypeError,
                        format!(
                            "builtin {} not supported for type {}",
                            builtin_canonical_name(id),
                            shown
                        ),
                    )
                }
            }
        }
        BuiltinId::Rest => {
            expect_arity(1)?;
            let rest = match store.try_get(args[0]) {
                Some(HeapObject::Array(elements)) => {
                    if elements.is_empty() {
                        return Ok(NULL_VALUE);
                    }
                    elements[1..].to_vec()
                }
                _ => {
                    let shown = display(store, args[0])?;
                    return fail(
                        RuntimeErrorKind::TypeError,
                        format!("builtin rest not supported for type {}", shown),
                    );
                }
            };
            Ok(store.alloc(HeapObject::Array(rest)))
        }
        BuiltinId::Push => {
            expect_arity(2)?;
            let pushed = match store.try_get(args[0]) {
                Some(HeapObject::Array(elements)) => {
                    let mut extended = elements.clone();
                    extended.push(args[1]);
                    extended
                }
                _ => {
                    let shown = display(store, args[0])?;
                    return fail(
                        RuntimeErrorKind::TypeError,
                        format!("builtin push not supported for type {}", shown),
                    );
                }
            };
            Ok(store.alloc(HeapObject::Array(pushed)))
        }
    }
}

/// How the execution adapter finishes an `Invoke` (design §8.1):
/// constructors always yield their instance.
#[derive(Clone, Debug, PartialEq)]
pub enum ReturnPolicy {
    Direct,
    ConstructorInstance(Value),
}

/// Dispatch result: builtins resolve immediately, code invocations bounce
/// back through the execution adapter (real function pointer natively, a
/// simulated PC in wasm).
#[derive(Clone, Debug)]
pub enum CallDispatch {
    Return(Value),
    Invoke { code: CodeHandle, closure: Value, args: Vec<Value>, return_policy: ReturnPolicy },
}

fn closure_signature<S: ValueStore>(store: &S, closure: Value) -> Option<(CodeHandle, u64)> {
    match store.try_get(closure) {
        Some(HeapObject::Closure(data)) => Some((data.code, data.num_parameters)),
        _ => None,
    }
}

/// Plain-call dispatch (design §7.2): closures, builtins and bound methods
/// are callable; classes must use `new`.
pub fn dispatch_call<S: ValueStore>(
    store: &mut S,
    sink: &mut dyn OutputSink,
    callee: Value,
    args: &[Value],
) -> RuntimeResult<CallDispatch> {
    if is_builtin(callee) {
        let id = match builtin_from_ordinal(callee >> 3) {
            Some(id) => id,
            None => return fail(RuntimeErrorKind::InternalError, "invalid builtin encoding"),
        };
        return Ok(CallDispatch::Return(call_builtin(store, sink, id, args)?));
    }
    if is_heap(callee) {
        match get_obj(store, callee)? {
            HeapObject::Closure(data) => {
                if data.num_parameters != args.len() as u64 {
                    return fail(
                        RuntimeErrorKind::ArityError,
                        format!(
                            "wrong number of arguments: want={}, got={}",
                            data.num_parameters,
                            args.len()
                        ),
                    );
                }
                return Ok(CallDispatch::Invoke {
                    code: data.code,
                    closure: callee,
                    args: args.to_vec(),
                    return_policy: ReturnPolicy::Direct,
                });
            }
            HeapObject::BoundMethod(bound) => {
                let (receiver, method, name) = (bound.receiver, bound.method, bound.name.clone());
                let (code, num_parameters) = match closure_signature(store, method) {
                    Some(signature) => signature,
                    None => {
                        return fail(
                            RuntimeErrorKind::InternalError,
                            "bound method is not a closure",
                        )
                    }
                };
                let expected = num_parameters.saturating_sub(1);
                if expected != args.len() as u64 {
                    let class_name = instance_class_name(store, receiver)?;
                    return fail(
                        RuntimeErrorKind::ArityError,
                        format!(
                            "wrong number of arguments for {}.{}: want={}, got={}",
                            class_name,
                            name,
                            expected,
                            args.len()
                        ),
                    );
                }
                let mut invoke_args = Vec::with_capacity(args.len() + 1);
                invoke_args.push(receiver);
                invoke_args.extend_from_slice(args);
                return Ok(CallDispatch::Invoke {
                    code,
                    closure: method,
                    args: invoke_args,
                    return_policy: ReturnPolicy::Direct,
                });
            }
            HeapObject::Class(data) => {
                return fail(
                    RuntimeErrorKind::NotCallable,
                    format!("class {} must be constructed with new", data.name),
                );
            }
            _ => {}
        }
    }
    let shown = display(store, callee)?;
    fail(RuntimeErrorKind::NotCallable, format!("cannot call {}", shown))
}

/// `new` dispatch (design §7.2): callee must be a class; the constructor —
/// when present — runs with the fresh instance as `this` and the instance is
/// always the result.
pub fn dispatch_construct<S: ValueStore>(
    store: &mut S,
    callee: Value,
    args: &[Value],
) -> RuntimeResult<CallDispatch> {
    let (class_name, constructor) = match store.try_get(callee) {
        Some(HeapObject::Class(data)) => (data.name.clone(), data.constructor),
        _ => {
            let shown = display(store, callee)?;
            return fail(RuntimeErrorKind::NotConstructable, format!("cannot construct {}", shown));
        }
    };
    let constructor = match constructor {
        None => {
            if !args.is_empty() {
                return fail(
                    RuntimeErrorKind::ArityError,
                    format!(
                        "wrong number of arguments for {}.constructor: want=0, got={}",
                        class_name,
                        args.len()
                    ),
                );
            }
            let instance = store.alloc(HeapObject::Instance(InstanceData {
                class: callee,
                fields: HashMap::new(),
            }));
            return Ok(CallDispatch::Return(instance));
        }
        Some(constructor) => constructor,
    };
    let (code, num_parameters) = match closure_signature(store, constructor) {
        Some(signature) => signature,
        None => return fail(RuntimeErrorKind::InternalError, "constructor is not a closure"),
    };
    let expected = num_parameters.saturating_sub(1);
    if expected != args.len() as u64 {
        return fail(
            RuntimeErrorKind::ArityError,
            format!(
                "wrong number of arguments for {}.constructor: want={}, got={}",
                class_name,
                expected,
                args.len()
            ),
        );
    }
    let instance = store.alloc(HeapObject::Instance(InstanceData {
        class: callee,
        fields: HashMap::new(),
    }));
    let mut invoke_args = Vec::with_capacity(args.len() + 1);
    invoke_args.push(instance);
    invoke_args.extend_from_slice(args);
    Ok(CallDispatch::Invoke {
        code,
        closure: constructor,
        args: invoke_args,
        return_policy: ReturnPolicy::ConstructorInstance(instance),
    })
}
