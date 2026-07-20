//! Storage backends behind the runtime semantics (design §8.1).
//!
//! `runtime_core` only talks to a [`ValueStore`]; the native runtime uses
//! [`PointerStore`] (tagged real pointers, `Box::leak`, never freed in v1)
//! while the wasm simulator and host tests use [`HandleStore`]
//! (arena indices). Code addresses are carried as an opaque [`CodeHandle`]:
//! the native runtime interprets it as a function address, a simulator as a
//! label / instruction index.

use std::cell::UnsafeCell;

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
pub struct HandleStore {
    arena: Vec<HeapObject>,
}

impl HandleStore {
    pub fn new() -> HandleStore {
        HandleStore {
            arena: vec![],
        }
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

/// Native store: heap objects live in leaked, 8-byte-aligned cells and the
/// tagged value is the cell address with the low bits `001` (design §5.2).
///
/// Safety model: generated code and the runtime run single-threaded, every
/// FFI entry creates a fresh `PointerStore` value, and all accesses flow
/// through that one store borrow, so Rust's borrow rules on the store keep
/// `&HeapObject` / `&mut HeapObject` from aliasing. Values are only decoded
/// if they carry the heap tag; forging a tagged pointer is an ABI violation.
#[cfg(not(target_family = "wasm"))]
#[repr(align(8))]
struct HeapCell(UnsafeCell<HeapObject>);

#[cfg(not(target_family = "wasm"))]
pub struct PointerStore;

#[cfg(not(target_family = "wasm"))]
impl PointerStore {
    fn cell(value: Value) -> Option<*mut HeapObject> {
        if value & PTR_TAG_MASK != HEAP_TAG {
            return None;
        }
        let cell = (value - HEAP_TAG) as *const HeapCell;
        Some(unsafe { (*cell).0.get() })
    }
}

#[cfg(not(target_family = "wasm"))]
impl ValueStore for PointerStore {
    fn alloc(&mut self, object: HeapObject) -> Value {
        let cell: &'static mut HeapCell = Box::leak(Box::new(HeapCell(UnsafeCell::new(object))));
        let address = cell as *mut HeapCell as u64;
        debug_assert_eq!(address & PTR_TAG_MASK, 0, "heap cells must be 8-byte aligned");
        address | HEAP_TAG
    }

    fn try_get(&self, value: Value) -> Option<&HeapObject> {
        Self::cell(value).map(|pointer| unsafe { &*pointer })
    }

    fn try_get_mut(&mut self, value: Value) -> Option<&mut HeapObject> {
        Self::cell(value).map(|pointer| unsafe { &mut *pointer })
    }
}
