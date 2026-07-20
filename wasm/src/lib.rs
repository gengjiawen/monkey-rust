mod utils;

use crate::utils::set_panic_hook;
use compiler::compiler::Compiler;
use compiler::snapshot::{read_bytecode, write_bytecode};
use compiler::snapshot_layout::describe_bytecode;
use monkey_asm::lower::lower_node;
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

/// Compile Monkey source to AArch64 assembly and return a tagged JSON envelope
/// of per-line `text`/`kind`/`span` records for the playground's godbolt-style
/// ARM64 view (arm64 backend design §12 V1).
///
/// The browser only renders the text `monkey-asm emit` would produce — nothing
/// executes arm64 here. Parse and lowering failures are data in the envelope,
/// not JavaScript exceptions, mirroring [`run_gc_with_report`].
#[wasm_bindgen]
pub fn compile_to_arm64(input: &str) -> String {
    set_panic_hook();

    let envelope = match arm64_envelope(input) {
        Ok(envelope) => envelope,
        Err((stage, message, span)) => serde_json::json!({
            "status": "error",
            "stage": stage,
            "message": message,
            "span": span.map(|(start, end)| serde_json::json!({ "start": start, "end": end })),
        }),
    };
    serde_json::to_string(&envelope).expect("arm64 envelope serialization should not fail")
}

type Arm64Failure = (&'static str, String, Option<(usize, usize)>);

fn arm64_envelope(input: &str) -> Result<serde_json::Value, Arm64Failure> {
    let node = parser_pase(input).map_err(|errors| {
        let message = errors
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown parse error".to_string());
        ("parse", message, None)
    })?;
    let assembly =
        lower_node(input, &node, false).map_err(|error| ("compile", error.message, error.span))?;

    // `Assembly` guarantees one `line_spans` entry per `\n`-terminated line.
    let lines: Vec<serde_json::Value> = assembly
        .text
        .lines()
        .zip(assembly.line_spans.iter())
        .map(|(text, span)| {
            serde_json::json!({
                "text": text,
                "kind": arm64_line_kind(text),
                "span": span.map(|(start, end)| serde_json::json!({ "start": start, "end": end })),
            })
        })
        .collect();
    Ok(serde_json::json!({ "status": "ok", "lines": lines }))
}

/// Presentation-level line class for the playground: the emitter only ever
/// writes `//` comments, so everything before the first `//` is the code part.
fn arm64_line_kind(text: &str) -> &'static str {
    let code = match text.find("//") {
        Some(index) => &text[..index],
        None => text,
    };
    let trimmed = code.trim();
    if trimmed.is_empty() {
        if text.trim().is_empty() {
            "blank"
        } else {
            "comment"
        }
    } else if trimmed.ends_with(':') {
        "label"
    } else if trimmed.starts_with('.') {
        "directive"
    } else {
        "code"
    }
}

/// Compile Monkey source into a `.mbc` snapshot and return a tagged JSON envelope
/// with the raw bytes (lowercase hex) plus a byte-range annotation of the container
/// layout for the playground inspector.
///
/// User parse and compile failures are data in the envelope, not JavaScript
/// exceptions, mirroring [`run_gc_with_report`].
#[wasm_bindgen]
pub fn compile_to_snapshot(input: &str, strip_debug: bool) -> String {
    set_panic_hook();

    let envelope = match snapshot_envelope(input, strip_debug) {
        Ok(envelope) => envelope,
        Err((stage, message)) => serde_json::json!({
            "status": "error",
            "stage": stage,
            "message": message,
        }),
    };
    serde_json::to_string(&envelope).expect("snapshot envelope serialization should not fail")
}

fn snapshot_envelope(
    input: &str,
    strip_debug: bool,
) -> Result<serde_json::Value, (&'static str, String)> {
    let program = parser_pase(input).map_err(|errors| {
        let message = errors
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown parse error".to_string());
        ("parse", message)
    })?;
    let mut compiler = Compiler::new();
    let bytecode = compiler
        .compile(&program)
        .map_err(|message| ("compile", message))?;
    let bytes = write_bytecode(&bytecode, strip_debug)
        .map_err(|error| ("snapshot", format!("{:?}", error)))?;
    let layout = describe_bytecode(&bytes).map_err(|error| ("snapshot", format!("{:?}", error)))?;
    Ok(serde_json::json!({
        "status": "ok",
        "bytesHex": hex_encode(&bytes),
        "layout": layout,
    }))
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{:02x}", byte).expect("writing to a String cannot fail");
    }
    out
}

/// Execute `.mbc` snapshot bytes on the cycle-collecting VM — the browser twin of
/// `monkey-gc run foo.mbc`, sharing its execution path (`gc::run_bytecode`).
///
/// The buffer is untrusted input: it goes through the validating snapshot reader
/// before the VM. Failures are data in the envelope — stage `snapshot` when the
/// bytes are rejected, `runtime` when the VM errors (the span is only present
/// when the snapshot kept its debug info).
#[wasm_bindgen]
pub fn run_snapshot(bytes: &[u8]) -> String {
    set_panic_hook();

    let envelope = match read_bytecode(bytes) {
        Ok(bytecode) => match gc::run_bytecode(bytecode, PLAYGROUND_GC_INSTRUCTION_BUDGET) {
            Ok(result) => serde_json::json!({
                "status": "ok",
                "result": result,
            }),
            Err(error) => serde_json::json!({
                "status": "error",
                "stage": "runtime",
                "message": error.message,
                "span": error.span,
            }),
        },
        Err(error) => serde_json::json!({
            "status": "error",
            "stage": "snapshot",
            "message": format!("{:?}", error),
            "span": null,
        }),
    };
    serde_json::to_string(&envelope).expect("snapshot run envelope serialization should not fail")
}
