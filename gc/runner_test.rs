use std::collections::HashMap;
use std::rc::Rc;

use compiler::compiler::{Bytecode, DebugInfo};
use compiler::op_code::{Instructions, Opcode};
use compiler::snapshot::{read_bytecode, write_bytecode};
use object::Object;

use crate::runner::{compile_source, run_bytecode, run_bytecode_with_output};

/// Representative programs for direct-vs-snapshot equivalence (design doc
/// §8): closure capture, recursion, class/instance, array/hash plus
/// builtins, string concatenation.
const EQUIVALENCE_PROGRAMS: &[(&str, &str)] = &[
    ("closure capture", "let make = fn(a) { fn(b) { a + b } }; make(20)(22)"),
    ("recursion", "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(10)"),
    (
        "class instance",
        r#"
        class Point {
          constructor(x, y) { this.x = x; this.y = y; }
          sum() { return this.x + this.y; }
        }
        new Point(20, 22).sum()
        "#,
    ),
    (
        "array and hash builtins",
        r#"let h = {"a": 1, "b": 2}; let arr = push([h["a"], h["b"]], 3); first(arr) + last(arr) + len(arr)"#,
    ),
    ("string concat", r#""hello" + " " + "world""#),
];

#[test]
fn snapshot_roundtrip_execution_matches_direct_execution() {
    for (name, source) in EQUIVALENCE_PROGRAMS {
        let direct = run_bytecode(compile_source(source).unwrap(), usize::MAX).unwrap();
        let blob = write_bytecode(&compile_source(source).unwrap(), false).unwrap();
        let via_snapshot = run_bytecode(read_bytecode(&blob).unwrap(), usize::MAX).unwrap();
        assert_eq!(direct, via_snapshot, "program: {}", name);
    }
}

#[test]
fn runtime_error_spans_survive_the_snapshot() {
    let source = "let not_callable = 5; not_callable()";
    let direct = run_bytecode(compile_source(source).unwrap(), usize::MAX).unwrap_err();
    assert!(direct.span.is_some(), "direct run should attach a span");

    let blob = write_bytecode(&compile_source(source).unwrap(), false).unwrap();
    let with_debug = run_bytecode(read_bytecode(&blob).unwrap(), usize::MAX).unwrap_err();
    assert_eq!(direct, with_debug);

    let stripped_blob = write_bytecode(&compile_source(source).unwrap(), true).unwrap();
    let stripped = run_bytecode(read_bytecode(&stripped_blob).unwrap(), usize::MAX).unwrap_err();
    assert_eq!(stripped.message, direct.message);
    assert_eq!(stripped.span, None);
}

#[test]
fn instruction_budget_is_enforced() {
    let source = "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } }; fib(30)";
    let error = run_bytecode(compile_source(source).unwrap(), 10).unwrap_err();
    assert!(error.message.contains("instruction limit exceeded"), "got: {}", error.message);
}

#[test]
fn captured_output_survives_a_later_runtime_error() {
    let bytecode = compile_source(r#"puts("one", 2); 1 / 0"#).unwrap();
    let (result, stdout) = run_bytecode_with_output(bytecode, usize::MAX);
    assert_eq!(stdout, "one\n2\n");
    assert_eq!(result.unwrap_err().kind.as_str(), "arithmetic");
}

fn hostile_bytecode(instructions: Vec<u8>, constants: Vec<Rc<Object>>) -> Bytecode {
    Bytecode {
        instructions: Instructions {
            data: instructions,
        },
        constants,
        debug_info: DebugInfo::default(),
        function_debug_info: HashMap::new(),
    }
}

/// Hostile-but-structurally-valid bytecode passes the reader's L1 checks by
/// design (L1 does not track stack depth or closure shapes), so the VM's own
/// L3 checks must turn it into runtime errors, never panics.
#[test]
fn structurally_valid_hostile_bytecode_errors_instead_of_panicking() {
    let cases = vec![
        ("lone OpPop underflows the stack", vec![Opcode::OpPop as u8], vec![]),
        ("OpGetFree reads outside the closure", vec![Opcode::OpGetFree as u8, 0], vec![]),
        (
            "OpCall on an integer",
            vec![Opcode::OpConst as u8, 0, 0, Opcode::OpCall as u8, 0],
            vec![Rc::new(Object::Integer(7))],
        ),
    ];
    for (name, instructions, constants) in cases {
        let blob = write_bytecode(&hostile_bytecode(instructions, constants), false).unwrap();
        let bytecode = read_bytecode(&blob).expect(name);
        let error = run_bytecode(bytecode, 10_000).unwrap_err();
        assert!(!error.message.is_empty(), "case: {}", name);
    }
}

#[test]
fn runaway_recursion_hits_the_frame_limit() {
    let source = "let spin = fn() { spin() }; spin()";
    let blob = write_bytecode(&compile_source(source).unwrap(), false).unwrap();
    let error = run_bytecode(read_bytecode(&blob).unwrap(), usize::MAX).unwrap_err();
    assert_eq!(error.message, "frame limit exceeded");
}

/// Design doc §8: single-byte corruptions that still read back `Ok` are
/// legitimate files for some other program, so the only requirement is that
/// executing them never panics. The finite budget bounds corruptions that
/// redirect a jump into an infinite loop.
#[test]
fn bit_flipped_snapshots_never_panic_the_vm() {
    let (_, source) = EQUIVALENCE_PROGRAMS[2];
    let blob = write_bytecode(&compile_source(source).unwrap(), false).unwrap();
    for index in 0..blob.len() {
        for pattern in [0x01u8, 0x80, 0xff] {
            let mut mutated = blob.clone();
            mutated[index] ^= pattern;
            if let Ok(bytecode) = read_bytecode(&mutated) {
                let _ = run_bytecode(bytecode, 10_000);
            }
        }
    }
}
