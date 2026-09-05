//! Lowering snapshot tests (design §10.3): whole `.s` modules for
//! representative programs, plus the lowering-time error cases.

use crate::emitter::AsmDialect;
use crate::lower::compile_source;

fn assembly(source: &str) -> String {
    compile_source(source, AsmDialect::LinuxElf, false)
        .expect("program should lower")
        .text
}

fn macho_assembly(source: &str) -> String {
    compile_source(source, AsmDialect::MachO, false)
        .expect("program should lower")
        .text
}

fn error_message(source: &str) -> String {
    compile_source(source, AsmDialect::LinuxElf, false).expect_err("program should be rejected")
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
fn snapshot_debugger_transparency() {
    // `debugger` lowers to a comment only, so the completion already in `x0`
    // (here `n * 2`) flows through it unchanged.
    insta::assert_snapshot!(assembly(
        "let f = fn(n) { n * 2; debugger; };\ndebugger;\nputs(f(21));"
    ));
}

#[test]
fn snapshot_boxed_integer_literal() {
    // i64::MAX exceeds the SMI range: materialize + rt_box_int.
    insta::assert_snapshot!(assembly("9223372036854775807;"));
}

#[test]
fn snapshot_observe_mode() {
    insta::assert_snapshot!(
        compile_source("1 + 2;", AsmDialect::LinuxElf, true)
            .unwrap()
            .text
    );
}

#[test]
fn snapshot_macho_dialect() {
    // Same lowering, Mach-O spelling: `_`-prefixed calls, `L` labels,
    // `@PAGE`/`@PAGEOFF`, `__TEXT,__const`, and `.zerofill` globals.
    insta::assert_snapshot!(macho_assembly(
        "let msg = \"hé\";\nlet inc = fn(n) { n + 1 };\nif (inc(1) < 3) { msg } else { \"no\" };"
    ));
}

#[test]
fn macho_never_leaks_elf_spellings() {
    let text = macho_assembly(
        "class Counter {\n  constructor(start) { this.count = start; }\n  inc() { this.count = this.count + 1; this.count }\n}\nlet c = new Counter(5);\nc.inc();\nlet a = [c.count, len(\"x\")];\n-a[0];\n!true;\nputs(a);"
    );
    assert!(!text.contains(":lo12:"));
    assert!(!text.contains(".L"));
    assert!(!text.contains("bl rt_"));
    assert!(text.contains("bl _rt_call"));
    assert!(text.contains(".zerofill __DATA,__bss,g_globals,"));
}

#[test]
fn line_spans_point_into_the_source() {
    let source = "let x = 41;\nx + 1;";
    let assembly = compile_source(source, AsmDialect::LinuxElf, false).unwrap();
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
    assert!(compile_source("fn(a, b, c, d, e, f, g) { 0 };", AsmDialect::LinuxElf, false).is_ok());
    assert!(compile_source("class C { m(a, b, c, d, e, f) { 0 } }", AsmDialect::LinuxElf, false)
        .is_ok());
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

#[test]
fn a_let_inside_a_block_converges_on_a_slot_picked_before_it() {
    // Blocks are not scopes, so the read after the branch means whatever the
    // block bound — but the block is skippable, so that cannot be the block's
    // own slot or the read lands on unwritten memory (see #335). The branch
    // gets a slot of its own, seeded before the jump and written by the arm.
    let text = assembly("let x = 1; if (false) { let x = 2; } x;");
    assert!(text.contains("3 global slot(s)"), "{}", text);
    assert!(text.contains("shadow x"), "{}", text);

    // A name the branch introduces converges the same way, on a slot seeded
    // with null so that an arm not binding it leaves the name worth null
    // rather than reading the other arm's slot.
    let text = assembly("if (false) { let n = 2; } else { let n = 3; } n;");
    assert!(text.contains("shadow seed: null"), "{}", text);
    assert!(text.contains("shadow n"), "{}", text);

    // Two `let`s in one block still take a slot each (here: x, f, x, plus a
    // shadow for each of the two names), so a closure made between them keeps
    // reading what it captured.
    let text = assembly("if (true) { let x = 1; let f = fn() { x }; let x = 2; f() };");
    assert!(text.contains("5 global slot(s)"), "{}", text);
}

#[test]
fn type_annotations_lower_to_identical_assembly() {
    // The native backend erases annotations too (design §6): every emitted
    // instruction has to match the unannotated program's. Only the trailing
    // `//` comments differ, because those echo the source line verbatim — the
    // same carve-out the bytecode backends make for debug info.
    let pairs = [
        (
            "let add = fn(a: int, b: int): int { a + b }; add(1, 2);",
            "let add = fn(a, b) { a + b }; add(1, 2);",
        ),
        (
            "class C { constructor(x: int) { this.x = x; } get(): int { this.x } } new C(1).get();",
            "class C { constructor(x) { this.x = x; } get() { this.x } } new C(1).get();",
        ),
        ("let xs: [int] = [1, 2]; xs[0];", "let xs = [1, 2]; xs[0];"),
    ];

    for (annotated, erased) in pairs {
        assert_eq!(
            strip_comments(&assembly(annotated)),
            strip_comments(&assembly(erased)),
            "assembly differs for {}",
            annotated
        );
        assert_eq!(
            strip_comments(&macho_assembly(annotated)),
            strip_comments(&macho_assembly(erased)),
            "Mach-O assembly differs for {}",
            annotated
        );
    }

    // And the comments are exactly where the difference shows up.
    assert!(assembly("let x: int = 1;").contains("// let x: int = 1;"));
}

/// Drops trailing `//` comments and the blank lines they leave behind.
fn strip_comments(text: &str) -> String {
    return text
        .lines()
        .map(|line| line.split("//").next().unwrap_or("").trim_end())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
}
