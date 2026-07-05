use compiler::compiler::Compiler;
use object::Object;
use parser::parse;
use std::collections::HashMap;
use std::rc::Rc;

use crate::GcVM;

pub struct VmTestCase<'a> {
    pub input: &'a str,
    pub expected: Object,
}

pub fn run_gc_vm_tests(tests: Vec<VmTestCase>) {
    for test in tests {
        let program = parse(test.input)
            .unwrap_or_else(|errors| panic!("parse error for {:?}: {}", test.input, errors[0]));
        let mut compiler = Compiler::new();
        let bytecode = compiler
            .compile(&program)
            .unwrap_or_else(|error| panic!("compile error for {:?}: {}", test.input, error));
        let mut vm = GcVM::new(bytecode);
        vm.run();
        let got = vm
            .export_last_result()
            .unwrap_or_else(|| panic!("no result on stack for {:?}", test.input));
        assert_eq!(got, test.expected, "input: {:?}", test.input);
    }
}

fn int_array(values: &[i64]) -> Object {
    Object::Array(
        values
            .iter()
            .map(|value| Rc::new(Object::Integer(*value)))
            .collect(),
    )
}

fn int_hash(pairs: &[(i64, i64)]) -> Object {
    Object::Hash(
        pairs
            .iter()
            .map(|(k, v)| (Rc::new(Object::Integer(*k)), Rc::new(Object::Integer(*v))))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer_arithmetic() {
        run_gc_vm_tests(vec![
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
        ]);
    }

    #[test]
    fn test_boolean_expressions() {
        run_gc_vm_tests(vec![
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
                input: "1 == 1",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "1 != 2",
                expected: Object::Boolean(true),
            },
            VmTestCase {
                input: "true == false",
                expected: Object::Boolean(false),
            },
            VmTestCase {
                input: "(1 < 2) == true",
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
                input: "!!5",
                expected: Object::Boolean(true),
            },
        ]);
    }

    #[test]
    fn test_conditionals() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "if (true) { 10 }",
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
        ]);
    }

    #[test]
    fn test_global_let_statements() {
        run_gc_vm_tests(vec![
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
        ]);
    }

    #[test]
    fn test_strings() {
        run_gc_vm_tests(vec![
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
        ]);
    }

    #[test]
    fn test_arrays() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "[]",
                expected: int_array(&[]),
            },
            VmTestCase {
                input: "[1, 2, 3]",
                expected: int_array(&[1, 2, 3]),
            },
            VmTestCase {
                input: "[1 + 2, 3 * 4, 5 + 6]",
                expected: int_array(&[3, 12, 11]),
            },
        ]);
    }

    #[test]
    fn test_hash() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "{}",
                expected: Object::Hash(HashMap::new()),
            },
            VmTestCase {
                input: "{1: 2, 2: 3}",
                expected: int_hash(&[(1, 2), (2, 3)]),
            },
            VmTestCase {
                input: "{1 + 1: 2 * 2, 3 + 3: 4 * 4}",
                expected: int_hash(&[(2, 4), (6, 16)]),
            },
        ]);
    }

    #[test]
    fn test_index() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "[1, 2, 3][1]",
                expected: Object::Integer(2),
            },
            VmTestCase {
                input: "[1, 2, 3][0 + 2]",
                expected: Object::Integer(3),
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
                input: "{1: 1}[0]",
                expected: Object::Null,
            },
            VmTestCase {
                input: "{}[0]",
                expected: Object::Null,
            },
        ]);
    }

    #[test]
    fn test_functions_without_arguments() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "let fivePlusTen = fn() { 5 + 10; }; fivePlusTen();",
                expected: Object::Integer(15),
            },
            VmTestCase {
                input: "let one = fn() { 1; }; let two = fn() { 2; }; one() + two();",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input:
                    "let a = fn() { 1 }; let b = fn() { a() + 1 }; let c = fn() { b() + 1 }; c();",
                expected: Object::Integer(3),
            },
        ]);
    }

    #[test]
    fn test_functions_without_return_value() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "let noReturn = fn() {}; noReturn();",
                expected: Object::Null,
            },
            VmTestCase {
                input: "let noReturn = fn() {}; let noReturnTwo = fn() { noReturn() }; noReturn(); noReturnTwo();",
                expected: Object::Null,
            },
        ]);
    }

    #[test]
    fn test_calling_functions_with_bindings() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "let one = fn() { let one = 1; one }; one();",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let oneAndTwo = fn() { let one = 1; let two = 2; one + two }; oneAndTwo();",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let globalSeed = 50; let minusOne = fn() { let num = 1; globalSeed - num }; let minusTwo = fn() { let num = 2; globalSeed - num }; minusOne() + minusTwo();",
                expected: Object::Integer(97),
            },
        ]);
    }

    #[test]
    fn test_calling_functions_with_arguments_and_bindings() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "let identity = fn(a) { a }; identity(4);",
                expected: Object::Integer(4),
            },
            VmTestCase {
                input: "let sum = fn(a, b) { a + b }; sum(1, 2);",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let sum = fn(a, b) { let c = a + b; c }; sum(1, 2);",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let sum = fn(a, b) { let c = a + b; c; }; let outer = fn() { sum(1, 2) + sum(3, 4); }; outer();",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "let globalNum = 10; let sum = fn(a, b) { let c = a + b; c + globalNum; }; let outer = fn() { sum(1, 2) + sum(3, 4) + globalNum; }; outer() + globalNum;",
                expected: Object::Integer(50),
            },
        ]);
    }

    #[test]
    fn test_closures() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "let newAdder = fn(a, b) { fn(c) { a + b + c } }; let adder = newAdder(1, 2); adder(8);",
                expected: Object::Integer(11),
            },
            VmTestCase {
                input: "let global = 10; let outer = fn(a) { let inner = fn(b) { a + b + global }; inner }; let adder = outer(2); adder(3);",
                expected: Object::Integer(15),
            },
        ]);
    }

    #[test]
    fn test_builtins() {
        run_gc_vm_tests(vec![
            VmTestCase {
                input: "len(\"\");",
                expected: Object::Integer(0),
            },
            VmTestCase {
                input: "len(\"four\");",
                expected: Object::Integer(4),
            },
            VmTestCase {
                input: "len(\"hello world\");",
                expected: Object::Integer(11),
            },
            VmTestCase {
                input: "len(\"one\", \"two\");",
                expected: Object::Error("builtin len expected 1 argument, got 2".to_string()),
            },
            VmTestCase {
                input: "len([1, 2, 3]);",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "len([]);",
                expected: Object::Integer(0),
            },
            VmTestCase {
                input: "first([1, 2, 3]);",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "first([]);",
                expected: Object::Null,
            },
            VmTestCase {
                input: "last([1, 2, 3]);",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "last([]);",
                expected: Object::Null,
            },
            VmTestCase {
                input: "rest([1, 2, 3]);",
                expected: int_array(&[2, 3]),
            },
            VmTestCase {
                input: "rest([]);",
                expected: Object::Null,
            },
            VmTestCase {
                input: "push([], 1);",
                expected: int_array(&[1]),
            },
        ]);
    }

    #[test]
    fn builtin_call_releases_callee_args_and_stack_temporaries() {
        let program = parse("len([1, 2, 3]);").unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();
        let mut vm = GcVM::new(bytecode);

        vm.run();
        assert_eq!(vm.export_last_result(), Some(Object::Integer(3)));

        vm.heap_mut().run_gc();
        assert_eq!(vm.heap().runtime().gc_object_count(), 6);
    }

    #[test]
    fn test_eval_source_helper() {
        let result = crate::eval_source("1 + 2 * 3").unwrap();
        assert_eq!(result, Object::Integer(7));
    }
}
