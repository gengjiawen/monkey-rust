//! Shared execution path for the `monkey-gc` CLI (design doc §7).
//!
//! Both CLI inputs — `.monkey` source and `.mbc` snapshots — funnel into
//! [`run_bytecode`] once a [`Bytecode`] is in hand, so results and runtime
//! errors render identically on both paths. This deliberately bypasses
//! `gc::eval_source`: exporting the final value back to an [`object::Object`]
//! fails for class instances and drops the runtime-error `Span`.

use compiler::compiler::{Bytecode, Compiler};

use crate::vm::{GcRuntimeError, GcVM};

/// Parse and compile Monkey source, reporting the first error as a string.
pub fn compile_source(source: &str) -> Result<Bytecode, String> {
    let program = parser::parse(source).map_err(|errors| {
        errors
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown parse error".to_string())
    })?;
    let mut compiler = Compiler::new();
    compiler.compile(&program)
}

/// Execute bytecode on a fresh VM and render the final popped value the way
/// the REPL does. `instruction_budget` is `usize::MAX` for normal runs; the
/// CLI's `--max-instructions` threads a finite budget through here.
pub fn run_bytecode(
    bytecode: Bytecode,
    instruction_budget: usize,
) -> Result<String, GcRuntimeError> {
    let mut vm = GcVM::new(bytecode);
    vm.run_with_budget(instruction_budget)?;
    Ok(vm.last_result_string())
}

/// Execute bytecode on a fresh VM while capturing all `puts`/`print` output.
/// The output is returned even when execution fails after producing it.
pub fn run_bytecode_with_output(
    bytecode: Bytecode,
    instruction_budget: usize,
) -> (Result<String, GcRuntimeError>, String) {
    let mut vm = GcVM::new(bytecode);
    vm.set_capture_output(true);
    let result = vm
        .run_with_budget(instruction_budget)
        .map(|()| vm.last_result_string());
    let output = vm.take_output();
    (result, output)
}
