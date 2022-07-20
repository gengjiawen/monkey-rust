mod utils;

use crate::utils::set_panic_hook;
use parser::parse as parser_pase;
use parser::parse_ast_json_string;
use wasm_bindgen::prelude::*;
use wasm_bindgen::throw_str;
use compiler::compiler::Compiler;
use parser::ast::Node;

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
    match  compiler.compile(&program) {
        Ok(bytecode) => {
            return bytecode.instructions.string()
        },
        Err(e) => throw_str(format!("compile error: {}", e).as_str()),
    }
}
