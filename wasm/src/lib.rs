mod utils;

use crate::utils::set_panic_hook;
use compiler::compiler::Compiler;
use parser::parse as parser_pase;
use parser::parse_ast_json_string;
use wasm_bindgen::prelude::*;
use wasm_bindgen::throw_str;

const PLAYGROUND_GC_INSTRUCTION_BUDGET: usize = 10_000;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn parse(input: &str) -> String {
    set_panic_hook();
    match parse_ast_json_string(input) {
        Ok(node) => node.to_string(),
        Err(e) => throw_str(format!("parse error: {}", e[0]).as_str()),
    }
}

#[wasm_bindgen]
pub fn compile(input: &str) -> String {
    set_panic_hook();

    let program = match parser_pase(input) {
        Ok(ast) => ast,
        Err(e) => throw_str(format!("parse error: {}", e[0]).as_str()),
    };
    let mut compiler = Compiler::new();
    match compiler.compile(&program) {
        Ok(bytecode) => return bytecode.instructions.string(),
        Err(e) => throw_str(format!("compile error: {}", e).as_str()),
    }
}

#[wasm_bindgen]
pub fn compile_detail(input: &str) -> String {
    set_panic_hook();

    let program = match parser_pase(input) {
        Ok(ast) => ast,
        Err(e) => throw_str(format!("parse error: {}", e[0]).as_str()),
    };
    let mut compiler = Compiler::new();
    match compiler.compile(&program) {
        Ok(bytecode) => return bytecode.string(),
        Err(e) => throw_str(format!("compile error: {}", e).as_str()),
    }
}

#[wasm_bindgen]
pub fn compile_with_debug(input: &str) -> String {
    set_panic_hook();

    let program = match parser_pase(input) {
        Ok(ast) => ast,
        Err(e) => throw_str(format!("parse error: {}", e[0]).as_str()),
    };
    let mut compiler = Compiler::new();
    match compiler.compile(&program) {
        Ok(bytecode) => match serde_json::to_string(&bytecode.debug_view()) {
            Ok(json) => json,
            Err(e) => throw_str(format!("json error: {}", e).as_str()),
        },
        Err(e) => throw_str(format!("compile error: {}", e).as_str()),
    }
}

/// Execute Monkey source on the cycle-collecting VM and return a tagged JSON envelope.
///
/// User parse, compile, runtime, and execution-limit failures are data in the envelope,
/// not JavaScript exceptions. This keeps the playground's Run GC path deterministic.
#[wasm_bindgen]
pub fn run_gc_with_report(input: &str) -> String {
    set_panic_hook();

    let envelope = match gc::run_source_with_report(input, PLAYGROUND_GC_INSTRUCTION_BUDGET) {
        Ok(success) => serde_json::json!({
            "status": "ok",
            "result": success.result,
            "report": success.report,
        }),
        Err(error) => serde_json::json!({
            "status": "error",
            "stage": error.stage,
            "message": error.message,
            "span": error.span,
        }),
    };

    serde_json::to_string(&envelope).expect("GC run envelope serialization should not fail")
}
