//! Test suite for the Web and headless browsers.

#![cfg(target_arch = "wasm32")]

extern crate wasm_bindgen_test;
use wasm_bindgen_test::*;
use monkey_wasm::parse;
use insta::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn pass() {
    let input = "let a = 3";
    let r = parse(input);
    assert_snapshot!("simple wasm test", r, input);
}
