#[cfg(test)]
mod gc_test;
#[cfg(test)]
mod value_test;
#[cfg(test)]
mod vm_test;

pub mod frame;
pub mod header;
pub mod heap;
pub mod list;
pub mod malloc;
pub mod runtime;
pub mod value;
pub mod vm;

pub use frame::Frame;
pub use heap::{GcHeap, GcRef};
pub use header::{
    GcId,
    GcObjectHeader,
    GcObjectType,
    GcPhase,
    RefCountHeader,
    RefCountId,
};
pub use malloc::{DEFAULT_GC_THRESHOLD, MALLOC_OVERHEAD, MallocState};
pub use runtime::{GcObject, GcRuntime, MarkFunc};
pub use value::{export_object, import_object, GcClosure, Value};
pub use vm::GcVM;

use compiler::compiler::{Bytecode, Compiler};
use object::Object;
use parser::ast::Node;

/// Compile Monkey source using the existing bytecode compiler.
pub fn compile(program: &Node) -> Result<Bytecode, String> {
    let mut compiler = Compiler::new();
    compiler.compile(program)
}

/// Compile and execute on the GC-backed VM.
pub fn eval(program: &Node) -> Result<Object, String> {
    let bytecode = compile(program)?;
    let mut vm = GcVM::new(bytecode);
    vm.run();
    vm.export_last_result().ok_or_else(|| "no result on stack".to_string())
}

/// Parse, compile, and execute Monkey source.
pub fn eval_source(source: &str) -> Result<Object, String> {
    let program = parser::parse(source).map_err(|errors| errors[0].clone())?;
    eval(&program)
}
