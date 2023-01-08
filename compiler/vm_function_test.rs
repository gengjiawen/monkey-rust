#[cfg(test)]
mod tests {
    use object::Object;
    use crate::vm_test::{run_vm_tests, VmTestCase};

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
}
