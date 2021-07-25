#[cfg(test)]
mod tests {
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
            (
                "3 + 4 * 5 == 3 * 1 + 4 * 5",
                "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
            ),
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
            ("fn() {};", "fn() {  }"),
            ("fn(x) {};", "fn(x) {  }"),
            ("fn(x, y, z) { x };", "fn(x, y, z) { x }"),
        ];
        verify_program(&tt);
    }

    #[test]
    fn test_fn_call_else_expression() {
        let tt = [
            ("add(1, 2 * 3, 4 + 5);", "add(1, (2 * 3), (4 + 5))")
        ];
        verify_program(&tt);
    }

    #[test]
    fn test_string_literal_expression() {
        let test_case = [(r#""hello world";"#, r#""hello world""#)];
        verify_program(&test_case);
    }

    #[test]
    fn test_array_literal_expression() {
        let test_case = [
            ("[]", "[]"),
            ("[1, 2 * 2, 3 + 3]", "[1, (2 * 2), (3 + 3)]")
        ];
        verify_program(&test_case);
    }

    #[test]
    fn test_index_expression() {
        let test_case = [
            ("a[1]", "(a[1])"),
            ("a[1 + 1]", "(a[(1 + 1)])")
        ];
        verify_program(&test_case);
    }

    #[test]
    fn test_hash_literal_expression() {
        let test_case = [
            (
                r#"{"a": 1}"#,
                r#"{"a": 1}"#,
            ),
            (
                r#"{"one": 1, "two": 2, "three": 3}"#,
                r#"{"one": 1, "two": 2, "three": 3}"#,
            ),
            (r#"{}"#, r#"{}"#),
            (
                r#"{"one": 0 + 1, "two": 10 - 8, "three": 15 / 5}"#,
                r#"{"one": (0 + 1), "two": (10 - 8), "three": (15 / 5)}"#,
            ),
        ];
        verify_program(&test_case);
    }
}
