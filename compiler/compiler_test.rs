use crate::compiler::Compiler;
use crate::op_code::{concat_instructions, Instructions};
use parser::parse;
use std::borrow::Borrow;
use std::rc::Rc;

use object::Object;

pub fn test_constants(expected: &Vec<Object>, actual: &Vec<Rc<Object>>) {
    assert_eq!(expected.len(), actual.len());
    for (exp, b_got) in expected.iter().zip(actual) {
        let got = b_got.borrow();
        assert_eq!(exp, got);
    }
}

#[derive(Debug, Clone)]
pub struct CompilerTestCase<'a> {
    pub(crate) input: &'a str,
    pub(crate) expected_constants: Vec<Object>,
    pub(crate) expected_instructions: Vec<Instructions>,
}

pub fn run_compiler_test(tests: Vec<CompilerTestCase>) {
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

    assert_eq!(
        expected_ins.data.len(),
        actual.data.len(),
        "instructions length not right\n actual  : \n{}\n expected: \n{}",
        actual.string(),
        expected_ins.string()
    );

    for (&exp, got) in expected_ins.data.iter().zip(actual.data.clone()) {
        assert_eq!(
            exp,
            got,
            "instruction not equal\n actual  : \n{}\n expected: \n{}",
            actual.string(),
            expected_ins.string()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op_code::make_instructions;
    use crate::op_code::Opcode::*;
    use parser::lexer::token::Span;

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
    fn bytecode_string_includes_compiled_function_constants() {
        let program = parse("let add = fn(a, b) { a + b };").unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();
        let output = bytecode.string();

        assert!(output.contains("Instructions:\n0000 OpClosure 0 0\n0004 OpSetGlobal 0\n"));
        assert!(
            output.contains("Constants:\n0000 CompiledFunction(num_locals=2, num_parameters=2)\n")
        );
        assert!(output.contains("       0000 OpGetLocal 0\n"));
        assert!(output.contains("       0002 OpGetLocal 1\n"));
        assert!(output.contains("       0004 OpAdd\n"));
        assert!(output.contains("       0005 OpReturnValue\n"));
    }

    #[test]
    fn bytecode_tracks_pc_to_source_spans() {
        let program = parse("1;\n22").unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();

        assert_eq!(
            bytecode.debug_info.pc_spans,
            vec![
                crate::compiler::PcSpan {
                    pc: 0,
                    span: Span {
                        start: 0,
                        end: 1
                    }
                },
                crate::compiler::PcSpan {
                    pc: 4,
                    span: Span {
                        start: 3,
                        end: 5
                    }
                },
            ]
        );
        assert_eq!(
            bytecode.debug_info.span_for_pc(3),
            Some(&Span {
                start: 0,
                end: 1
            })
        );
        assert_eq!(
            bytecode.debug_info.span_for_pc(7),
            Some(&Span {
                start: 3,
                end: 5
            })
        );
    }

    #[test]
    fn bytecode_tracks_function_constant_pc_to_source_spans() {
        let input = "let add = fn(a, b) { a + b; };";
        let program = parse(input).unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();

        let expression_start = input.find("a + b").unwrap();
        let function_debug_info = bytecode.function_debug_info.get(&0).unwrap();

        assert_eq!(
            function_debug_info.span_for_pc(4),
            Some(&Span {
                start: expression_start,
                end: expression_start + "a + b".len()
            })
        );
    }

    #[test]
    fn bytecode_debug_view_maps_instruction_lines_to_pc() {
        let input = "1;\n22";
        let program = parse(input).unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();
        let view = bytecode.debug_view();

        assert_eq!(view.detail, bytecode.string());
        assert_eq!(
            view.instruction_lines,
            vec![
                crate::compiler::InstructionLineMapping {
                    line: 1,
                    pc: 0,
                    scope: crate::compiler::InstructionScope::Main,
                },
                crate::compiler::InstructionLineMapping {
                    line: 2,
                    pc: 3,
                    scope: crate::compiler::InstructionScope::Main,
                },
                crate::compiler::InstructionLineMapping {
                    line: 3,
                    pc: 4,
                    scope: crate::compiler::InstructionScope::Main,
                },
                crate::compiler::InstructionLineMapping {
                    line: 4,
                    pc: 7,
                    scope: crate::compiler::InstructionScope::Main,
                },
            ]
        );
    }

    #[test]
    fn bytecode_debug_view_maps_function_instruction_lines() {
        let input = "let add = fn(a, b) { a + b; };";
        let program = parse(input).unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();
        let view = bytecode.debug_view();

        let add_line = view
            .instruction_lines
            .iter()
            .find(|line| {
                matches!(
                    line.scope,
                    crate::compiler::InstructionScope::Function {
                        constant_index: 0
                    }
                ) && line.pc == 4
            })
            .expect("OpAdd instruction line");

        let expression_start = input.find("a + b").unwrap();
        let function_debug_info = view.function_debug_info.get(&0).unwrap();

        assert_eq!(add_line.line > 0, true);
        assert_eq!(
            function_debug_info.span_for_pc(add_line.pc),
            Some(&Span {
                start: expression_start,
                end: expression_start + "a + b".len(),
            })
        );
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

    #[test]
    fn test_string() {
        let tests = vec![
            CompilerTestCase {
                input: "\"monkey\"",
                expected_constants: vec![Object::String("monkey".to_string())],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: r#""mon" + "key""#,
                expected_constants: vec![
                    Object::String("mon".to_string()),
                    Object::String("key".to_string()),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpAdd, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }

    #[test]
    fn test_array() {
        let tests = vec![
            CompilerTestCase {
                input: "[]",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instructions(OpArray, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "[1, 2, 3]",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                    Object::Integer(3),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpConst, &vec![2]),
                    make_instructions(OpArray, &vec![3]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "[1 + 2, 3 - 4, 5 * 6]",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                    Object::Integer(3),
                    Object::Integer(4),
                    Object::Integer(5),
                    Object::Integer(6),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpAdd, &vec![0]),
                    make_instructions(OpConst, &vec![2]),
                    make_instructions(OpConst, &vec![3]),
                    make_instructions(OpSub, &vec![0]),
                    make_instructions(OpConst, &vec![4]),
                    make_instructions(OpConst, &vec![5]),
                    make_instructions(OpMul, &vec![0]),
                    make_instructions(OpArray, &vec![3]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }

    #[test]
    fn test_hashmap() {
        let tests = vec![
            CompilerTestCase {
                input: "{}",
                expected_constants: vec![],
                expected_instructions: vec![
                    make_instructions(OpHash, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "{1: 2, 3: 4, 5: 6}",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                    Object::Integer(3),
                    Object::Integer(4),
                    Object::Integer(5),
                    Object::Integer(6),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpConst, &vec![2]),
                    make_instructions(OpConst, &vec![3]),
                    make_instructions(OpConst, &vec![4]),
                    make_instructions(OpConst, &vec![5]),
                    make_instructions(OpHash, &vec![6]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "{1: 2 + 3, 4: 5 * 6}",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                    Object::Integer(3),
                    Object::Integer(4),
                    Object::Integer(5),
                    Object::Integer(6),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpConst, &vec![2]),
                    make_instructions(OpAdd, &vec![0]),
                    make_instructions(OpConst, &vec![3]),
                    make_instructions(OpConst, &vec![4]),
                    make_instructions(OpConst, &vec![5]),
                    make_instructions(OpMul, &vec![0]),
                    make_instructions(OpHash, &vec![4]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }

    #[test]
    fn test_index() {
        let tests = vec![
            CompilerTestCase {
                input: "[1, 2, 3][1 + 1]",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                    Object::Integer(3),
                    Object::Integer(1),
                    Object::Integer(1),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpConst, &vec![2]),
                    make_instructions(OpArray, &vec![3]),
                    make_instructions(OpConst, &vec![3]),
                    make_instructions(OpConst, &vec![4]),
                    make_instructions(OpAdd, &vec![0]),
                    make_instructions(OpIndex, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
            CompilerTestCase {
                input: "{1: 2 }[2 -1]",
                expected_constants: vec![
                    Object::Integer(1),
                    Object::Integer(2),
                    Object::Integer(2),
                    Object::Integer(1),
                ],
                expected_instructions: vec![
                    make_instructions(OpConst, &vec![0]),
                    make_instructions(OpConst, &vec![1]),
                    make_instructions(OpHash, &vec![2]),
                    make_instructions(OpConst, &vec![2]),
                    make_instructions(OpConst, &vec![3]),
                    make_instructions(OpSub, &vec![0]),
                    make_instructions(OpIndex, &vec![0]),
                    make_instructions(OpPop, &vec![0]),
                ],
            },
        ];

        run_compiler_test(tests);
    }

    #[test]
    fn compiles_class_constructor_properties_and_new_exactly() {
        let constructor = Object::CompiledFunction(Rc::new(object::CompiledFunction {
            name: "Point.constructor".to_string(),
            instructions: concat_instructions(&vec![
                make_instructions(OpGetLocal, &vec![0]),
                make_instructions(OpGetLocal, &vec![1]),
                make_instructions(OpSetProperty, &vec![1]),
                make_instructions(OpNull, &vec![]),
                make_instructions(OpPop, &vec![]),
                make_instructions(OpGetLocal, &vec![0]),
                make_instructions(OpReturnValue, &vec![]),
            ])
            .data,
            num_locals: 2,
            num_parameters: 2,
        }));
        let value_method = Object::CompiledFunction(Rc::new(object::CompiledFunction {
            name: "Point.value".to_string(),
            instructions: concat_instructions(&vec![
                make_instructions(OpGetLocal, &vec![0]),
                make_instructions(OpGetProperty, &vec![4]),
                make_instructions(OpReturnValue, &vec![]),
            ])
            .data,
            num_locals: 1,
            num_parameters: 1,
        }));

        run_compiler_test(vec![CompilerTestCase {
            input: "class Point { constructor(x) { this.value = x; } value() { this.value; } } let point = new Point(1); point.value = 2; point.value;",
            expected_constants: vec![
                Object::String("Point".to_string()),
                Object::String("value".to_string()),
                constructor,
                Object::String("constructor".to_string()),
                Object::String("value".to_string()),
                value_method,
                Object::String("value".to_string()),
                Object::Integer(1),
                Object::Integer(2),
                Object::String("value".to_string()),
                Object::String("value".to_string()),
            ],
            expected_instructions: vec![
                make_instructions(OpClass, &vec![0]),
                make_instructions(OpClosure, &vec![2, 0]),
                make_instructions(OpMethod, &vec![3, 1]),
                make_instructions(OpClosure, &vec![5, 0]),
                make_instructions(OpMethod, &vec![6, 0]),
                make_instructions(OpSetGlobal, &vec![0]),
                make_instructions(OpNull, &vec![]),
                make_instructions(OpPop, &vec![]),
                make_instructions(OpGetGlobal, &vec![0]),
                make_instructions(OpConst, &vec![7]),
                make_instructions(OpNew, &vec![1]),
                make_instructions(OpSetGlobal, &vec![1]),
                make_instructions(OpGetGlobal, &vec![1]),
                make_instructions(OpConst, &vec![8]),
                make_instructions(OpSetProperty, &vec![9]),
                make_instructions(OpNull, &vec![]),
                make_instructions(OpPop, &vec![]),
                make_instructions(OpGetGlobal, &vec![1]),
                make_instructions(OpGetProperty, &vec![10]),
                make_instructions(OpPop, &vec![]),
            ],
        }]);
    }

    #[test]
    fn compiles_this_through_each_nested_closure_scope() {
        let program = parse("class Box { reader() { fn() { fn() { this.value; }; }; } }").unwrap();
        let mut compiler = Compiler::new();
        let bytecode = compiler.compile(&program).unwrap();

        let compiled = |index: usize| match bytecode.constants[index].as_ref() {
            Object::CompiledFunction(function) => function,
            value => panic!("constant {} should be a compiled function, got {:?}", index, value),
        };

        assert_eq!(
            compiled(2).instructions,
            concat_instructions(&vec![
                make_instructions(OpGetFree, &vec![0]),
                make_instructions(OpGetProperty, &vec![1]),
                make_instructions(OpReturnValue, &vec![]),
            ])
            .data
        );
        assert_eq!(
            compiled(3).instructions,
            concat_instructions(&vec![
                make_instructions(OpGetFree, &vec![0]),
                make_instructions(OpClosure, &vec![2, 1]),
                make_instructions(OpReturnValue, &vec![]),
            ])
            .data
        );
        assert_eq!(
            compiled(4).instructions,
            concat_instructions(&vec![
                make_instructions(OpGetLocal, &vec![0]),
                make_instructions(OpClosure, &vec![3, 1]),
                make_instructions(OpReturnValue, &vec![]),
            ])
            .data
        );
        assert_eq!((compiled(4).num_locals, compiled(4).num_parameters), (1, 1));
    }

    #[test]
    fn validation_accepts_globals_from_previous_compiler_state() {
        let mut first = Compiler::new();
        first.compile(&parse("let answer = 41;").unwrap()).unwrap();

        let mut next = Compiler::new_with_state(first.symbol_table, first.constants);
        next.compile(&parse("answer + 1;").unwrap()).unwrap();
    }
}
