#[cfg(test)]
mod tests {
    use crate::compiler_test::{run_compiler_test, CompilerTestCase};
    use crate::op_code::Opcode::*;
    use crate::op_code::{concat_instructions, make_instructions};
    use object::Object;
    use std::rc::Rc;

    #[test]
    fn test_functions() {
        let tests = vec![
            CompilerTestCase {
                input: "fn() { return 5 + 10; }",
                expected_constants: vec![
                    Object::Integer(5),
                    Object::Integer(10),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpConst, &vec![0]),
                            make_instructions(OpConst, &vec![1]),
                            make_instructions(OpAdd, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 0,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![2, 0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "fn() { 5 + 10; }",
                expected_constants: vec![
                    Object::Integer(5),
                    Object::Integer(10),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpConst, &vec![0]),
                            make_instructions(OpConst, &vec![1]),
                            make_instructions(OpAdd, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 0,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![2, 0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "fn() { 1; 2}",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpConst, &vec![0]),
                            make_instructions(OpPop, &vec![0]),
                            make_instructions(OpConst, &vec![1]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 0,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![2, 0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];
        run_compiler_test(tests);
    }

    #[test]
    fn test_function_without_return_value() {
        let tests = vec![CompilerTestCase {
            input: "fn() { }",
            expected_constants: vec![Object::CompiledFunction(Rc::from(
                object::CompiledFunction {
                    instructions: concat_instructions(&vec![make_instructions(OpReturn, &vec![0])])
                        .data,
                    num_locals: 0,
                    num_parameters: 0,
                },
            ))],
            expected_instructions: vec![
                make_instructions(OpClosure, &vec![0, 0]),
                make_instructions(OpPop, &vec![0]),
            ],
        }];
        run_compiler_test(tests);
    }

    #[test]
    fn test_function_calls() {
        let tests = vec![
            CompilerTestCase {
                input: "fn() { 24 }();",
                expected_constants: vec![
                    Object::Integer(24),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpConst, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 0,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![1, 0]),
                    make_instructions(OpCall, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "let noArg = fn() { 24; }; noArg();",
                expected_constants: vec![
                    Object::Integer(24),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpConst, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 0,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![1, 0]),
                    make_instructions(OpSetGlobal, &vec![0]),
                    make_instructions(OpGetGlobal, &vec![0]),
                    make_instructions(OpCall, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "let oneArg = fn(a) { a; }; oneArg(24);",
                expected_constants: vec![
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpGetLocal, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 1,
                        num_parameters: 1,
                    })),
                    Object::Integer(24),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![0, 0]),
                    make_instructions(OpSetGlobal, &vec![0]),
                    make_instructions(OpGetGlobal, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpCall, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "let manyArg = fn(a, b, c) { a; b; c; }; manyArg(24, 25, 26);",
                expected_constants: vec![
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpGetLocal, &vec![0]),
                            make_instructions(OpPop, &vec![0]),
                            make_instructions(OpGetLocal, &vec![1]),
                            make_instructions(OpPop, &vec![0]),
                            make_instructions(OpGetLocal, &vec![2]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 3,
                        num_parameters: 3,
                    })),
                    Object::Integer(24),
                    Object::Integer(25),
                    Object::Integer(26),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![0, 0]),
                    make_instructions(OpSetGlobal, &vec![0]),
                    make_instructions(OpGetGlobal, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpConst, &vec![2]),
                    make_instructions(OpConst, &vec![3]),
                    make_instructions(OpCall, &vec![3]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }

    #[test]
    fn test_let_statement_scope() {
        let tests = vec![
            CompilerTestCase {
                input: "let num = 55; fn() { num; }",
                expected_constants: vec![
                    Object::Integer(55),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpGetGlobal, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 0,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpSetGlobal, &vec![0]),
                    make_instructions(OpClosure, &vec![1, 0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "fn() { let num = 55; num; }",
                expected_constants: vec![
                    Object::Integer(55),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpConst, &vec![0]),
                            make_instructions(OpSetLocal, &vec![0]),
                            make_instructions(OpGetLocal, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 1,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![1, 0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "fn() { let a = 55; let b = 77; a + b; }",
                expected_constants: vec![
                    Object::Integer(55),
                    Object::Integer(77),
                    Object::CompiledFunction(Rc::from(object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpConst, &vec![0]),
                            make_instructions(OpSetLocal, &vec![0]),
                            make_instructions(OpConst, &vec![1]),
                            make_instructions(OpSetLocal, &vec![1]),
                            make_instructions(OpGetLocal, &vec![0]),
                            make_instructions(OpGetLocal, &vec![1]),
                            make_instructions(OpAdd, &vec![0]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 2,
                        num_parameters: 0,
                    })),
                ],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![2, 0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];
        run_compiler_test(tests);
    }

    #[test]
    fn test_builtins() {
        let tests = vec![
            CompilerTestCase {
                input: "len([]); push([], 1);",
                expected_constants: vec![Object::Integer(1)],
                expected_instructions: vec![
                    make_instructions(OpGetBuiltin, &vec![0]),
                    make_instructions(OpArray, &vec![0]),
                    make_instructions(OpCall, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                    make_instructions(OpGetBuiltin, &vec![5]),
                    make_instructions(OpArray, &vec![0]),
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpCall, &vec![2]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "fn() { len([]) }",
                expected_constants: vec![Object::CompiledFunction(Rc::from(
                    object::CompiledFunction {
                        instructions: concat_instructions(&vec![
                            make_instructions(OpGetBuiltin, &vec![0]),
                            make_instructions(OpArray, &vec![0]),
                            make_instructions(OpCall, &vec![1]),
                            make_instructions(OpReturnValue, &vec![0]),
                        ])
                        .data,
                        num_locals: 0,
                        num_parameters: 0,
                    },
                ))],
                expected_instructions: vec![
                    make_instructions(OpClosure, &vec![0, 0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];
        run_compiler_test(tests);
    }
}
