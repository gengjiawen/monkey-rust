use object::Object;

struct VmTestCase<'a> {
    input: &'a str,
    expected: Object,
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use object::Object;
    use parser::parse;

    use crate::compiler::Compiler;
    use crate::compiler_test::test_constants;
    use crate::vm::VM;
    use crate::vm_test::VmTestCase;

    fn test_expected_object(expected: Object, got: Rc<Object>) {
        test_constants(&vec![expected], &vec![got]);
    }

    fn run_vm_tests(tests: Vec<VmTestCase>) {
        for t in tests {
            let program = parse(t.input).unwrap();
            let mut compiler = Compiler::new();
            let bytecodes = compiler.compile(&program).unwrap();
            println!("ins {} for input {}", bytecodes.instructions.string(), t.input);
            let mut vm = VM::new(bytecodes);
            vm.run();
            let got = vm.last_popped_stack_elm().unwrap();
            test_expected_object(t.expected, got);
        }
    }

    #[test]
    fn test_integer_arithmetic() {
        let tests: Vec<VmTestCase> = vec![
            VmTestCase { input: "1", expected: Object::Integer(1) },
            VmTestCase { input: "2", expected: Object::Integer(2) },
            VmTestCase { input: "1 + 2", expected: Object::Integer(3) },
            VmTestCase { input: "4 / 2", expected: Object::Integer(2) },
            VmTestCase { input: "50 / 2 * 2 + 10 - 5", expected: Object::Integer(55) },
            VmTestCase { input: "5 * (2 + 10)", expected: Object::Integer(60) },
            VmTestCase { input: "5 + 5 + 5 + 5 - 10", expected: Object::Integer(10) },
            VmTestCase { input: "2 * 2 * 2 * 2 * 2", expected: Object::Integer(32) },
            VmTestCase { input: "5 * 2 + 10", expected: Object::Integer(20) },
            VmTestCase { input: "5 + 2 * 10", expected: Object::Integer(25) },
            VmTestCase { input: "5 * (2 + 10)", expected: Object::Integer(60) },
            VmTestCase { input: "-5", expected: Object::Integer(-5) },
            VmTestCase { input: "-10", expected: Object::Integer(-10) },
            VmTestCase { input: "-50 + 100 + -50", expected: Object::Integer(0) },
            VmTestCase { input: "(5 + 10 * 2 + 15 / 3) * 2 + -10", expected: Object::Integer(50) },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_boolean_expressions() {
        let tests: Vec<VmTestCase> = vec![
            VmTestCase { input: "true", expected: Object::Boolean(true) },
            VmTestCase { input: "false", expected: Object::Boolean(false) },
            VmTestCase { input: "true", expected: Object::Boolean(true) },
            VmTestCase { input: "false", expected: Object::Boolean(false) },
            VmTestCase { input: "1 < 2", expected: Object::Boolean(true) },
            VmTestCase { input: "1 > 2", expected: Object::Boolean(false) },
            VmTestCase { input: "1 < 1", expected: Object::Boolean(false) },
            VmTestCase { input: "1 > 1", expected: Object::Boolean(false) },
            VmTestCase { input: "1 == 1", expected: Object::Boolean(true) },
            VmTestCase { input: "1 != 1", expected: Object::Boolean(false) },
            VmTestCase { input: "1 == 2", expected: Object::Boolean(false) },
            VmTestCase { input: "1 != 2", expected: Object::Boolean(true) },
            VmTestCase { input: "true == true", expected: Object::Boolean(true) },
            VmTestCase { input: "false == false", expected: Object::Boolean(true) },
            VmTestCase { input: "true == false", expected: Object::Boolean(false) },
            VmTestCase { input: "true != false", expected: Object::Boolean(true) },
            VmTestCase { input: "false != true", expected: Object::Boolean(true) },
            VmTestCase { input: "(1 < 2) == true", expected: Object::Boolean(true) },
            VmTestCase { input: "(1 < 2) == false", expected: Object::Boolean(false) },
            VmTestCase { input: "(1 > 2) == true", expected: Object::Boolean(false) },
            VmTestCase { input: "(1 > 2) == false", expected: Object::Boolean(true) },
            VmTestCase { input: "!true", expected: Object::Boolean(false) },
            VmTestCase { input: "!false", expected: Object::Boolean(true) },
            VmTestCase { input: "!5", expected: Object::Boolean(false) },
            VmTestCase { input: "!!true", expected: Object::Boolean(true) },
            VmTestCase { input: "!!false", expected: Object::Boolean(false) },
            VmTestCase { input: "!!5", expected: Object::Boolean(true) },
            // VmTestCase { input: "!(if (false) { 5; })", expected: Object::Boolean(true) },
        ];

        run_vm_tests(tests);
    }

    #[test]
    fn test_conditionals() {
        let tests = vec![
            VmTestCase { input: "if (true) { 10 }", expected: Object::Integer(10) },
            VmTestCase { input: "if (true) { 10 } else { 20 }", expected: Object::Integer(10) },
            VmTestCase { input: "if (false) { 10 } else { 20 }", expected: Object::Integer(20) },
            VmTestCase { input: "if (1) { 10 }", expected: Object::Integer(10) },
            VmTestCase { input: "if (1 < 2) { 10 }", expected: Object::Integer(10) },
            VmTestCase { input: "if (1 < 2) { 10 } else { 20 }", expected: Object::Integer(10) },
            VmTestCase { input: "if (1 > 2) { 10 } else { 20 }", expected: Object::Integer(20) },
            // VmTestCase{input: "if (1 > 2) { 10 }", expected: Object::Null},
            // VmTestCase{input: "if (false) { 10 }", expected: Object::Null},
            // VmTestCase{input: "if ((if (false) { 10 })) { 10 } else { 20 }", expected: Object::Integer(20)},
        ];

        run_vm_tests(tests);
    }
}
