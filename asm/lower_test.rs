//! Lowering snapshot tests (design §10.3): whole `.s` modules for
//! representative programs, plus the lowering-time error cases.

use crate::lower::compile_source;

fn assembly(source: &str) -> String {
    compile_source(source, false)
        .expect("program should lower")
        .text
}

fn error_message(source: &str) -> String {
    compile_source(source, false).expect_err("program should be rejected")
}

#[test]
fn snapshot_integer_arithmetic() {
    insta::assert_snapshot!(assembly("1 + 2 * 3;"));
}

#[test]
fn snapshot_global_rebinding() {
    // Each `let x` gets a fresh global slot; the second reads the first.
    insta::assert_snapshot!(assembly("let x = 1; let x = x + 2; x;"));
}

#[test]
fn snapshot_if_else_and_comparison() {
    insta::assert_snapshot!(assembly("if (1 < 2) { 10 } else { 20 };"));
}

#[test]
fn snapshot_recursion() {
    insta::assert_snapshot!(assembly(
        "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } };\nfib(10);"
    ));
}

#[test]
fn snapshot_named_function_self_reference() {
    // `f` inside the body resolves to the Function scope (the spilled
    // closure slot), not a global read.
    insta::assert_snapshot!(assembly(
        "let f = fn(n) { if (n > 0) { f(n - 1) } else { 0 } };\nf(3);"
    ));
}

#[test]
fn snapshot_closure_capture() {
    insta::assert_snapshot!(assembly("let adder = fn(x) { fn(y) { x + y } };\nadder(1)(2);"));
}

#[test]
fn snapshot_builtins_first_class() {
    insta::assert_snapshot!(assembly("let p = puts; p(len(\"abc\"));"));
}

#[test]
fn snapshot_aggregates_and_index() {
    insta::assert_snapshot!(assembly("let a = [1, 2];\nlet h = {\"k\": a[0]};\nh[\"k\"];"));
}

#[test]
fn snapshot_classes() {
    insta::assert_snapshot!(assembly(
        "class Counter {\n  constructor(start) { this.count = start; }\n  inc() { this.count = this.count + 1; this.count }\n}\nlet c = new Counter(5);\nc.inc();\nc.count;"
    ));
}

#[test]
fn snapshot_return_paths() {
    insta::assert_snapshot!(assembly("let f = fn() { return 1; 2; };\nf();"));
}

#[test]
fn snapshot_boxed_integer_literal() {
    // i64::MAX exceeds the SMI range: materialize + rt_box_int.
    insta::assert_snapshot!(assembly("9223372036854775807;"));
}

#[test]
fn snapshot_observe_mode() {
    insta::assert_snapshot!(compile_source("1 + 2;", true).unwrap().text);
}

#[test]
fn line_spans_point_into_the_source() {
    let source = "let x = 41;\nx + 1;";
    let assembly = compile_source(source, false).unwrap();
    assert_eq!(assembly.text.lines().count(), assembly.line_spans.len());
    let mut spanned = 0;
    for span in assembly.line_spans.iter().flatten() {
        let (start, end) = *span;
        assert!(start < end && end <= source.len(), "span out of range: {:?}", span);
        spanned += 1;
    }
    assert!(spanned > 0, "expected some lines to carry source spans");
    // The rt_add fallback line maps to the infix expression `x + 1`.
    let lines: Vec<&str> = assembly.text.lines().collect();
    let add_line = lines
        .iter()
        .position(|line| line.contains("bl rt_add"))
        .unwrap();
    let (start, end) = assembly.line_spans[add_line].unwrap();
    assert_eq!(&source[start..end], "x + 1");
}

#[test]
fn parameter_limits_are_rejected() {
    assert_eq!(
        error_message("fn(a, b, c, d, e, f, g, h) { 0 };"),
        "functions accept at most 7 parameters"
    );
    assert_eq!(
        error_message("class C { m(a, b, c, d, e, f, g) { 0 } }"),
        "methods accept at most 6 parameters"
    );
    // Seven function parameters / six method parameters are fine.
    assert!(compile_source("fn(a, b, c, d, e, f, g) { 0 };", false).is_ok());
    assert!(compile_source("class C { m(a, b, c, d, e, f) { 0 } }", false).is_ok());
}

#[test]
fn validation_runs_before_lowering() {
    assert!(error_message("missing;").contains("undefined variable 'missing'"));
    assert!(error_message("this;").contains("this is only available inside a method"));
    assert!(error_message("class C { constructor() { return 1; } }")
        .contains("constructor cannot return a value"));
}

#[test]
fn builtins_do_not_occupy_global_slots() {
    // A program using only builtins allocates zero global slots.
    let text = assembly("puts(1);");
    assert!(text.contains(".skip 0"));
    // And `puts` is a tagged immediate, not a load from g_globals.
    assert!(text.contains("movz x0, #0xd"));
}
