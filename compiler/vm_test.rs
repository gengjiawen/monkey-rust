use crate::compiler::Compiler;
use crate::compiler_test::test_constants;
use crate::vm::{VmRuntimeError, VM};
use object::Object;
use parser::parse;

pub struct VmTestCase<'a> {
    pub(crate) input: &'a str,
    pub(crate) expected: Object,
}

pub fn run_vm_tests(tests: Vec<VmTestCase>) {
    for t in tests {
        let program = parse(t.input).unwrap();
        let mut compiler = Compiler::new();
        let bytecodes = compiler.compile(&program).unwrap();
        println!("ins {} for input {}", bytecodes.instructions.string(), t.input);
        let mut vm = VM::new(bytecodes);
        vm.run_checked()
            .unwrap_or_else(|error| panic!("VM error for {:?}: {}", t.input, error));
        let got = vm.last_popped_stack_elm().unwrap();
        let expected_argument = t.expected;
        test_constants(&[expected_argument], &vec![got]);
    }
}

/// Run `input` and return the runtime error it must raise.
pub fn vm_runtime_error(input: &str) -> VmRuntimeError {
    let program = parse(input).unwrap();
    let mut compiler = Compiler::new();
    let bytecode = compiler.compile(&program).unwrap();
    let mut vm = VM::new(bytecode);
    vm.run_checked().expect_err("VM should return an error")
}

#[cfg(test)]
mod tests {
    use object::Object;
    use std::collections::HashMap;
    use std::rc::Rc;

    use crate::compiler::Compiler;
    use crate::vm::{VmRuntimeErrorKind, VM};
    use crate::vm_test::{run_vm_tests, vm_runtime_error, VmTestCase};
    use parser::parse;

    #[test]
    fn test_integer_arithmetic() {
        let tests: Vec<VmTestCase> = vec![
            VmTestCase {
                input: "1",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "2",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "1 + 2",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "4 / 2",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "50 / 2 * 2 + 10 - 5",
                expected: Object::Integer(55),
            },
            VmTestCase {
                input: "5 * (2 + 10)",
                expected: Object::Integer(60),
            },
            VmTestCase {
                input: "5 + 5 + 5 + 5 - 10",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "2 * 2 * 2 * 2 * 2",
                expected: Object::Integer(32),
            },
            VmTestCase {
                input: "5 * 2 + 10",
                expected: Object::Integer(20),
            },
            VmTestCase {
                input: "5 + 2 * 10",
                expected: Object::Integer(25),
            },
            VmTestCase {
                input: "5 * (2 + 10)",
                expected: Object::Integer(60),
            },
            VmTestCase {
                input: "-5",
                expected: Object::Integer(-5),
            },
            VmTestCase {
                input: "-10",
                expected: Object::Integer(-10),
            },
            VmTestCase {
                input: "-50 + 100 + -50",
                expected: Object::Integer(0),
            },
            VmTestCase {
                input: "(5 + 10 * 2 + 15 / 3) * 2 + -10",
                expected: Object::Integer(50),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_boolean_expressions() {
        let tests: Vec<VmTestCase> = vec![
            VmTestCase {
                input: "true",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "false",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "true",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "false",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "1 < 2",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "1 > 2",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "1 < 1",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "1 > 1",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "1 == 1",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "1 != 1",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "1 == 2",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "1 != 2",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "true == true",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "false == false",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "true == false",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "true != false",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "false != true",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "(1 < 2) == true",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "(1 < 2) == false",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "(1 > 2) == true",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "(1 > 2) == false",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "!true",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "!false",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "!5",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "!!true",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "!!false",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "!!5",
                expected: Object::Boolean(true),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_conditionals() {
        let tests = vec![
            VmTestCase {
                input: "if (true) { 10 }",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "if (true) { 10 } else { 20 }",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "if (false) { 10 } else { 20 }",
                expected: Object::Integer(20),
            },
            VmTestCase {
                input: "if (1) { 10 }",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "if (1 < 2) { 10 }",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "if (1 < 2) { 10 } else { 20 }",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "if (1 > 2) { 10 } else { 20 }",
                expected: Object::Integer(20),
            },
            VmTestCase {
                input: "if (1 > 2) { 10 }",
                expected: Object::Null,
            },
            VmTestCase {
                input: "if (false) { 10 }",
                expected: Object::Null,
            },
            VmTestCase {
                input: "if ((if (false) { 10 })) { 10 } else { 20 }",
                expected: Object::Integer(20),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_conditionals_without_values() {
        let tests = vec![
            VmTestCase {
                input: "if (true) { let y = 1; }",
                expected: Object::Null,
            },
            VmTestCase {
                input: "if (true) {} 2",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "if (false) { 1 } else {}",
                expected: Object::Null,
            },
            VmTestCase {
                input: "let result = if (true) { let y = 1; } else { 2 }; result",
                expected: Object::Null,
            },
            VmTestCase {
                input: "let f = fn() { if (true) { let y = 2; }; y }; f()",
                expected: Object::Integer(2),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_global_let_statements() {
        let tests = vec![
            VmTestCase {
                input: "let one = 1; one",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let one = 1; let two = 2; one + two",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let one = 1; let two = one + one; one + two",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let x = 1; let x = x + 2; x",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let wrapper = fn() { let count = fn(n) { if (n > 0) { count(n - 1) } else { 7 } }; count(2) }; wrapper()",
                expected: Object::Integer(7),
            },
            VmTestCase {
                input: "class Counter { constructor() { this.value = 0; } next() { this.value = this.value + 1; this.value } } let counter = new Counter(); counter.next() < counter.next()",
                expected: Object::Boolean(true),
            },
        ];

        run_vm_tests(tests);
    }

    /// A block is not a scope: a `let` inside one rebinds the name that is
    /// still visible after the block. The store therefore has to hit the slot
    /// later reads use, or a branch that does not run leaves them reading an
    /// uninitialised slot (see #335).
    #[test]
    fn test_let_inside_a_block_rebinds_the_outer_name() {
        let tests = vec![
            VmTestCase {
                input: "let x = 1; if (false) { let x = \"shadow\"; } x + 1",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "let x = 1; if (true) { let x = 2; } x",
                expected: Object::Integer(2),
            },
            // Several `let`s of one name in one arm: the last one the arm ran
            // is what the name means afterwards, and an arm that did not run
            // leaves the binding from before the branch alone.
            VmTestCase {
                input: "let x = 1; if (false) { let x = 2; let x = 3; } x",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let x = 1; if (true) { let x = 2; let x = 3; } x",
                expected: Object::Integer(3),
            },
            // The else arm rebinds the same name; whichever arm ran decides.
            VmTestCase {
                input: "let x = 1; if (true) { let x = 2; } else { let x = 3; } x",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "let x = 1; if (false) { let x = 2; } else { let x = 3; } x",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let x = 1; if (true) { 0 } else { let x = 3; } x",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let x = 1; if (false) { 0 } else { let x = 3; } x",
                expected: Object::Integer(3),
            },
            // Sequential and nested blocks, each conditional on its own.
            VmTestCase {
                input: "if (true) { let x = 1; } if (false) { let x = 2; } x",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "if (false) { let x = 1; } if (true) { let x = 2; } x",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "let x = 1; if (true) { if (true) { let x = 5; } } x",
                expected: Object::Integer(5),
            },
            VmTestCase {
                input: "let x = 1; if (true) { if (false) { let x = 2; } } x",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let x = 1; if (false) { if (true) { let x = 2; } } x",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let x = 1; if (true) { if (false) { let x = 2; } let x = 4; } x",
                expected: Object::Integer(4),
            },
            VmTestCase {
                input: "let x = 1; if (true) { if (false) { let x = 2; } x }",
                expected: Object::Integer(1),
            },
            // A closure made before the branch keeps reading what it captured,
            // whichever way the branch went.
            VmTestCase {
                input: "let x = 1; let f = fn() { x }; if (true) { let x = 2; } f()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let x = 1; let f = fn() { x }; if (false) { let x = 2; } f()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let z = 1; if (true) { let z = 2; let f = fn() { z }; let z = 9; f() }",
                expected: Object::Integer(2),
            },
            // Once the block closes the name is unconditional again, so the
            // next `let` binds anew and a closure made earlier keeps reading
            // what it captured — the block in between changes nothing.
            VmTestCase {
                input: "let x = 1; let f = fn() { x }; if (false) { let x = 2; } let x = 3; f()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "if (true) { let y = 1; let g = fn() { y }; } let y = 2; g()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let x = 1; if (true) { let x = 2; if (true) { let x = 3; } let f = fn() { x }; let x = 4; f() }",
                expected: Object::Integer(3),
            },
            // Function locals and parameters go the same way through frame
            // slots, and a captured free variable is untouched by an arm that
            // shadows it.
            VmTestCase {
                input: "fn() { let y = 1; if (false) { let y = 2; } y }()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "fn() { let y = 1; if (true) { let y = 2; } y }()",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "fn(p) { if (true) { let p = 5; } p }(1)",
                expected: Object::Integer(5),
            },
            VmTestCase {
                input: "let f = fn() { let x = 1; if (false) { let x = 2; let x = 3; } x }; f()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let f = fn() { let x = 1; if (true) { let x = 2; let x = 3; } x }; f()",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let x = 1; let f = fn() { if (false) { let x = 2; } x }; f()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let x = 1; let f = fn() { if (true) { let x = 2; } x }; f()",
                expected: Object::Integer(2),
            },
            // The arm is still an expression: its own value is what the `if`
            // evaluates to, rebinding or not.
            VmTestCase {
                input: "let x = 1; let y = if (true) { let x = 2; 42 }; y",
                expected: Object::Integer(42),
            },
            VmTestCase {
                input: "let x = 1; if (true) { let x = 2; x }",
                expected: Object::Integer(2),
            },
            // A name neither the branch nor anything before it had a binding
            // for still has to mean one thing afterwards: both arms converge
            // on the same slot rather than each keeping its own.
            VmTestCase {
                input: "if (true) { let n = 2; } else { let n = 3; } n",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "if (false) { let n = 2; } else { let n = 3; } n",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "if (true) { let n = 2; let n = 5; } else { let n = 3; } n",
                expected: Object::Integer(5),
            },
            VmTestCase {
                input: "if (false) { if (true) { let n = 2; } else { let n = 3; } } else { let n = 4; } n",
                expected: Object::Integer(4),
            },
            VmTestCase {
                input: "let f = fn() { if (true) { let n = 2; } else { let n = 3; } n }; f()",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "let f = fn(p) { if (true) { let p = 2; } else { let p = 3; } p }; f(9)",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "if (true) { let n = 1; let g = fn() { n }; } else { let n = 2; let g = fn() { n }; } g()",
                expected: Object::Integer(1),
            },
            // The condition runs before either arm, so a rebinding it makes is
            // in force for both of them and for the code after the branch.
            VmTestCase {
                input: "let x = 1; let y = if (if (true) { let x = 2; false }) { let x = 3; 30 } else { let y = x; let x = 4; y }; y",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "let x = 1; let y = if (if (true) { let x = 2; true }) { let y = x; let x = 3; y } else { 40 }; y",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "let x = 1; if (if (true) { let x = 2; false }) { let x = 3; } else { 0 } x",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "let x = 1; if (if (true) { let x = 2; true }) { 0 } else { 0 } x",
                expected: Object::Integer(2),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_strings() {
        let tests = vec![
            VmTestCase {
                input: "\"monkey\"",
                expected: Object::String("monkey".to_string()),
            },
            VmTestCase {
                input: "\"mon\" + \"key\"",
                expected: Object::String("monkey".to_string()),
            },
            VmTestCase {
                input: "\"mon\" + \"key\" + \"banana\"",
                expected: Object::String("monkeybanana".to_string()),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_arrays() {
        fn map_vec_to_object(vec: Vec<i64>) -> Object {
            let array = vec
                .iter()
                .map(|i| Rc::new(Object::Integer(*i)))
                .collect::<Vec<Rc<Object>>>();
            return Object::Array(array);
        }
        let tests = vec![
            VmTestCase {
                input: "[]",
                expected: map_vec_to_object(vec![]),
            },
            VmTestCase {
                input: "[1, 2, 3]",
                expected: map_vec_to_object(vec![1, 2, 3]),
            },
            VmTestCase {
                input: "[1 + 2, 3 * 4, 5 + 6]",
                expected: map_vec_to_object(vec![3, 12, 11]),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_hash() {
        #[allow(clippy::mutable_key_type)]
        fn map_vec_to_object(vec: Vec<(i64, i64)>) -> Object {
            let hash = vec.iter().fold(HashMap::new(), |mut acc, (k, v)| {
                acc.insert(Rc::new(Object::Integer(*k)), Rc::new(Object::Integer(*v)));
                acc
            });
            return Object::Hash(hash);
        }
        let tests = vec![
            VmTestCase {
                input: "{}",
                expected: Object::Hash(HashMap::new()),
            },
            VmTestCase {
                input: "{1: 2, 2: 3}",
                expected: map_vec_to_object(vec![(1, 2), (2, 3)]),
            },
            VmTestCase {
                input: "{1 + 1: 2 * 2, 3 + 3: 4 * 4}",
                expected: map_vec_to_object(vec![(2, 4), (6, 16)]),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_index() {
        let tests = vec![
            VmTestCase {
                input: "[1, 2, 3][1]",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "[1, 2, 3][0 + 2]",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "[1, 2, 3][0]",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "[[1, 1, 1]][0][0]",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "[][0]",
                expected: Object::Null,
            },
            VmTestCase {
                input: "[1, 2, 3][99]",
                expected: Object::Null,
            },
            VmTestCase {
                input: "[1][-1]",
                expected: Object::Null,
            },
            VmTestCase {
                input: "{1: 1, 2: 2}[1]",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "{1: 1, 2: 2}[2]",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "{1: 1}[0]",
                expected: Object::Null,
            },
            VmTestCase {
                input: "{}[0]",
                expected: Object::Null,
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_top_level_return() {
        run_vm_tests(vec![
            VmTestCase {
                input: "return 1;",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "if (true) { return 5; } 9;",
                expected: Object::Integer(5),
            },
            VmTestCase {
                input: "let f = fn() { 2 }; return f() + 1; 9;",
                expected: Object::Integer(3),
            },
        ]);
    }

    #[test]
    fn test_debugger_is_a_no_op_with_transparent_completion() {
        run_vm_tests(vec![
            VmTestCase {
                input: "debugger;",
                expected: Object::Null,
            },
            VmTestCase {
                input: "1; debugger;",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "fn() { 1; debugger; }()",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "fn() { let x = 1; debugger; }()",
                expected: Object::Null,
            },
            VmTestCase {
                input: "fn() { return 2; debugger; }()",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "fn(x) { if (x) { 1; debugger; } }(true)",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "if (true) { debugger; }",
                expected: Object::Null,
            },
            VmTestCase {
                input: "class Counter { constructor(start) { this.count = start; debugger; } value() { this.count; debugger; } } let counter = new Counter(41); counter.value() + 1;",
                expected: Object::Integer(42),
            },
        ]);
    }

    #[test]
    fn test_class_semantics() {
        run_vm_tests(vec![
            VmTestCase {
                input: "class Point { constructor(x, y) { this.x = x; this.y = y; } sum() { this.x + this.y; } } let point = new Point(20, 22); point.sum();",
                expected: Object::Integer(42),
            },
            VmTestCase {
                input: "class Mutable { constructor(value) { this.value = value; } } let item = new Mutable(1); item.value = 42; item.value;",
                expected: Object::Integer(42),
            },
            VmTestCase {
                input: "class Counter { constructor(value) { this.value = value; } current() { this.value; } } let counter = new Counter(42); let current = counter.current; current();",
                expected: Object::Integer(42),
            },
            VmTestCase {
                input: "class Box { constructor(value) { this.value = value; } reader() { fn() { fn() { this.value; }; }; } } let read = new Box(42).reader()(); read();",
                expected: Object::Integer(42),
            },
            VmTestCase {
                input: "class Example { value() { 1; } } let example = new Example(); example.value = 42; example.value;",
                expected: Object::Integer(42),
            },
            VmTestCase {
                input: "class Empty {}",
                expected: Object::Null,
            },
            VmTestCase {
                input: "class Empty {} let empty = new Empty(); empty.value = 1;",
                expected: Object::Null,
            },
            VmTestCase {
                input: "class Empty {} let Type = Empty; new Type() == new Type();",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "class Empty {} let Type = Empty; Empty == Type;",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "class Example { value() { 1; } } let example = new Example(); let method = example.value; method == method;",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "class Example { value() { 1; } } let example = new Example(); example.value == example.value;",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "class Trace { constructor() { this.order = 0; } mark(value) { this.order = this.order * 10 + value; value; } target() { this.mark(1); this; } } class Pair { constructor(left, right) { this.value = left + right; } } let trace = new Trace(); trace.target().value = trace.mark(2); let pair = new Pair(trace.mark(3), trace.mark(4)); trace.order;",
                expected: Object::Integer(1234),
            },
        ]);
    }

    #[test]
    fn runtime_errors_are_returned_instead_of_panicking() {
        let cases = [
            ("1 + \"a\";", VmRuntimeErrorKind::Type, "unsupported binary operation for 1 and a"),
            (
                "let add = fn(a, b) { a + b; }; add(1);",
                VmRuntimeErrorKind::Call,
                "wrong number of arguments: want=2, got=1",
            ),
            ("1();", VmRuntimeErrorKind::Call, "calling non-closure"),
            (
                "true > false;",
                VmRuntimeErrorKind::Type,
                "unsupported comparison for true and false",
            ),
            ("-\"a\";", VmRuntimeErrorKind::Type, "unsupported type for negation OpMinus: a"),
            ("1[0];", VmRuntimeErrorKind::Index, "unsupported index operation for 1 with 0"),
            ("{[]: 1};", VmRuntimeErrorKind::Index, "hash key must be hashable, got []"),
            ("1 / 0;", VmRuntimeErrorKind::Arithmetic, "division by zero"),
            (
                "9223372036854775807 + 1;",
                VmRuntimeErrorKind::Arithmetic,
                "integer overflow in addition",
            ),
            (
                "let recurse = fn() { recurse(); }; recurse();",
                VmRuntimeErrorKind::Stack,
                "frame limit exceeded",
            ),
        ];

        for (input, kind, message) in cases {
            let error = vm_runtime_error(input);
            assert_eq!(error.kind, kind, "input: {input}");
            assert_eq!(error.message, message, "input: {input}");
        }
    }

    #[test]
    fn legacy_run_retains_runtime_error_without_panicking() {
        let program = parse("1 + \"a\";").unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();
        let mut vm = VM::new(bytecode);

        vm.run();

        let error = vm.last_error().expect("legacy run should retain its error");
        assert_eq!(error.kind, VmRuntimeErrorKind::Type);
        assert_eq!(error.message, "unsupported binary operation for 1 and a");
    }

    #[test]
    fn class_runtime_errors_use_user_visible_arity() {
        let cases = [
            (
                "class Empty {} new Empty(1);",
                VmRuntimeErrorKind::Call,
                "wrong number of arguments for Empty.constructor: want=0, got=1",
            ),
            (
                "class Point { constructor(x) {} } new Point();",
                VmRuntimeErrorKind::Call,
                "wrong number of arguments for Point.constructor: want=1, got=0",
            ),
            (
                "class Counter { increment(amount) { amount; } } new Counter().increment();",
                VmRuntimeErrorKind::Call,
                "wrong number of arguments for Counter.increment: want=1, got=0",
            ),
            (
                "class Empty {} Empty();",
                VmRuntimeErrorKind::Call,
                "class Empty must be constructed with new",
            ),
            (
                "let factory = fn() {}; new factory();",
                VmRuntimeErrorKind::Call,
                "cannot construct [closure function]",
            ),
            (
                "class Empty {} new Empty().missing;",
                VmRuntimeErrorKind::Property,
                "property 'missing' does not exist on Empty",
            ),
            ("1.value;", VmRuntimeErrorKind::Property, "cannot read property 'value' of 1"),
            ("1.value = 2;", VmRuntimeErrorKind::Property, "cannot set property 'value' of 1"),
        ];

        for (input, kind, expected) in cases {
            let error = vm_runtime_error(input);
            assert_eq!(error.kind, kind, "input: {input}");
            assert_eq!(error.message, expected, "input: {input}");
        }
    }
}
