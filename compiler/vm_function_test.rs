#[cfg(test)]
mod tests {
    use crate::vm_test::{run_vm_tests, VmTestCase};
    use object::Object;

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
            }
        ];
        run_vm_tests(tests);
    }
}
