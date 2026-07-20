//! Native `extern "C"` runtime the generated `.s` links against (design §8).
//!
//! Every entry point is a thin shell: decode the raw FFI arguments, run the
//! shared semantics from [`crate::runtime_core`] on a [`PointerStore`], and
//! turn any [`RuntimeFailure`] into `rt_fatal` (observer error record when
//! initialized, stderr message, `exit(1)`). Panics never cross the FFI
//! boundary: shells run under `catch_unwind` and report `InternalError`.
//!
//! There is no global runtime object. `PointerStore` is stateless (heap
//! objects live in leaked cells addressed by the tagged values themselves),
//! so each shell builds a fresh store; the only process-level state is the
//! observer fd and the registered globals area, both atomics.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use crate::runtime_backend::{CodeHandle, PointerStore};
use crate::runtime_core::{
    self, CallDispatch, OutputSink, ReturnPolicy, RuntimeErrorKind, RuntimeFailure, RuntimeResult,
    Value, NULL_VALUE,
};

/// Observer channel fd registered by `rt_observer_init`; -1 = not installed.
static OBSERVER_FD: AtomicI64 = AtomicI64::new(-1);

/// Globals area registered by `rt_globals_init` (future GC root scanning).
static GLOBALS_BASE: AtomicU64 = AtomicU64::new(0);
static GLOBALS_COUNT: AtomicU64 = AtomicU64::new(0);

struct StdoutSink;

impl OutputSink for StdoutSink {
    fn write_line(&mut self, line: &str) {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        // stdout must stay the exact byte stream of puts/print (design §10.2);
        // flush per line so exit(1)/exit paths cannot drop buffered bytes.
        let _ = handle.write_all(line.as_bytes());
        let _ = handle.write_all(b"\n");
        let _ = handle.flush();
    }
}

/// Writes the single framed observer record: u64 big-endian payload length,
/// then the UTF-8 JSON payload (design §10.2).
#[cfg(unix)]
fn observer_write(payload: &str) {
    use std::io::Write;
    use std::os::unix::io::{FromRawFd, IntoRawFd};

    let fd = OBSERVER_FD.load(Ordering::SeqCst);
    if fd < 0 {
        return;
    }
    let mut file = unsafe { std::fs::File::from_raw_fd(fd as i32) };
    let length = (payload.len() as u64).to_be_bytes();
    let _ = file.write_all(&length);
    let _ = file.write_all(payload.as_bytes());
    let _ = file.flush();
    // The harness owns the fd; keep it open.
    let _ = file.into_raw_fd();
}

#[cfg(not(unix))]
fn observer_write(_payload: &str) {}

/// Terminal error path shared by all shells (design §8): optional observer
/// error record, human-readable stderr line, `exit(1)`.
fn fatal(kind: RuntimeErrorKind, message: &str) -> ! {
    observer_write(&format!("{{\"status\":\"error\",\"kind\":\"{}\"}}", kind.name()));
    eprintln!("monkey: {}: {}", kind.name(), message);
    std::process::exit(1);
}

/// Runs one FFI shell body: panics become `InternalError`, `RuntimeFailure`
/// becomes `rt_fatal` semantics. Only `Ok` values return to generated code.
fn ffi_shell<T>(body: impl FnOnce(&mut PointerStore) -> RuntimeResult<T>) -> T {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut store = PointerStore;
        body(&mut store)
    }));
    match outcome {
        Ok(Ok(value)) => value,
        Ok(Err(RuntimeFailure {
            kind,
            message,
        })) => fatal(kind, &message),
        Err(_) => fatal(RuntimeErrorKind::InternalError, "runtime panicked"),
    }
}

/// `(ptr, len)` decoding per §8: zero length may be null and is an empty
/// slice; non-zero length must be a valid, aligned region.
unsafe fn value_slice<'a>(ptr: *const Value, len: u64) -> &'a [Value] {
    if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, len as usize)
    }
}

unsafe fn byte_slice<'a>(ptr: *const u8, len: u64) -> &'a [u8] {
    if len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(ptr, len as usize)
    }
}

fn name_from_bytes(bytes: &[u8]) -> RuntimeResult<&str> {
    std::str::from_utf8(bytes).map_err(|_| RuntimeFailure {
        kind: RuntimeErrorKind::InternalError,
        message: "identifier bytes are not valid UTF-8".to_string(),
    })
}

/// Calls generated code for a `CallDispatch::Invoke` (design §8.1): pick the
/// `extern "C"` signature by arity, closure in `x0`, arguments in `x1..x7`.
unsafe fn invoke_code(code: CodeHandle, closure: Value, args: &[Value]) -> Value {
    use std::mem::transmute;
    let address = code as usize;
    match args {
        [] => transmute::<usize, extern "C" fn(Value) -> Value>(address)(closure),
        [a1] => transmute::<usize, extern "C" fn(Value, Value) -> Value>(address)(closure, *a1),
        [a1, a2] => transmute::<usize, extern "C" fn(Value, Value, Value) -> Value>(address)(
            closure, *a1, *a2,
        ),
        [a1, a2, a3] => transmute::<usize, extern "C" fn(Value, Value, Value, Value) -> Value>(
            address,
        )(closure, *a1, *a2, *a3),
        [a1, a2, a3, a4] => transmute::<
            usize,
            extern "C" fn(Value, Value, Value, Value, Value) -> Value,
        >(address)(closure, *a1, *a2, *a3, *a4),
        [a1, a2, a3, a4, a5] => transmute::<
            usize,
            extern "C" fn(Value, Value, Value, Value, Value, Value) -> Value,
        >(address)(closure, *a1, *a2, *a3, *a4, *a5),
        [a1, a2, a3, a4, a5, a6] => transmute::<
            usize,
            extern "C" fn(Value, Value, Value, Value, Value, Value, Value) -> Value,
        >(address)(closure, *a1, *a2, *a3, *a4, *a5, *a6),
        [a1, a2, a3, a4, a5, a6, a7] => {
            transmute::<
                usize,
                extern "C" fn(Value, Value, Value, Value, Value, Value, Value, Value) -> Value,
            >(address)(closure, *a1, *a2, *a3, *a4, *a5, *a6, *a7)
        }
        _ => fatal(
            RuntimeErrorKind::ResourceLimit,
            "call requires more arguments than the calling convention allows",
        ),
    }
}

/// Finishes a dispatch outside any store borrow: `Invoke` re-enters generated
/// code, which will recursively call back into these shells.
fn complete_dispatch(dispatch: CallDispatch) -> Value {
    match dispatch {
        CallDispatch::Return(value) => value,
        CallDispatch::Invoke {
            code,
            closure,
            args,
            return_policy,
        } => {
            let returned = unsafe { invoke_code(code, closure, &args) };
            match return_policy {
                ReturnPolicy::Direct => returned,
                ReturnPolicy::ConstructorInstance(instance) => instance,
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn rt_globals_init(base: *mut Value, count: u64) {
    ffi_shell(|_store| {
        for index in 0..count as usize {
            unsafe {
                *base.add(index) = NULL_VALUE;
            }
        }
        GLOBALS_BASE.store(base as u64, Ordering::SeqCst);
        GLOBALS_COUNT.store(count, Ordering::SeqCst);
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn rt_string_from_bytes(ptr: *const u8, len: u64) -> Value {
    ffi_shell(|store| {
        let bytes = unsafe { byte_slice(ptr, len) };
        runtime_core::string_from_utf8(store, bytes)
    })
}

#[no_mangle]
pub extern "C" fn rt_box_int(raw: i64) -> Value {
    ffi_shell(|store| Ok(runtime_core::make_int(store, raw)))
}

#[no_mangle]
pub extern "C" fn rt_array(argv: *const Value, len: u64) -> Value {
    ffi_shell(|store| {
        let values = unsafe { value_slice(argv, len) };
        Ok(runtime_core::array_from_values(store, values))
    })
}

#[no_mangle]
pub extern "C" fn rt_hash(argv: *const Value, pairs: u64) -> Value {
    ffi_shell(|store| {
        let values = unsafe { value_slice(argv, pairs * 2) };
        runtime_core::hash_from_pairs(store, values)
    })
}

#[no_mangle]
pub extern "C" fn rt_closure(
    code: *const u8,
    num_parameters: u64,
    free: *const Value,
    num_free: u64,
) -> Value {
    ffi_shell(|store| {
        let free_values = unsafe { value_slice(free, num_free) };
        runtime_core::closure_new(store, code as CodeHandle, num_parameters, free_values)
    })
}

#[no_mangle]
pub extern "C" fn rt_get_free(closure: Value, index: u64) -> Value {
    ffi_shell(|store| runtime_core::get_free(store, closure, index))
}

#[no_mangle]
pub extern "C" fn rt_class(name: *const u8, len: u64) -> Value {
    ffi_shell(|store| {
        let bytes = unsafe { byte_slice(name, len) };
        let class_name = name_from_bytes(bytes)?;
        Ok(runtime_core::class_new(store, class_name))
    })
}

#[no_mangle]
pub extern "C" fn rt_class_add_method(
    class: Value,
    name: *const u8,
    len: u64,
    method: Value,
    is_ctor: u64,
) {
    ffi_shell(|store| {
        let bytes = unsafe { byte_slice(name, len) };
        let method_name = name_from_bytes(bytes)?;
        runtime_core::class_add_method(store, class, method_name, method, is_ctor != 0)
    })
}

#[no_mangle]
pub extern "C" fn rt_get_property(obj: Value, name: *const u8, len: u64) -> Value {
    ffi_shell(|store| {
        let bytes = unsafe { byte_slice(name, len) };
        let property = name_from_bytes(bytes)?;
        runtime_core::get_property(store, obj, property)
    })
}

#[no_mangle]
pub extern "C" fn rt_set_property(obj: Value, name: *const u8, len: u64, v: Value) {
    ffi_shell(|store| {
        let bytes = unsafe { byte_slice(name, len) };
        let property = name_from_bytes(bytes)?;
        runtime_core::set_property(store, obj, property, v)
    })
}

#[no_mangle]
pub extern "C" fn rt_index(obj: Value, idx: Value) -> Value {
    ffi_shell(|store| runtime_core::index(store, obj, idx))
}

#[no_mangle]
pub extern "C" fn rt_add(l: Value, r: Value) -> Value {
    ffi_shell(|store| runtime_core::add(store, l, r))
}

#[no_mangle]
pub extern "C" fn rt_sub(l: Value, r: Value) -> Value {
    ffi_shell(|store| runtime_core::sub(store, l, r))
}

#[no_mangle]
pub extern "C" fn rt_mul(l: Value, r: Value) -> Value {
    ffi_shell(|store| runtime_core::mul(store, l, r))
}

#[no_mangle]
pub extern "C" fn rt_div(l: Value, r: Value) -> Value {
    ffi_shell(|store| runtime_core::div(store, l, r))
}

#[no_mangle]
pub extern "C" fn rt_eq(l: Value, r: Value) -> Value {
    ffi_shell(|store| runtime_core::eq_values(store, l, r).map(runtime_core::bool_value))
}

#[no_mangle]
pub extern "C" fn rt_neq(l: Value, r: Value) -> Value {
    ffi_shell(|store| {
        runtime_core::eq_values(store, l, r).map(|equal| runtime_core::bool_value(!equal))
    })
}

#[no_mangle]
pub extern "C" fn rt_gt(l: Value, r: Value) -> Value {
    ffi_shell(|store| runtime_core::gt(store, l, r))
}

#[no_mangle]
pub extern "C" fn rt_minus(v: Value) -> Value {
    ffi_shell(|store| runtime_core::minus(store, v))
}

#[no_mangle]
pub extern "C" fn rt_bang(v: Value) -> Value {
    ffi_shell(|_store| Ok(runtime_core::bang(v)))
}

#[no_mangle]
pub extern "C" fn rt_truthy(v: Value) -> u64 {
    ffi_shell(|_store| Ok(if runtime_core::truthy(v) { 1 } else { 0 }))
}

#[no_mangle]
pub extern "C" fn rt_call(callee: Value, argc: u64, argv: *const Value) -> Value {
    let dispatch = ffi_shell(|store| {
        let args = unsafe { value_slice(argv, argc) };
        runtime_core::dispatch_call(store, &mut StdoutSink, callee, args)
    });
    complete_dispatch(dispatch)
}

#[no_mangle]
pub extern "C" fn rt_construct(callee: Value, argc: u64, argv: *const Value) -> Value {
    let dispatch = ffi_shell(|store| {
        let args = unsafe { value_slice(argv, argc) };
        runtime_core::dispatch_construct(store, callee, args)
    });
    complete_dispatch(dispatch)
}

#[no_mangle]
pub extern "C" fn rt_observer_init(fd: u64) {
    OBSERVER_FD.store(fd as i64, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn rt_observe_result(v: Value) {
    let payload = ffi_shell(|store| {
        let value = runtime_core::canonical_value(store, v)?;
        Ok(format!("{{\"status\":\"ok\",\"value\":{}}}", value))
    });
    observer_write(&payload);
}

#[no_mangle]
pub extern "C" fn rt_fatal(kind: u64, msg: *const u8, len: u64) -> ! {
    let outcome = catch_unwind(|| {
        let kind = RuntimeErrorKind::from_u64(kind).unwrap_or(RuntimeErrorKind::InternalError);
        let bytes = unsafe { byte_slice(msg, len) };
        (kind, String::from_utf8_lossy(bytes).into_owned())
    });
    match outcome {
        Ok((kind, message)) => fatal(kind, &message),
        Err(_) => fatal(RuntimeErrorKind::InternalError, "runtime panicked"),
    }
}
