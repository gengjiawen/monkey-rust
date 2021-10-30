use object::Object;

struct VmTestCase<'a> {
    input: &'a str,
    expected: Object,
}


#[cfg(test)]
mod tests {
    use std::fmt::Debug;
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

    #[test]
    fn test_integer_arithmetic() {
        let tests = vec![
            // VmTestCase { input: "1", expected: Object::Integer(1) },
            // VmTestCase { input: "2", expected: Object::Integer(2) },
            VmTestCase { input: "1 + 2", expected: Object::Integer(3) },
        ];

        run_vm_tests(tests);
    }

    fn run_vm_tests(tests: Vec<VmTestCase>) {
        for t in tests {
            let program = parse(t.input).unwrap();
            let mut compiler = Compiler::new();
            let bytecodes = compiler.compile(&program).unwrap();
            println!("ins {} for input {}", bytecodes.instructions.string(), t.input);
            let mut vm = VM::new(bytecodes);
            vm.run();
            let got = vm.stack_top().unwrap();
            test_expected_object(t.expected, got);
        }
    }
}
