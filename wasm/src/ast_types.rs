//! Ships the hand-written TypeScript declarations for the JSON AST that
//! `parse` and `analyze_lossless` emit. wasm-bindgen appends the file to the
//! generated `monkey_wasm.d.ts`, so the npm package carrying the parser also
//! carries the types for its output.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const AST_TYPES: &str = include_str!("ast_types.d.ts");
