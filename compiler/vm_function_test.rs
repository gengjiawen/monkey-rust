#[cfg(test)]
mod tests {
    use crate::vm::VmRuntimeErrorKind;
    use crate::vm_test::{run_vm_tests, vm_runtime_error, VmTestCase};
    use object::Object;
    use std::rc::Rc;

    #[test]
    fn test_function_without_arguments() {
        let tests = vec![
            VmTestCase {
                input: "let fivePlusTen= fn() { 5 + 10; }; \
                    fivePlusTen();",
                expected: Object::Integer(15),
            },
            VmTestCase {
                input: "let one = fn() { 1; }; \
                    let two = fn() { 2; }; \
                    one() + two();",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let a = fn() { 1 }; \
                 let b = fn() { a() + 1 }; \
                 let c = fn() { b() + 1 }; \
                 c();",
                expected: Object::Integer(3),
            },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_function_without_return_value() {
        let tests = vec![
            VmTestCase {
                input: "let noReturn = fn() {}; \
                   noReturn();",
                expected: Object::Null,
            },
            VmTestCase {
                input: "let noReturn = fn() {}; \
                   let noReturnTwo = fn() { noReturn() }; \
                   noReturn(); \
                   noReturnTwo();",
                expected: Object::Null,
            },
        ];
        run_vm_tests(tests);
    }

    #[test]
    fn test_calling_functions_with_bindings() {
        let tests = vec![
            VmTestCase {
                input: "let one = fn() { let one = 1; one }; \
                    one();",
                expected: Object::Integer(1),
            },
            VmTestCase {
                input: "let oneAndTwo = fn() { let one = 1; let two = 2; one + two }; \
                    oneAndTwo();",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let oneAndTwo = fn() { let one = 1; let two = 2; one + two }; \
                    let threeAndFour = fn() { let three = 3; let four = 4; three + four }; \
                    oneAndTwo() + threeAndFour();",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "let firstFooBar = fn() { let foobar = 50; foobar }; \
                    let secondFooBar = fn() { let foobar = 100; foobar }; \
                    firstFooBar() + secondFooBar();",
                expected: Object::Integer(150),
            },
            VmTestCase {
                input: "let globalSeed = 50; \
                    let minusOne = fn() { \
                        let num = 1; \
                        globalSeed - num \
                    }; \
                    let minusTwo = fn() { \
                        let num = 2; \
                        globalSeed - num \
                    }; \
                    minusOne() + minusTwo();",
                expected: Object::Integer(97),
            },
        ];
        run_vm_tests(tests);
    }

    #[test]
    fn test_calling_functions_with_arguments_and_bindings() {
        let tests = vec![
            VmTestCase {
                input: "let identity = fn(a) { a }; \
                    identity(4);",
                expected: Object::Integer(4),
            },
            VmTestCase {
                input: "let sum = fn(a, b) { a + b }; \
                    sum(1, 2);",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let sum = fn(a, b) { let c = a + b; c }; \
                    sum(1, 2);",
                expected: Object::Integer(3),
            },
            VmTestCase {
                input: "let sum = fn(a, b) { \
                            let c = a + b; \
                            c; \
                        }; \
                        sum(1, 2) + sum(3, 4);",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "let sum = fn(a, b) { \
                            let c = a + b; \
                            c; \
                        }; \
                        let outer = fn() { \
                            sum(1, 2) + sum(3, 4); \
                        };\
                        outer();",
                expected: Object::Integer(10),
            },
            VmTestCase {
                input: "let globalNum = 10; \
                    let sum = fn(a, b) { \
                        let c = a + b; \
                        c + globalNum; \
                    }; \
                    let outer = fn() { \
                        sum(1, 2) + sum(3, 4) + globalNum; \
                    }; \
                    outer() + globalNum;",
                expected: Object::Integer(50),
            },
        ];
        run_vm_tests(tests);
    }

    #[test]
    fn test_builtins() {
        let tests = vec![
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
                expected: Object::Array(vec![
                    Rc::from(Object::Integer(2)),
                    Rc::from(Object::Integer(3)),
                ]),
            },
            VmTestCase {
                input: "rest([]);",
                expected: Object::Null,
            },
            VmTestCase {
                input: "push([], 1);",
                expected: Object::Array(vec![Rc::from(Object::Integer(1))]),
            },
        ];
        run_vm_tests(tests);
    }

    #[test]
    fn test_builtin_failures_are_runtime_errors() {
        // A failing builtin aborts the program, the same way the interpreter
        // and the asm backend do; the message must never survive as a value.
        let tests = [
            ("len(1);", "builtin len not supported for type 1"),
            ("len(\"one\", \"two\");", "builtin len expected 1 argument, got 2"),
            ("first();", "builtin first expected 1 argument, got 0"),
            ("first(1);", "builtin first not supported for type 1"),
            ("first([1], [2]);", "builtin first expected 1 argument, got 2"),
            ("last(1);", "builtin last not supported for type 1"),
            ("last([1], [2]);", "builtin last expected 1 argument, got 2"),
            ("rest(1);", "builtin rest not supported for type 1"),
            ("rest([1], [2]);", "builtin rest expected 1 argument, got 2"),
            ("push(1, 1);", "builtin push not supported for type 1"),
            ("push([1]);", "builtin push expected 2 arguments, got 1"),
            ("push([1], 2, 3);", "builtin push expected 2 arguments, got 3"),
        ];

        for (input, message) in tests {
            let error = vm_runtime_error(input);
            assert_eq!(error.kind, VmRuntimeErrorKind::Call, "input: {:?}", input);
            assert_eq!(error.message, message, "input: {:?}", input);
        }
    }

    #[test]
    fn a_failed_builtin_never_flows_on_as_a_value() {
        // Before this was an Err, the error object was pushed on the stack and
        // kept being used: `len(1) + 1` produced another error value, and the
        // message was truthy, so `if (len(1))` took the consequent.
        let tests = [
            "len(1) + 1",
            "[len(1), 2]",
            "if (len(1)) { \"truthy\" } else { \"falsy\" }",
            "len(1) == len(1)",
            "len(1); 42",
            "let broken = len(1); broken",
            "let identity = fn(x) { x }; identity(len(1))",
        ];

        for input in tests {
            let error = vm_runtime_error(input);
            assert_eq!(error.kind, VmRuntimeErrorKind::Call, "input: {:?}", input);
            assert_eq!(
                error.message, "builtin len not supported for type 1",
                "input: {:?}",
                input
            );
        }
    }
}
