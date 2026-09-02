//! Type annotations must be erased before code generation: see
//! docs/type-system-design.md section 6. Every pair below is the *same* program
//! written twice — once fully annotated, once with every annotation removed —
//! and the compiler has to produce identical bytecode for both.

#[cfg(test)]
mod tests {
    use parser::parse;
    use std::rc::Rc;

    use crate::compiler::{Bytecode, Compiler};
    use crate::snapshot::write_bytecode;
    use crate::vm::VM;
    use object::Object;

    /// `(annotated, erased)` pairs. The erased side is what a reader would get
    /// by deleting every `: T` from the annotated side, nothing else.
    const PAIRS: &[(&str, &str)] = &[
        ("let x: int = 5; x", "let x = 5; x"),
        (
            "let add = fn(a: int, b: int): int { a + b }; add(1, 2)",
            "let add = fn(a, b) { a + b }; add(1, 2)",
        ),
        (
            "let pick = fn(xs: [int], i: int): int? { xs[i] }; pick([1, 2], 0)",
            "let pick = fn(xs, i) { xs[i] }; pick([1, 2], 0)",
        ),
        (
            "let apply = fn(f: fn(int): int, v: int): int { f(v) }; apply(fn(n: int): int { n }, 3)",
            "let apply = fn(f, v) { f(v) }; apply(fn(n) { n }, 3)",
        ),
        (
            "let m: {string: [int]} = {\"a\": [1]}; m[\"a\"]",
            "let m = {\"a\": [1]}; m[\"a\"]",
        ),
        (
            "let outer = fn(a: int): fn(int): int { fn(b: int): int { a + b } }; outer(1)(2)",
            "let outer = fn(a) { fn(b) { a + b } }; outer(1)(2)",
        ),
        (
            "class Point { constructor(x: int, y: int) { this.x = x; this.y = y; } sum(): int { this.x + this.y } } let p = new Point(1, 2); p.sum()",
            "class Point { constructor(x, y) { this.x = x; this.y = y; } sum() { this.x + this.y } } let p = new Point(1, 2); p.sum()",
        ),
        (
            "if (true) { let a: string = \"y\"; a } else { let b: bool = false; b }",
            "if (true) { let a = \"y\"; a } else { let b = false; b }",
        ),
    ];

    fn compile(source: &str) -> Bytecode {
        let program =
            parse(source).unwrap_or_else(|e| panic!("{} failed to parse: {}", source, e[0]));
        let mut compiler = Compiler::new();
        return compiler
            .compile(&program)
            .unwrap_or_else(|e| panic!("{} failed to compile: {}", source, e));
    }

    /// Constants compare structurally, except `CompiledFunction`, whose name is
    /// the only field that could smuggle source text through.
    fn describe_constants(constants: &[Rc<Object>]) -> Vec<String> {
        return constants
            .iter()
            .map(|constant| match constant.as_ref() {
                Object::CompiledFunction(function) => format!(
                    "fn {} locals={} params={} {:?}",
                    function.name,
                    function.num_locals,
                    function.num_parameters,
                    function.instructions
                ),
                other => format!("{:?}", other),
            })
            .collect();
    }

    #[test]
    fn annotations_do_not_change_instructions_or_constants() {
        for (annotated, erased) in PAIRS {
            let with_types = compile(annotated);
            let without_types = compile(erased);

            assert_eq!(
                with_types.instructions.data, without_types.instructions.data,
                "instructions differ for {}",
                annotated
            );
            assert_eq!(
                describe_constants(&with_types.constants),
                describe_constants(&without_types.constants),
                "constants differ for {}",
                annotated
            );
        }
    }

    #[test]
    fn annotations_do_not_change_stripped_snapshots() {
        for (annotated, erased) in PAIRS {
            let with_types =
                write_bytecode(&compile(annotated), true).expect("annotated bytecode serializes");
            let without_types =
                write_bytecode(&compile(erased), true).expect("erased bytecode serializes");

            assert_eq!(with_types, without_types, "stripped snapshots differ for {}", annotated);
        }
    }

    #[test]
    fn annotations_do_not_change_binding_debug_info() {
        // Spans are the one part of the artifact allowed to move, because they
        // are absolute byte offsets into source that got longer. Names and
        // slots must not.
        for (annotated, erased) in PAIRS {
            let with_types = compile(annotated);
            let without_types = compile(erased);

            assert_eq!(
                with_types.debug_info.local_bindings, without_types.debug_info.local_bindings,
                "local bindings differ for {}",
                annotated
            );
            assert_eq!(
                with_types.debug_info.free_names, without_types.debug_info.free_names,
                "free names differ for {}",
                annotated
            );
            assert_eq!(
                with_types.debug_info.pc_spans.len(),
                without_types.debug_info.pc_spans.len(),
                "span count differs for {}",
                annotated
            );

            let mut annotated_functions = with_types
                .function_debug_info
                .keys()
                .copied()
                .collect::<Vec<_>>();
            let mut erased_functions = without_types
                .function_debug_info
                .keys()
                .copied()
                .collect::<Vec<_>>();
            annotated_functions.sort_unstable();
            erased_functions.sort_unstable();
            assert_eq!(
                annotated_functions, erased_functions,
                "function debug info keys differ for {}",
                annotated
            );

            for index in annotated_functions {
                let annotated_info = &with_types.function_debug_info[&index];
                let erased_info = &without_types.function_debug_info[&index];
                assert_eq!(
                    annotated_info.local_bindings, erased_info.local_bindings,
                    "local bindings differ for function {} of {}",
                    index, annotated
                );
                assert_eq!(
                    annotated_info.free_names, erased_info.free_names,
                    "free names differ for function {} of {}",
                    index, annotated
                );
            }
        }
    }

    #[test]
    fn the_vm_never_prints_annotations() {
        // Unlike the tree-walking interpreter, the VM only ever sees a closure,
        // so no annotation can reach its output.
        let mut vm = VM::new(compile("fn(x: int?): [string] { x }"));
        vm.run_checked().expect("annotated function runs");
        assert_eq!(vm.last_popped_stack_elm().unwrap().to_string(), "[closure function]");
    }

    #[test]
    fn unstripped_snapshots_still_carry_the_wider_spans() {
        // The escape hatch is deliberate: debug info records absolute offsets,
        // so an annotated program's snapshot is expected to differ *only* once
        // debug info is kept.
        let annotated = write_bytecode(&compile("let x: int = 5; x"), false).unwrap();
        let erased = write_bytecode(&compile("let x = 5; x"), false).unwrap();
        assert_ne!(annotated, erased);
    }
}
