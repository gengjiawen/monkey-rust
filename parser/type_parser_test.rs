#[cfg(test)]
mod tests {
    use crate::ast::{Expression, MethodKind, Node, Param, Statement, TypeAnnotation};
    use crate::parse;
    use lexer::token::Span;

    /// Parses one program and hands back its single statement.
    fn parse_statement(input: &str) -> Statement {
        let Node::Program(program) = parse(input).unwrap() else {
            panic!("expected a program for {}", input)
        };
        assert_eq!(program.body.len(), 1, "expected exactly one statement in {}", input);
        return program.body.into_iter().next().unwrap();
    }

    fn let_annotation(input: &str) -> TypeAnnotation {
        let Statement::Let(statement) = parse_statement(input) else {
            panic!("expected a let statement for {}", input)
        };
        return statement
            .type_annotation
            .unwrap_or_else(|| panic!("expected a type annotation in {}", input));
    }

    fn function(input: &str) -> (Vec<Param>, Option<TypeAnnotation>) {
        let Statement::Expr(Expression::FUNCTION(function)) = parse_statement(input) else {
            panic!("expected a function expression for {}", input)
        };
        return (function.params, function.return_type);
    }

    fn slice<'a>(input: &'a str, span: &Span) -> &'a str {
        return &input[span.start..span.end];
    }

    fn first_error(input: &str) -> String {
        let errors = parse(input).expect_err("expected a parse error");
        return errors[0].clone();
    }

    #[test]
    fn parse_type_forms() {
        // Every shape in the grammar, checked through the `Display` round trip.
        let cases = [
            ("let a: int = 1;", "int"),
            ("let a: bool = true;", "bool"),
            ("let a: string = \"x\";", "string"),
            ("let a: any = 1;", "any"),
            ("let a: null = 1;", "null"),
            ("let a: Point = 1;", "Point"),
            ("let a: int? = 1;", "int?"),
            ("let a: [int] = 1;", "[int]"),
            ("let a: [[int]] = 1;", "[[int]]"),
            ("let a: [int?] = 1;", "[int?]"),
            ("let a: [int]? = 1;", "[int]?"),
            ("let a: {string: int} = 1;", "{string: int}"),
            ("let a: {string: [int]} = 1;", "{string: [int]}"),
            ("let a: {string: int}? = 1;", "{string: int}?"),
            ("let a: fn(): int = 1;", "fn(): int"),
            ("let a: fn(int): int = 1;", "fn(int): int"),
            ("let a: fn(int, string): bool = 1;", "fn(int, string): bool"),
            ("let a: fn(fn(int): int, int): [string] = 1;", "fn(fn(int): int, int): [string]"),
            ("let a: (fn(int): int)? = 1;", "(fn(int): int)?"),
            // `?` binds to the return type, not to the whole function type.
            ("let a: fn(int): int? = 1;", "fn(int): int?"),
        ];

        for (input, expected) in cases {
            assert_eq!(let_annotation(input).to_string(), expected, "for input {}", input);
        }
    }

    #[test]
    fn type_display_round_trips() {
        // Rendering an annotation must produce source that parses back the same
        // way, which is what lets the prettier plugin print from the AST.
        let inputs = [
            "let a: int? = 1;",
            "let a: [int]? = 1;",
            "let a: {string: [int]} = 1;",
            "let a: fn(fn(int): int, int): [string] = 1;",
            "let a: (fn(int): int)? = 1;",
        ];

        for input in inputs {
            let printed = let_annotation(input).to_string();
            let reparsed = let_annotation(&format!("let a: {} = 1;", printed));
            assert_eq!(reparsed.to_string(), printed, "for input {}", input);
        }
    }

    #[test]
    fn type_spans_cover_their_source() {
        let cases = [
            "let a: int? = 1;",
            "let a: [int] = 1;",
            "let a: {string: [int]} = 1;",
            "let a: fn(fn(int): int, int): [string] = 1;",
            "let a: (fn(int): int)? = 1;",
        ];

        for input in cases {
            let annotation = let_annotation(input);
            let expected = input
                .trim_start_matches("let a: ")
                .trim_end_matches(" = 1;");
            assert_eq!(slice(input, annotation.span()), expected, "for input {}", input);
        }
    }

    #[test]
    fn nested_type_spans() {
        let input = "let a: {string: [int]} = 1;";
        let TypeAnnotation::Hash(hash) = let_annotation(input) else {
            panic!("expected a hash type")
        };
        assert_eq!(slice(input, hash.key.span()), "string");
        assert_eq!(slice(input, hash.value.span()), "[int]");

        let TypeAnnotation::Array(array) = *hash.value else { panic!("expected an array type") };
        assert_eq!(slice(input, array.element.span()), "int");
    }

    #[test]
    fn function_type_spans() {
        let input = "let a: fn(int, string): bool = 1;";
        let TypeAnnotation::Function(function) = let_annotation(input) else {
            panic!("expected a function type")
        };
        assert_eq!(slice(input, &function.span), "fn(int, string): bool");
        assert_eq!(slice(input, function.params[0].span()), "int");
        assert_eq!(slice(input, function.params[1].span()), "string");
        assert_eq!(slice(input, function.return_type.span()), "bool");
    }

    #[test]
    fn grouping_widens_the_span_without_adding_a_node() {
        let input = "let a: (int) = 1;";
        let annotation = let_annotation(input);
        // The parenthesised type is still a plain named type…
        assert!(matches!(annotation, TypeAnnotation::Named(_)));
        // …but its span covers the parentheses.
        assert_eq!(slice(input, annotation.span()), "(int)");
    }

    #[test]
    fn double_question_normalizes() {
        let input = "let a: int?? = 1;";
        let annotation = let_annotation(input);
        assert_eq!(annotation.to_string(), "int?");
        // Normalized to a single node, but the span still covers both `?`s.
        assert_eq!(slice(input, annotation.span()), "int??");
        let TypeAnnotation::Optional(optional) = annotation else {
            panic!("expected an optional type")
        };
        assert!(matches!(*optional.inner, TypeAnnotation::Named(_)));
    }

    #[test]
    fn parse_function_annotations() {
        let (params, return_type) = function("fn(a: int, b: [string]): bool { a; }");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].to_string(), "a: int");
        assert_eq!(params[1].to_string(), "b: [string]");
        assert_eq!(return_type.unwrap().to_string(), "bool");

        // Annotations are optional and may be mixed with bare parameters.
        let (params, return_type) = function("fn(a, b: int) { a; }");
        assert_eq!(params[0].to_string(), "a");
        assert_eq!(params[1].to_string(), "b: int");
        assert!(return_type.is_none());

        // An unannotated function keeps parsing exactly as it did before.
        let (params, return_type) = function("fn(a, b) { a; }");
        assert!(params.iter().all(|param| param.type_annotation.is_none()));
        assert!(return_type.is_none());
    }

    #[test]
    fn param_spans_cover_name_and_annotation() {
        let input = "fn(a: int, b) { a; }";
        let (params, _) = function(input);
        assert_eq!(slice(input, &params[0].span), "a: int");
        assert_eq!(slice(input, &params[1].span), "b");
        assert_eq!(slice(input, &params[0].identifier.span), "a");
    }

    #[test]
    fn annotated_function_display() {
        let cases = [
            ("fn(a: int): int { a; }", "fn (a: int): int { a }"),
            ("fn(a: int?, b: [string]) { a; }", "fn (a: int?, b: [string]) { a }"),
            (
                "let f: fn(int): int = fn(a: int): int { a; };",
                "let f: fn(int): int = fn f(a: int): int { a };",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(parse(input).unwrap().to_string(), expected, "for input {}", input);
        }
    }

    #[test]
    fn parse_class_method_annotations() {
        let input =
            "class Point { constructor(x: int, y: int) { this.x = x; } norm(): int { 1; } }";
        let Statement::Class(class) = parse_statement(input) else {
            panic!("expected a class declaration")
        };

        let constructor = &class.methods[0];
        assert_eq!(constructor.kind, MethodKind::Constructor);
        assert_eq!(constructor.params[0].to_string(), "x: int");
        assert!(constructor.return_type.is_none());

        let method = &class.methods[1];
        assert_eq!(method.kind, MethodKind::Method);
        assert_eq!(method.return_type.as_ref().unwrap().to_string(), "int");
    }

    #[test]
    fn type_names_stay_soft_keywords() {
        // `int`, `bool`, `string`, `any` and `null` are only types in type
        // position; everywhere else they stay ordinary identifiers.
        let cases = [
            ("let int = 5;", "let int = 5;"),
            ("let string = \"x\";", "let string = \"x\";"),
            ("let any = fn(any) { any; };", "let any = fn any(any) { any };"),
            ("let null: int = 5;", "let null: int = 5;"),
            ("let int: int = int;", "let int: int = int;"),
        ];

        for (input, expected) in cases {
            assert_eq!(parse(input).unwrap().to_string(), expected, "for input {}", input);
        }
    }

    #[test]
    fn reject_malformed_types() {
        let cases = [
            ("let x: = 5;", "expected a type, got"),
            ("fn(a:) { a; }", "expected a type, got"),
            ("let x: [int = 5;", "expected token: ]"),
            ("let x: {int} = 5;", "expected token: :"),
            ("let x: {int: } = 5;", "expected a type, got"),
            ("let x: fn(int) = 5;", "function type requires a return type"),
            ("let x: fn(int: int): int = 5;", "expected token: )"),
            ("let x: (int = 5;", "expected token: )"),
            ("let x: 5 = 5;", "expected a type, got"),
            (
                "class P { constructor(): int { 1; } }",
                "constructor cannot have a return type annotation",
            ),
        ];

        for (input, expected) in cases {
            let error = first_error(input);
            assert!(
                error.contains(expected),
                "expected {} to report {:?}, got {:?}",
                input,
                expected,
                error
            );
        }
    }

    #[test]
    fn question_mark_outside_type_position_is_rejected() {
        // `?` has no expression-level meaning; it must not silently parse.
        assert!(parse("1 ? 2 : 3").is_err());
    }
}
