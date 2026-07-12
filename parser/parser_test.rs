#[cfg(test)]
mod tests {
    use crate::ast::{Expression, MethodKind, Node, Statement};
    use crate::parse;

    fn verify_program(test_cases: &[(&str, &str)]) {
        for (input, expected) in test_cases {
            let ast = parse(input).unwrap();
            let parsed = ast.to_string();
            assert_eq!(&format!("{}", parsed), expected);
        }
    }

    #[test]
    fn parse_let_statement() {
        let let_tests = [
            ("let x=5;", "let x = 5;"),
            ("let y=true;", "let y = true;"),
            ("let foo=y;", "let foo = y;"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_return_statement() {
        let let_tests = [
            ("return 5", "return 5;"),
            ("return true;", "return true;"),
            ("return foobar;", "return foobar;"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn test_parse_prefix_expression() {
        let let_tests = [
            ("-15;", "(-15)"),
            ("!5;", "(!5)"),
            ("!foobar;", "(!foobar)"),
            ("-foobar;", "(-foobar)"),
            ("!true;", "(!true)"),
            ("!false;", "(!false)"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn test_parse_infix_expression() {
        let let_tests = [
            ("5 + 5;", "(5 + 5)"),
            ("5 - 5;", "(5 - 5)"),
            ("5 * 5;", "(5 * 5)"),
            ("5 / 5;", "(5 / 5)"),
            ("5 > 5;", "(5 > 5)"),
            ("5 < 5;", "(5 < 5)"),
            ("5 == 5;", "(5 == 5)"),
            ("5 != 5;", "(5 != 5)"),
            ("foobar + barfoo;", "(foobar + barfoo)"),
            ("foobar - barfoo;", "(foobar - barfoo)"),
            ("foobar * barfoo;", "(foobar * barfoo)"),
            ("foobar / barfoo;", "(foobar / barfoo)"),
            ("foobar > barfoo;", "(foobar > barfoo)"),
            ("foobar < barfoo;", "(foobar < barfoo)"),
            ("foobar == barfoo;", "(foobar == barfoo)"),
            ("foobar != barfoo;", "(foobar != barfoo)"),
            ("true == true", "(true == true)"),
            ("true != false", "(true != false)"),
            ("false == false", "(false == false)"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_op_expression() {
        let tt = [
            ("-a * b", "((-a) * b)"),
            ("!-a", "(!(-a))"),
            ("a + b + c", "((a + b) + c)"),
            ("a + b - c", "((a + b) - c)"),
            ("a * b * c", "((a * b) * c)"),
            ("a * b / c", "((a * b) / c)"),
            ("a + b / c", "(a + (b / c))"),
            ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
            ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
            ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
            ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4))"),
            ("3 + 4 * 5 == 3 * 1 + 4 * 5", "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))"),
            ("true", "true"),
            ("false", "false"),
            ("3 > 5 == false", "((3 > 5) == false)"),
            ("3 < 5 == true", "((3 < 5) == true)"),
        ];

        verify_program(&tt);
    }

    #[test]
    fn parse_brace_expression() {
        let tt = [
            ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
            ("(5 + 5) * 2", "((5 + 5) * 2)"),
            ("2 / (5 + 5)", "(2 / (5 + 5))"),
            ("(5 + 5) * 2 * (5 + 5)", "(((5 + 5) * 2) * (5 + 5))"),
            ("-(5 + 5)", "(-(5 + 5))"),
            ("!(true == true)", "(!(true == true))"),
        ];

        verify_program(&tt);
    }

    #[test]
    fn test_if_expression() {
        let tt = [("if (x < y) { x }", "if (x < y) { x }")];
        verify_program(&tt);
    }

    #[test]
    fn test_if_else_expression() {
        let tt = [("if (x < y) { x } else { y }", "if (x < y) { x } else { y }")];
        verify_program(&tt);
    }

    #[test]
    fn test_fn_else_expression() {
        let tt = [
            ("fn() {};", "fn () {  }"),
            ("fn(x) {};", "fn (x) {  }"),
            ("fn(x, y, z) { x };", "fn (x, y, z) { x }"),
        ];
        verify_program(&tt);
    }

    #[test]
    fn test_fn_call_else_expression() {
        let tt = [("add(1, 2 * 3, 4 + 5);", "add(1, (2 * 3), (4 + 5))")];
        verify_program(&tt);
    }

    #[test]
    fn test_string_literal_expression() {
        let test_case = [(r#""hello world";"#, r#""hello world""#)];
        verify_program(&test_case);
    }

    #[test]
    fn test_array_literal_expression() {
        let test_case = [("[]", "[]"), ("[1, 2 * 2, 3 + 3]", "[1, (2 * 2), (3 + 3)]")];
        verify_program(&test_case);
    }

    #[test]
    fn test_index_expression() {
        let test_case = [("a[1]", "(a[1])"), ("a[1 + 1]", "(a[(1 + 1)])")];
        verify_program(&test_case);
    }

    #[test]
    fn test_hash_literal_expression() {
        let test_case = [
            (r#"{"a": 1}"#, r#"{"a": 1}"#),
            (r#"{"one": 1, "two": 2, "three": 3}"#, r#"{"one": 1, "two": 2, "three": 3}"#),
            (r#"{}"#, r#"{}"#),
            (
                r#"{"one": 0 + 1, "two": 10 - 8, "three": 15 / 5}"#,
                r#"{"one": (0 + 1), "two": (10 - 8), "three": (15 / 5)}"#,
            ),
        ];
        verify_program(&test_case);
    }

    #[test]
    fn parses_class_new_property_and_bound_method_syntax() {
        let input = r#"class Node {
  constructor(value) { this.value = value; }
  connect(other) { this.next = other; }
}
let node = new Node(1);
node.connect(node);
let connect = node.connect;
connect();"#;
        let Node::Program(program) = parse(input).unwrap() else { panic!("expected program") };
        let Statement::Class(class) = &program.body[0] else {
            panic!("expected class declaration")
        };

        assert_eq!(class.name.name, "Node");
        assert_eq!(class.methods.len(), 2);
        assert_eq!(class.methods[0].kind, MethodKind::Constructor);
        assert_eq!(class.methods[1].name.name, "connect");
        assert_eq!(&input[class.span.start..class.span.end], &input[..class.span.end]);

        let Statement::Let(node) = &program.body[1] else { panic!("expected node binding") };
        assert!(matches!(node.expr, Expression::New(_)));
        let Statement::Expr(Expression::FunctionCall(call)) = &program.body[2] else {
            panic!("expected method call")
        };
        assert!(matches!(*call.callee, Expression::Property(_)));
    }

    #[test]
    fn parses_property_set_as_statement() {
        let input = "node.next.value = new Node(1);";
        let Node::Program(program) = parse(input).unwrap() else { panic!("expected program") };
        let Statement::SetProperty(set) = &program.body[0] else { panic!("expected property set") };
        assert_eq!(set.property.name, "value");
        assert!(matches!(*set.object, Expression::Property(_)));
        assert_eq!(&input[set.span.start..set.span.end], input);
    }

    #[test]
    fn postfix_spans_include_grouping_without_expanding_inner_nodes() {
        for input in ["(a + b).value", "((a)).value"] {
            let Node::Program(program) = parse(input).unwrap() else { panic!("expected program") };
            let Statement::Expr(Expression::Property(property)) = &program.body[0] else {
                panic!("expected property")
            };
            assert_eq!(&input[property.span.start..property.span.end], input);
        }

        let input = "(fn() { 1 })()";
        let Node::Program(program) = parse(input).unwrap() else { panic!("expected program") };
        let Statement::Expr(Expression::FunctionCall(call)) = &program.body[0] else {
            panic!("expected call")
        };
        assert_eq!(&input[call.span.start..call.span.end], input);
    }

    #[test]
    fn rejects_invalid_class_and_assignment_forms_without_panicking() {
        for (input, expected) in [
            ("class A { constructor() {} constructor() {} }", "more than one constructor"),
            ("class A { method() {} method() {} }", "duplicate method"),
            ("class A { let value = 1; }", "expected method definition"),
            ("fn() { class A {} }", "only allowed at top level"),
            ("new A", "requires an argument list"),
            ("value = 1", "only instance property assignment"),
            ("let value = object.field = 1", "only allowed as a statement"),
            ("1 + ;", "no prefix function"),
            ("fn() { 1 + ; }", "no prefix function"),
        ] {
            let errors = parse(input).unwrap_err();
            assert!(
                errors.iter().any(|error| error.contains(expected)),
                "{:?}: expected {:?}, got {:?}",
                input,
                expected,
                errors
            );
        }
    }
}
