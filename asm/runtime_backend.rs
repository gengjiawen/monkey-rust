//! Storage backends behind the runtime semantics (design §8.1).
//!
//! `runtime_core` only talks to a [`ValueStore`]; the native runtime uses
//! [`PointerStore`] (validated tagged real pointers retained for the store's
//! lifetime)
//! while the wasm simulator and host tests use [`HandleStore`]
//! (arena indices). Code addresses are carried as an opaque [`CodeHandle`]:
//! the native runtime interprets it as a function address, a simulator as a
//! label / instruction index.

#[cfg(not(target_family = "wasm"))]
use std::collections::HashMap;

use crate::runtime_core::{HeapObject, Value, HEAP_TAG, PTR_TAG_MASK};

/// Opaque code reference stored inside closures. Only the execution adapter
/// that created it may interpret it (function pointer vs simulated PC).
pub type CodeHandle = u64;

pub trait ValueStore {
    /// Moves `object` into the store and returns its tagged heap `Value`.
    fn alloc(&mut self, object: HeapObject) -> Value;
    /// Resolves a tagged heap value. `None` when `value` does not carry the
    /// heap tag or does not name a live object of this store.
    fn try_get(&self, value: Value) -> Option<&HeapObject>;
    fn try_get_mut(&mut self, value: Value) -> Option<&mut HeapObject>;
}

/// Arena-backed store: `((index << 3) | 0b001)`. Used by the wasm simulator
/// and by host-side tests; never hands out host pointers.
#[derive(Default)]
pub struct HandleStore {
    arena: Vec<HeapObject>,
}

impl HandleStore {
    pub fn new() -> HandleStore {
        HandleStore::default()
    }

    fn index_of(value: Value) -> Option<usize> {
        if value & PTR_TAG_MASK != HEAP_TAG {
            return None;
        }
        Some((value >> 3) as usize)
    }
}

impl ValueStore for HandleStore {
    fn alloc(&mut self, object: HeapObject) -> Value {
        self.arena.push(object);
        (((self.arena.len() - 1) as u64) << 3) | HEAP_TAG
    }

    fn try_get(&self, value: Value) -> Option<&HeapObject> {
        self.arena.get(Self::index_of(value)?)
    }

    fn try_get_mut(&mut self, value: Value) -> Option<&mut HeapObject> {
        let index = Self::index_of(value)?;
        self.arena.get_mut(index)
    }
}

/// Native store: heap objects live in owned, 8-byte-aligned cells and the
/// tagged value is the stable cell address with the low bits `001` (design
/// §5.2).
///
/// The address map is part of the safety boundary: an arbitrary tagged
/// integer is never dereferenced. A value resolves only when this store owns
/// the exact cell, and references remain tied to the corresponding store
/// borrow. The native runtime keeps one process-wide store behind a mutex so
/// values remain live across FFI entries without creating independent aliasing
/// tokens.
#[cfg(not(target_family = "wasm"))]
#[repr(align(8))]
struct HeapCell(HeapObject);

#[cfg(not(target_family = "wasm"))]
#[derive(Default)]
pub struct PointerStore {
    cells: HashMap<Value, Box<HeapCell>>,
}

#[cfg(not(target_family = "wasm"))]
impl PointerStore {
    pub fn new() -> PointerStore {
        PointerStore::default()
    }
}

#[cfg(not(target_family = "wasm"))]
impl ValueStore for PointerStore {
    fn alloc(&mut self, object: HeapObject) -> Value {
        let cell = Box::new(HeapCell(object));
        let address = cell.as_ref() as *const HeapCell as u64;
        debug_assert_eq!(address & PTR_TAG_MASK, 0, "heap cells must be 8-byte aligned");
        let value = address | HEAP_TAG;
        let replaced = self.cells.insert(value, cell);
        debug_assert!(replaced.is_none(), "live heap addresses must be unique");
        value
    }

    fn try_get(&self, value: Value) -> Option<&HeapObject> {
        if value & PTR_TAG_MASK != HEAP_TAG {
            return None;
        }
        self.cells.get(&value).map(|cell| &cell.0)
    }

    fn try_get_mut(&mut self, value: Value) -> Option<&mut HeapObject> {
        if value & PTR_TAG_MASK != HEAP_TAG {
            return None;
        }
        self.cells.get_mut(&value).map(|cell| &mut cell.0)
    }
}
