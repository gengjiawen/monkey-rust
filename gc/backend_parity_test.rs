//! Cross-backend semantic conformance.
//!
//! The tree-walking interpreter, the bytecode VM and this GC VM are three
//! independent implementations of one language, and nothing but a test keeps
//! them honest: each used to have its own idea of what `!null`, `1 == "a"`,
//! `9223372036854775807 + 1` and `{"b": 2, "a": 1}` mean. The rules live in
//! `docs/arm64-asm-backend-design.md` §10.1/§10.2 (the frozen semantics
//! matrix); this file runs one corpus through all three backends and fails
//! when any of them disagrees with the matrix.
//!
//! It lives in the `gc` crate because that is the only crate that can already
//! see the other two backends; `monkey-interpreter` is a dev-dependency added
//! for exactly this suite.
//!
//! Deliberately **not** compared yet, each tracked separately:
//!
//! * error message wording — all three agree on *which* programs fail, but the
//!   interpreter still phrases the failures differently ("can't apply prefix
//!   minus operator: true" vs "unsupported type for negation: true");
//! * builtin failures (`len(1)`) — the two VMs still push `Object::Error` as a
//!   value instead of terminating;
//! * `let x = 5;` as the final statement — the interpreter answers `null`, the
//!   bytecode VM answers `5`;
//! * closure equality — `fn(x) { x } == fn(x) { x }` is structural in the
//!   interpreter and identity-ish in the VMs. `f == f` is in the corpus, two
//!   separately written functions are not.

use std::cell::RefCell;
use std::rc::Rc;

use compiler::compiler::Compiler;
use compiler::vm::VM;
use object::environment::Env;

/// What a backend answered. `Error` keeps the message for the failure report
/// only; see the module docs for why the wording is not compared.
#[derive(Debug, Eq, PartialEq)]
enum Outcome {
    Value(String),
    Error(String),
}

/// What the frozen matrix says the answer is.
#[derive(Debug)]
enum Expect {
    /// The rendered final value, identical in every backend.
    Value(&'static str),
    /// Every backend rejects the program at runtime.
    RuntimeError,
}

fn interpreter_outcome(source: &str) -> Outcome {
    let program = match parser::parse(source) {
        Ok(program) => program,
        Err(errors) => return Outcome::Error(errors[0].clone()),
    };
    let env: Env = Rc::new(RefCell::new(Default::default()));
    match interpreter::eval(program, &env) {
        Ok(value) => Outcome::Value(value.to_string()),
        Err(error) => Outcome::Error(error),
    }
}

fn compiler_outcome(source: &str) -> Outcome {
    let program = match parser::parse(source) {
        Ok(program) => program,
        Err(errors) => return Outcome::Error(errors[0].clone()),
    };
    let mut compiler = Compiler::new();
    let bytecode = match compiler.compile(&program) {
        Ok(bytecode) => bytecode,
        Err(error) => return Outcome::Error(error),
    };
    let mut vm = VM::new(bytecode);
    match vm.run_checked() {
        Ok(()) => Outcome::Value(
            vm.last_popped_stack_elm()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "<nothing was popped>".to_string()),
        ),
        Err(error) => Outcome::Error(error.message),
    }
}

fn gc_outcome(source: &str) -> Outcome {
    match crate::run_source_with_report(source, usize::MAX) {
        Ok(success) => Outcome::Value(success.result),
        Err(error) => Outcome::Error(error.message),
    }
}

type Backend = (&'static str, fn(&str) -> Outcome);

const BACKENDS: &[Backend] = &[
    ("interpreter", interpreter_outcome),
    ("bytecode vm", compiler_outcome),
    ("gc vm", gc_outcome),
];

fn check(cases: &[(&str, Expect)]) {
    let mut failures = Vec::new();
    for (source, expected) in cases {
        for (name, run) in BACKENDS {
            let actual = run(source);
            let agrees = match (expected, &actual) {
                (Expect::Value(expected), Outcome::Value(actual)) => expected == actual,
                (Expect::RuntimeError, Outcome::Error(_)) => true,
                _ => false,
            };
            if !agrees {
                failures.push(format!(
                    "  {}\n    program : {}\n    expected: {:?}\n    actual  : {:?}",
                    name, source, expected, actual
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} backend(s) disagree with the frozen semantics matrix:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// §10.1, integers: every arithmetic operation is checked, and division by
/// zero is its own failure. The GC VM used to wrap silently and hand back
/// `-9223372036854775808` where the other two raised.
#[test]
fn test_integer_arithmetic_is_checked_in_every_backend() {
    check(&[
        ("9223372036854775807 + 1", Expect::RuntimeError),
        ("0 - 9223372036854775807 - 2", Expect::RuntimeError),
        ("9223372036854775807 * 2", Expect::RuntimeError),
        ("let x = 9223372036854775807; x + x", Expect::RuntimeError),
        ("1 / 0", Expect::RuntimeError),
        ("let x = 0; 1 / x", Expect::RuntimeError),
        ("-(0 - 9223372036854775807 - 1)", Expect::RuntimeError),
        // The boundary cases that must still succeed.
        ("9223372036854775807 + 0", Expect::Value("9223372036854775807")),
        ("0 - 9223372036854775807 - 1", Expect::Value("-9223372036854775808")),
        ("-9223372036854775807", Expect::Value("-9223372036854775807")),
        ("6 / 4", Expect::Value("1")),
        ("(0 - 7) / 2", Expect::Value("-3")),
    ]);
}

/// §10.1, scalar and aggregate equality: `==` and `!=` are total. Values of
/// different types are unequal, never a type error, and arrays and hashes
/// compare recursively and independently of iteration order.
#[test]
fn test_equality_is_total_and_structural() {
    check(&[
        ("1 == 1", Expect::Value("true")),
        ("1 == 2", Expect::Value("false")),
        ("1 == \"a\"", Expect::Value("false")),
        ("1 != \"a\"", Expect::Value("true")),
        ("true == 1", Expect::Value("false")),
        ("true != 1", Expect::Value("true")),
        ("\"a\" == \"a\"", Expect::Value("true")),
        ("\"a\" == \"b\"", Expect::Value("false")),
        ("let n = if (false) { 1 }; n == n", Expect::Value("true")),
        ("let n = if (false) { 1 }; n == 1", Expect::Value("false")),
        ("let n = if (false) { 1 }; n != 1", Expect::Value("true")),
        ("let n = if (false) { 1 }; n == false", Expect::Value("false")),
        ("[] == []", Expect::Value("true")),
        ("[1, 2] == [1, 2]", Expect::Value("true")),
        ("[1, 2] == [1, 3]", Expect::Value("false")),
        ("[1, 2] != [1, 3]", Expect::Value("true")),
        ("[1] == [1, 2]", Expect::Value("false")),
        ("[1, [2, [3]]] == [1, [2, [3]]]", Expect::Value("true")),
        ("[1, [2, [3]]] == [1, [2, [4]]]", Expect::Value("false")),
        ("[1] == 1", Expect::Value("false")),
        ("{} == {}", Expect::Value("true")),
        ("{\"a\": 1} == {\"a\": 1}", Expect::Value("true")),
        ("{\"a\": 1} == {\"a\": 2}", Expect::Value("false")),
        ("{\"a\": 1} == {\"b\": 1}", Expect::Value("false")),
        ("{\"a\": 1} == {\"a\": 1, \"b\": 2}", Expect::Value("false")),
        // Key order must not matter.
        ("{\"a\": 1, \"b\": 2} == {\"b\": 2, \"a\": 1}", Expect::Value("true")),
        ("{\"a\": [1, 2]} == {\"a\": [1, 2]}", Expect::Value("true")),
        ("{\"a\": [1, 2]} == {\"a\": [1, 3]}", Expect::Value("false")),
        ("{\"a\": 1} == [1]", Expect::Value("false")),
        // §10.1, identity equality: same object, tested inside the program.
        ("let f = fn(x) { x }; f == f", Expect::Value("true")),
        ("let f = fn(x) { x }; let g = f; f == g", Expect::Value("true")),
        ("let a = [1]; let b = a; a == b", Expect::Value("true")),
        // `>` / `<` stay integer-only.
        ("1 < 2", Expect::Value("true")),
        ("2 > 1", Expect::Value("true")),
        ("\"a\" < \"b\"", Expect::RuntimeError),
        ("true > false", Expect::RuntimeError),
        ("1 > true", Expect::RuntimeError),
    ]);
}

/// §10.1, truthiness: only `false` and `null` are falsy, and `!v` is exactly
/// `!truthy(v)`. Both VMs used to answer `false` for `!null` while branching
/// on `null` as false — `!n` and `if (n)` contradicted each other.
#[test]
fn test_bang_is_the_inverse_of_truthiness() {
    check(&[
        ("!true", Expect::Value("false")),
        ("!false", Expect::Value("true")),
        ("!!true", Expect::Value("true")),
        ("!0", Expect::Value("false")),
        ("!1", Expect::Value("false")),
        ("!\"\"", Expect::Value("false")),
        ("![]", Expect::Value("false")),
        ("!{}", Expect::Value("false")),
        ("let n = if (false) { 1 }; !n", Expect::Value("true")),
        ("let n = if (false) { 1 }; !!n", Expect::Value("false")),
        // `!` and `if` must read the same value the same way.
        ("let n = if (false) { 1 }; if (!n) { 1 } else { 2 }", Expect::Value("1")),
        ("let n = if (false) { 1 }; if (n) { 1 } else { 2 }", Expect::Value("2")),
        ("if (!0) { 1 } else { 2 }", Expect::Value("2")),
        ("if (0) { 1 } else { 2 }", Expect::Value("1")),
    ]);
}

/// §10.2, language display: a hash renders as `{k: v}` — not `[k: v]` — and
/// its entries are ordered by `(key type rank, canonical key bytes)`, so the
/// same hash prints the same way on every run and in every backend. Walking
/// `HashMap` iteration order printed a different string each time.
#[test]
fn test_hash_display_is_brace_delimited_and_stably_ordered() {
    check(&[
        ("{}", Expect::Value("{}")),
        ("{\"a\": 1}", Expect::Value("{a: 1}")),
        ("{\"b\": 2, \"a\": 1}", Expect::Value("{a: 1, b: 2}")),
        // Canonical bytes, so integers sort as decimal text.
        ("{1: 1, 2: 2, 10: 10}", Expect::Value("{1: 1, 10: 10, 2: 2}")),
        ("{10: 10, 2: 2, 1: 1}", Expect::Value("{1: 1, 10: 10, 2: 2}")),
        ("{true: 1, false: 2}", Expect::Value("{false: 2, true: 1}")),
        // Rank first: integer, then boolean, then string.
        ("{\"a\": 3, true: 2, 1: 1}", Expect::Value("{1: 1, true: 2, a: 3}")),
        ("{\"a\": {\"c\": 3, \"b\": 2}}", Expect::Value("{a: {b: 2, c: 3}}")),
        ("[{\"b\": 2, \"a\": 1}]", Expect::Value("[{a: 1, b: 2}]")),
    ]);
}

/// The rest of the language, so the three backends cannot drift apart in the
/// parts that already agreed.
#[test]
fn test_core_language_agrees_across_backends() {
    check(&[
        ("1", Expect::Value("1")),
        ("1 + 2 * 3", Expect::Value("7")),
        ("(1 + 2) * 3", Expect::Value("9")),
        ("true", Expect::Value("true")),
        ("\"mon\" + \"key\"", Expect::Value("monkey")),
        ("[1, [2, 3], \"x\"]", Expect::Value("[1, [2, 3], x]")),
        ("[1, 2, 3][1]", Expect::Value("2")),
        // Out of range and missing keys are `null`, not errors.
        ("[1, 2, 3][9]", Expect::Value("null")),
        ("[1, 2, 3][0 - 1]", Expect::Value("null")),
        ("{\"a\": 1}[\"b\"]", Expect::Value("null")),
        ("{\"a\": 1}[\"a\"]", Expect::Value("1")),
        ("if (1 > 2) { 1 }", Expect::Value("null")),
        ("if (1 < 2) { 1 } else { 2 }", Expect::Value("1")),
        ("let add = fn(a, b) { a + b }; add(1, 2)", Expect::Value("3")),
        ("let make = fn(a) { fn(b) { a + b } }; make(1)(2)", Expect::Value("3")),
        (
            "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)",
            Expect::Value("55"),
        ),
        ("fn() { return 1; 2 }()", Expect::Value("1")),
        ("len(\"abc\")", Expect::Value("3")),
        ("len([1, 2])", Expect::Value("2")),
        ("first([1, 2])", Expect::Value("1")),
        ("last([1, 2])", Expect::Value("2")),
        ("rest([1, 2, 3])", Expect::Value("[2, 3]")),
        ("push([1], 2)", Expect::Value("[1, 2]")),
        (
            "class Counter { constructor(start) { this.count = start; } next() { this.count + 1 } } \
             let counter = new Counter(41); counter.next()",
            Expect::Value("42"),
        ),
        (
            "class Box { constructor(value) { this.value = value; } } \
             let box = new Box(1); box.value",
            Expect::Value("1"),
        ),
        (
            "class Box { constructor(value) { this.value = value; } } \
             let box = new Box(1); box == box",
            Expect::Value("true"),
        ),
        (
            "class Box { constructor(value) { this.value = value; } } \
             new Box(1) == new Box(1)",
            Expect::Value("false"),
        ),
        // Failures every backend must agree on, wording aside.
        ("1 + true", Expect::RuntimeError),
        ("true + true", Expect::RuntimeError),
        ("\"a\" - \"b\"", Expect::RuntimeError),
        ("-true", Expect::RuntimeError),
        ("-\"a\"", Expect::RuntimeError),
        ("1[0]", Expect::RuntimeError),
        ("{[1]: 2}", Expect::RuntimeError),
        ("let f = 1; f(1)", Expect::RuntimeError),
        ("undefined_name", Expect::RuntimeError),
    ]);
}
