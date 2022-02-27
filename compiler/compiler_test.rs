use std::borrow::Borrow;
use std::rc::Rc;

use object::Object;
use parser::parse;

use crate::compiler::Compiler;
use crate::op_code::Instructions;

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

pub fn test_constants(expected: &Vec<Object>, actual: &Vec<Rc<Object>>) {
    assert_eq!(expected.len(), actual.len());
    for (exp, b_got) in expected.iter().zip(actual) {
        let got = b_got.borrow();
        match (exp, got) {
            (Object::Integer(exp_val), Object::Integer(got_val)) => {
                assert_eq!(exp_val, got_val, "integer not equal {} {}", exp_val, got_val);
            },
            (Object::Boolean(exp_val), Object::Boolean(got_val)) => {
                assert_eq!(exp_val, got_val, "boolean not equal {} {}", exp_val, got_val);
            },
            _ => {
                panic!("can't compare object types");
            }
        }
    }
}

fn test_instructions(expected: &Vec<Instructions>, actual: &Instructions) {
    let concatted = concat_instructions(expected);

    assert_eq!(concatted.data.len(), actual.data.len());

    for (exp, got) in concatted.data.into_iter().zip(actual.data.clone()) {
        assert_eq!(exp, got)
    }
}

fn concat_instructions(expected: &Vec<Instructions>) -> Instructions {
    let mut out = Instructions {
        data: vec![],
    };

    for instruction in expected {
        out = out.merge_instructions(instruction)
    }

    return out;
}

#[cfg(test)]
mod tests {
    use crate::compiler::*;
    use crate::op_code::make_instructions;
    use crate::op_code::Opcode::*;

    use super::*;

    #[test]
    fn integer_arithmetic() {
        let tests = vec![
            CompilerTestCase {
                input: "1 + 2",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpAdd, &vec![1]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "1; 2",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpPop, &vec![1]) },
                ],
            },
            CompilerTestCase {
                input: "1 - 2",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpSub, &vec![1]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "1 * 2",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpMul, &vec![1]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "2 / 1",
                expected_constants: vec![
                    Object::Integer(2),
                    Object::Integer(1),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpDiv, &vec![1]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "-1",
                expected_constants: vec![
                    Object::Integer(1),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpMinus, &vec![1]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "!true",
                expected_constants: vec![
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpTrue, &vec![0]) },
                    Instructions { data: make_instructions(OpBang, &vec![1]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
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
                expected_constants: vec![
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpTrue, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "false",
                expected_constants: vec![
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpFalse, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "1 > 2",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpGreaterThan, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "1 < 2",
                expected_constants: vec![
                    Object::Integer(2),
                    Object::Integer(1),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpGreaterThan, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "1 == 2",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpEqual, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "1 != 2",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpConst, &vec![0]) },
                    Instructions { data: make_instructions(OpConst, &vec![1]) },
                    Instructions { data: make_instructions(OpNotEqual, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "true == false",
                expected_constants: vec![
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpTrue, &vec![0]) },
                    Instructions { data: make_instructions(OpFalse, &vec![0]) },
                    Instructions { data: make_instructions(OpEqual, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
            CompilerTestCase {
                input: "true != false",
                expected_constants: vec![
                ],
                expected_instructions: vec![
                    Instructions { data: make_instructions(OpTrue, &vec![0]) },
                    Instructions { data: make_instructions(OpFalse, &vec![0]) },
                    Instructions { data: make_instructions(OpNotEqual, &vec![0]) },
                    Instructions { data: make_instructions(OpPop, &vec![0]) },
                ],
            },
        ];

        run_compiler_test(tests);
    }
}

