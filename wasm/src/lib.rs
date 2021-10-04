mod utils;

use wasm_bindgen::prelude::*;
use parser::{parse_ast_json_string};
use wasm_bindgen::throw_str;
use crate::utils::set_panic_hook;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub fn parse(input: &str) -> String {
    set_panic_hook();
    match parse_ast_json_string(input) {
        Ok(node) => {
            node.to_string()
        },
        Err(e) => throw_str(format!("parse error: {}", e[0]).as_str())
    }
}
