use std::collections::HashMap;
use std::rc::Rc;

use compiler::compiler::{Bytecode, DebugInfo};
use compiler::op_code::{Instructions, Opcode};
use compiler::snapshot::{read_bytecode, write_bytecode};
use object::{CompiledFunction, Object};

use crate::runner::{compile_source, run_bytecode, run_bytecode_with_output};
use crate::{GcRunError, GcRunStage, GcRuntimeError, GcVM};

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
    ("debugger transparency", "let f = fn(n) { n * 2; debugger; }; debugger; f(21)"),
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

#[test]
fn taking_output_keeps_capture_enabled() {
    let mut vm = GcVM::new(compile_source(r#"puts("one")"#).unwrap());
    vm.set_capture_output(true);
    vm.run_with_budget(usize::MAX).unwrap();
    assert_eq!(vm.take_output(), "one\n");

    vm.load_bytecode(compile_source(r#"puts("two")"#).unwrap());
    vm.run_with_budget(usize::MAX).unwrap();
    assert_eq!(vm.take_output(), "two\n");
}

#[test]
fn legacy_error_structs_remain_constructible() {
    let runtime_error = GcRuntimeError {
        message: "runtime".to_string(),
        span: None,
    };
    let run_error = GcRunError {
        stage: GcRunStage::Runtime,
        message: runtime_error.message,
        span: runtime_error.span,
    };

    assert_eq!(run_error.message, "runtime");
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

/// `read_bytecode` walks operand widths, so a stream that ends mid-operand
/// never reaches the VM through a `.mbc` file. `Bytecode` is a plain struct
/// with public fields, though, so one can reach it directly — and the dispatch
/// loop runs the final byte of the stream as an opcode, leaving its operand
/// off the end.
#[test]
fn an_operand_past_the_end_of_the_stream_is_a_runtime_error() {
    use crate::vm::GcRuntimeErrorKind;

    // Both truncations of each operand: entirely absent, and half written.
    let cases: Vec<(Vec<u8>, &str)> = vec![
        (vec![Opcode::OpConst as u8], "OpConst"),
        (vec![Opcode::OpConst as u8, 0], "OpConst"),
        (vec![Opcode::OpJump as u8, 0], "OpJump"),
        (vec![Opcode::OpJumpNotTruthy as u8, 0], "OpJumpNotTruthy"),
        (vec![Opcode::OpGetGlobal as u8, 0], "OpGetGlobal"),
        (vec![Opcode::OpArray as u8, 0], "OpArray"),
        (vec![Opcode::OpGetLocal as u8], "OpGetLocal"),
        (vec![Opcode::OpSetLocal as u8], "OpSetLocal"),
        (vec![Opcode::OpCall as u8], "OpCall"),
        (vec![Opcode::OpGetBuiltin as u8], "OpGetBuiltin"),
        (vec![Opcode::OpGetFree as u8], "OpGetFree"),
        (vec![Opcode::OpNew as u8], "OpNew"),
        (vec![Opcode::OpGetProperty as u8, 0], "OpGetProperty"),
        // Two-byte operand present, the trailing one-byte one missing.
        (vec![Opcode::OpClosure as u8, 0, 0], "OpClosure"),
        (vec![Opcode::OpMethod as u8, 0, 0], "OpMethod"),
    ];

    for (instructions, opcode) in cases {
        let mut vm = GcVM::new(hostile_bytecode(instructions, vec![]));
        let error = vm
            .run_with_budget_classified(10_000)
            .expect_err(&format!("{} has no operand to read", opcode));
        assert_eq!(error.kind, GcRuntimeErrorKind::InvalidBytecode);
        assert_eq!(
            error.message,
            format!("{} operand runs past the end of its instructions", opcode)
        );
    }
}

/// A frame's locals are bounded by the function's own `num_locals`: a higher
/// index still lands inside the stack, but on the caller's operands rather
/// than on a local of this call.
#[test]
fn a_local_index_past_the_frames_locals_is_rejected() {
    use crate::vm::GcRuntimeErrorKind;

    let function = Rc::new(Object::CompiledFunction(Rc::new(CompiledFunction {
        name: "one_local".to_string(),
        instructions: vec![Opcode::OpGetLocal as u8, 200, Opcode::OpReturnValue as u8],
        num_locals: 1,
        num_parameters: 0,
    })));
    let bytecode = hostile_bytecode(
        vec![
            Opcode::OpClosure as u8,
            0,
            0,
            0,
            Opcode::OpCall as u8,
            0,
            Opcode::OpPop as u8,
        ],
        vec![function],
    );

    let mut vm = GcVM::new(bytecode);
    let error = vm
        .run_with_budget_classified(10_000)
        .expect_err("local 200 is outside a frame holding one local");
    assert_eq!(error.kind, GcRuntimeErrorKind::InvalidBytecode);
    assert_eq!(error.message, "local index 200 out of range for a frame with 1 locals");
}

#[test]
fn oversized_function_locals_fail_before_frame_allocation() {
    let function = Rc::new(Object::CompiledFunction(Rc::new(CompiledFunction {
        name: "oversized".to_string(),
        instructions: vec![Opcode::OpReturn as u8],
        num_locals: usize::MAX,
        num_parameters: 0,
    })));
    let bytecode = hostile_bytecode(
        vec![Opcode::OpClosure as u8, 0, 0, 0, Opcode::OpCall as u8, 0],
        vec![function],
    );

    let error = run_bytecode(bytecode, 10_000).unwrap_err();
    assert_eq!(error.message, "stack limit exceeded");
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

/// Type annotations never reach the GcVM: it runs the same bytecode either
/// way, and its closures have no source text to leak (design §6).
#[test]
fn type_annotations_are_erased_before_execution() {
    let pairs = [
        (
            "let add = fn(a: int, b: int): int { a + b }; add(20, 22)",
            "let add = fn(a, b) { a + b }; add(20, 22)",
        ),
        (
            "class Point { constructor(x: int, y: int) { this.x = x; this.y = y; } sum(): int { return this.x + this.y; } } new Point(20, 22).sum()",
            "class Point { constructor(x, y) { this.x = x; this.y = y; } sum() { return this.x + this.y; } } new Point(20, 22).sum()",
        ),
    ];

    for (annotated, erased) in pairs {
        let with_types = run_bytecode(compile_source(annotated).unwrap(), usize::MAX).unwrap();
        let without_types = run_bytecode(compile_source(erased).unwrap(), usize::MAX).unwrap();
        assert_eq!(with_types, without_types, "results differ for {}", annotated);
        assert_eq!(with_types, "42");
    }

    let closure =
        run_bytecode(compile_source("fn(x: int?): [string] { x }").unwrap(), usize::MAX).unwrap();
    assert_eq!(closure, "[closure function]");
}
