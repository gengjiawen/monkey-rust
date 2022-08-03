use std::borrow::Borrow;
use std::rc::Rc;

use object::Object;

pub fn test_constants(expected: &Vec<Object>, actual: &Vec<Rc<Object>>) {
    assert_eq!(expected.len(), actual.len());
    for (exp, b_got) in expected.iter().zip(actual) {
        let got = b_got.borrow();
        match (exp, got) {
            (Object::Integer(exp_val), Object::Integer(got_val)) => {
                assert_eq!(exp_val, got_val, "integer not equal {} {}", exp_val, got_val);
            }
            (Object::Boolean(exp_val), Object::Boolean(got_val)) => {
                assert_eq!(exp_val, got_val, "boolean not equal {} {}", exp_val, got_val);
            }
            (Object::Null, Object::Null) => {}
            _ => {
                panic!("can't compare object types, expected {}, got {}", exp, got);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::op_code::make_instructions;
    use crate::op_code::Opcode::*;
    use parser::parse;

    use crate::compiler::Compiler;
    use crate::op_code::{concat_instructions, Instructions};

    #[derive(Debug, Clone)]
    struct CompilerTestCase<'a> {
        input: &'a str,
        expected_constants: Vec<Object>,
        expected_instructions: Vec<Instructions>,
    }

    fn run_compiler_test(tests: Vec<CompilerTestCase>) {
        for t in tests {
            let program = parse(t.input).unwrap();
            let mut compiler = Compiler::new();
            let bytecodes = compiler.compile(&program).unwrap();
            test_instructions(&t.expected_instructions, &bytecodes.instructions);
            test_constants(&t.expected_constants, &bytecodes.constants);
        }
    }

    fn test_instructions(expected: &Vec<Instructions>, actual: &Instructions) {
        let expected_ins = concat_instructions(expected);

        // assert_eq!(expected_ins.data.len(), actual.data.len(), "instructions length not right");

        for (&exp, got) in expected_ins.data.iter().zip(actual.data.clone()) {
            assert_eq!(
                exp,
                got,
                "instruction not equal\n actual  : {:?}\n expected: {}",
                actual.string(),
                expected_ins.string()
            );
        }
    }

    #[test]
    fn integer_arithmetic() {
        let tests = vec![
            CompilerTestCase {
                input: "1 + 2",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpAdd, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "1; 2",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpPop, &vec![1]),
                ],
            },
            CompilerTestCase {
                input: "1 - 2",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpSub, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "1 * 2",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpMul, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "2 / 1",
                expected_constants: vec![Object::Integer(2), Object::Integer(1)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpDiv, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "-1",
                expected_constants: vec![Object::Integer(1)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpMinus, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "!true",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instructions(OpTrue, &vec![0]),
                    make_instructions(OpBang, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }
    #[test]
    fn boolean_expression() {
        let tests = vec![
            CompilerTestCase {
                input: "true",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instructions(OpTrue, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "false",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instructions(OpFalse, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "1 > 2",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpGreaterThan, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "1 < 2",
                expected_constants: vec![Object::Integer(2), Object::Integer(1)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpGreaterThan, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "1 == 2",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpEqual, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "1 != 2",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpNotEqual, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "true == false",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instructions(OpTrue, &vec![0]),
                    make_instructions(OpFalse, &vec![0]),
                    make_instructions(OpEqual, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "true != false",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instructions(OpTrue, &vec![0]),
                    make_instructions(OpFalse, &vec![0]),
                    make_instructions(OpNotEqual, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }

    #[test]
    fn conditions_only_if() {
        let tests = vec![CompilerTestCase {
            input: "if (true) { 10 }; 3333;",
            expected_constants: vec![Object::Integer(10), Object::Integer(3333)],
            expected_instructions: vec![
                make_instructions(OpTrue, &vec![0]),
                make_instructions(OpJumpNotTruthy, &vec![10]),
                make_instructions(OpConst, &vec![0]),
                make_instructions(OpJump, &vec![11]),
                make_instructions(OpNull, &vec![0]),
                make_instructions(OpPop, &vec![0]),
                make_instructions(OpConst, &vec![1]),
                make_instructions(OpPop, &vec![0]),
            ],
        }];

        run_compiler_test(tests);
    }

    #[test]
    fn conditions_with_else() {
        let tests = vec![CompilerTestCase {
            input: "if (true) { 10 } else { 20 }; 3333;",
            expected_constants: vec![
                Object::Integer(10),
                Object::Integer(20),
                Object::Integer(3333),
            ],
            expected_instructions: vec![
                make_instructions(OpTrue, &vec![0]),
                make_instructions(OpJumpNotTruthy, &vec![10]),
                make_instructions(OpConst, &vec![0]),
                make_instructions(OpJump, &vec![13]),
                make_instructions(OpConst, &vec![1]),
                make_instructions(OpPop, &vec![0]),
                make_instructions(OpConst, &vec![2]),
                make_instructions(OpPop, &vec![0]),
            ],
        }];

        run_compiler_test(tests);
    }

    #[test]
    fn test_global_constants() {
        let tests = vec![
            CompilerTestCase {
                input: "let one = 1; let two = 2;",
                expected_constants: vec![Object::Integer(1), Object::Integer(2)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpSetGlobal, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpSetGlobal, &vec![1]),
                ],
            },
            CompilerTestCase {
                input: "let one = 1; one",
                expected_constants: vec![Object::Integer(1)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpSetGlobal, &vec![0]),
                    make_instructions(OpGetGlobal, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "let one = 1; let two = one; two",
                expected_constants: vec![Object::Integer(1)],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpSetGlobal, &vec![0]),
                    make_instructions(OpGetGlobal, &vec![0]),
                    make_instructions(OpSetGlobal, &vec![1]),
                    make_instructions(OpGetGlobal, &vec![1]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }
}
