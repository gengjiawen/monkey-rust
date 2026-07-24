use crate::compiler::Compiler;
use crate::compiler_test::test_constants;
use crate::vm::VM;
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
        vm.run();
        let got = vm.last_popped_stack_elm().unwrap();
        let expected_argument = t.expected;
        test_constants(&[expected_argument], &vec![got]);
    }
}

#[cfg(test)]
mod tests {
    use object::Object;
    use std::collections::HashMap;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;

    use crate::compiler::Compiler;
    use crate::vm::VM;
    use crate::vm_test::{run_vm_tests, VmTestCase};
    use parser::parse;

    fn vm_panic_message(input: &str) -> String {
        let program = parse(input).unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();
        let mut vm = VM::new(bytecode);
        let panic = catch_unwind(AssertUnwindSafe(|| vm.run())).expect_err("VM should panic");
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                panic
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_string())
            })
            .unwrap_or_else(|| "non-string panic".to_string())
    }

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
    fn class_runtime_errors_use_user_visible_arity() {
        let cases = [
            (
                "class Empty {} new Empty(1);",
                "wrong number of arguments for Empty.constructor: want=0, got=1",
            ),
            (
                "class Point { constructor(x) {} } new Point();",
                "wrong number of arguments for Point.constructor: want=1, got=0",
            ),
            (
                "class Counter { increment(amount) { amount; } } new Counter().increment();",
                "wrong number of arguments for Counter.increment: want=1, got=0",
            ),
            ("class Empty {} Empty();", "class Empty must be constructed with new"),
            ("let factory = fn() {}; new factory();", "cannot construct [closure function]"),
            ("class Empty {} new Empty().missing;", "property 'missing' does not exist on Empty"),
            ("1.value;", "cannot read property 'value' of 1"),
            ("1.value = 2;", "cannot set property 'value' of 1"),
        ];

        for (input, expected) in cases {
            assert_eq!(vm_panic_message(input), expected, "input: {input}");
        }
    }
}
