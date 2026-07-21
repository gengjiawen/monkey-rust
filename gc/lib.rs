#[cfg(test)]
mod gc_test;
#[cfg(test)]
mod report_test;
#[cfg(test)]
mod runner_test;
#[cfg(test)]
mod value_test;
#[cfg(test)]
mod vm_test;

pub mod frame;
pub mod header;
pub mod heap;
pub mod list;
pub mod malloc;
pub mod report;
pub mod runner;
// The file is named gc_runtime.rs for editor clarity, but the module keeps
// its historical public path `gc::runtime`.
#[path = "gc_runtime.rs"]
pub mod runtime;
pub mod value;
pub mod vm;

pub use frame::Frame;
pub use header::{GcId, GcObjectHeader, GcObjectType, GcPhase, RefCountHeader, RefCountId};
pub use heap::{GcHeap, GcRef};
pub use malloc::{MallocState, DEFAULT_GC_THRESHOLD, MALLOC_OVERHEAD};
pub use report::{
    EdgeRelation, FinalFate, FreeCycleStats, GcCollectionReport, GcObjectSummary, GcPhaseStats,
    GcStatsBundle, GlobalRoot, HashKeyKind, HeapSnapshot, ObjectDecision, RestorationWitness,
    ScanStats, TrialDecision, TrialDeletionStats, ValueKindCounts, VisitedEdge,
};
pub use runner::{compile_source, run_bytecode, run_bytecode_with_output};
pub use runtime::{GcObject, GcRuntime, MarkFunc};
pub use value::{
    export_object, import_object, try_export_object, value_to_string, GcClosure, Value, ValueKind,
};
pub use vm::{
    GcClassifiedRuntimeError, GcRuntimeError, GcRuntimeErrorKind, GcVM, DEFAULT_INSTRUCTION_BUDGET,
};

use compiler::compiler::{Bytecode, Compiler};
use object::Object;
use parser::ast::Node;
use parser::lexer::token::Span;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GcRunStage {
    Parse,
    Compile,
    Runtime,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcRunSuccess {
    pub result: String,
    pub report: GcCollectionReport,
}

/// Parse, compile, or runtime failure returned by the established report API.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcRunError {
    pub stage: GcRunStage,
    pub message: String,
    pub span: Option<Span>,
}

/// Report failure with a stable, machine-readable error category.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GcClassifiedRunError {
    pub stage: GcRunStage,
    pub kind: String,
    pub message: String,
    pub span: Option<Span>,
}

impl From<GcClassifiedRunError> for GcRunError {
    fn from(error: GcClassifiedRunError) -> Self {
        Self {
            stage: error.stage,
            message: error.message,
            span: error.span,
        }
    }
}

/// Compile Monkey source using the existing bytecode compiler.
pub fn compile(program: &Node) -> Result<Bytecode, String> {
    let mut compiler = Compiler::new();
    compiler.compile(program)
}

/// Compile and execute on the GC-backed VM.
pub fn eval(program: &Node) -> Result<Object, String> {
    let bytecode = compile(program)?;
    let mut vm = GcVM::new(bytecode);
    vm.run_with_budget(usize::MAX)
        .map_err(|error| error.message)?;
    vm.try_export_last_result()
}

/// Parse, compile, and execute Monkey source.
pub fn eval_source(source: &str) -> Result<Object, String> {
    let program = parser::parse(source).map_err(|errors| errors[0].clone())?;
    eval(&program)
}

/// Parse, compile, execute with deterministic GC settings, then collect cycles.
pub fn run_source_with_report(
    source: &str,
    instruction_budget: usize,
) -> Result<GcRunSuccess, GcRunError> {
    run_source_with_report_classified(source, instruction_budget).map_err(Into::into)
}

/// Parse, compile, and execute Monkey source while classifying failures at
/// their raise sites.
pub fn run_source_with_report_classified(
    source: &str,
    instruction_budget: usize,
) -> Result<GcRunSuccess, GcClassifiedRunError> {
    let program = parser::parse(source).map_err(|errors| GcClassifiedRunError {
        stage: GcRunStage::Parse,
        kind: "syntax".to_string(),
        message: errors
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown parse error".to_string()),
        span: None,
    })?;
    let mut compiler = Compiler::new();
    let bytecode = compiler
        .compile(&program)
        .map_err(|message| GcClassifiedRunError {
            stage: GcRunStage::Compile,
            kind: "compile".to_string(),
            message,
            span: None,
        })?;
    let global_names = compiler.symbol_table.global_symbols();
    let mut vm = GcVM::new(bytecode);
    vm.set_global_names(global_names);
    vm.heap_mut().set_gc_threshold(usize::MAX);
    vm.run_with_budget_classified(instruction_budget)
        .map_err(|error| GcClassifiedRunError {
            stage: GcRunStage::Runtime,
            kind: error.kind.as_str().to_string(),
            message: error.message,
            span: error.span,
        })?;
    let result = vm.last_result_string();
    let report = vm.collect_garbage();
    Ok(GcRunSuccess {
        result,
        report,
    })
}
