#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use object::environment::*;
    use parser::*;

    use crate::eval;

    fn apply_test(test_cases: &[(&str, &str)]) {
        let env: Env = Rc::new(RefCell::new(Default::default()));
        for (input, expected) in test_cases {
            match parse(input) {
                Ok(node) => match eval(node, &env) {
                    Ok(evaluated) => assert_eq!(&format!("{}", evaluated), expected),
                    Err(e) => assert_eq!(&e.to_string(), expected),
                },
                Err(e) => panic!("parse error: {}", e[0]),
            }
        }
    }

    fn apply_error_test(test_cases: &[(&str, &str)]) {
        let env: Env = Rc::new(RefCell::new(Default::default()));
        for (input, expected) in test_cases {
            match parse(input) {
                Ok(node) => match eval(node, &env) {
                    Ok(evaluated) => panic!("expected `{}` to fail, got {}", input, evaluated),
                    Err(e) => assert_eq!(&e.to_string(), expected),
                },
                Err(e) => panic!("parse error: {}", e[0]),
            }
        }
    }

    #[test]
    fn test_integer_expressions() {
        let test_case = [
            ("1", "1"),
            ("-10", "-10"),
            ("5 + 5 + 5 + 5 - 10", "10"),
            ("2 * 2 * 2 * 2 * 2", "32"),
            ("(5 + 10 * 2 + 15 / 3) * 2 + -10", "50"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_boolean_expressions() {
        let test_case = [
            ("true", "true"),
            ("false", "false"),
            ("1 < 2", "true"),
            ("1 > 2", "false"),
            ("1 < 1", "false"),
            ("1 > 1", "false"),
            ("1 == 1", "true"),
            ("1 != 1", "false"),
            ("1 == 2", "false"),
            ("1 != 2", "true"),
            ("true == true", "true"),
            ("false == false", "true"),
            ("true == false", "false"),
            ("true != false", "true"),
            ("false != true", "true"),
            ("(1 < 2) == true", "true"),
            ("(1 < 2) == false", "false"),
            ("(1 > 2) == true", "false"),
            ("(1 > 2) == false", "true"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_bang_operators() {
        let test_case = [
            ("!true", "false"),
            ("!false", "true"),
            ("!5", "false"),
            ("!!true", "true"),
            ("!!false", "false"),
            ("!!5", "true"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_if_else_expressions() {
        let test_case = [
            ("if (true) { 10 }", "10"),
            ("if (false) { 10 }", "null"),
            ("if (1) { 10 }", "10"),
            ("if (1 < 2) { 10 }", "10"),
            ("if (1 > 2) { 10 }", "null"),
            ("if (1 > 2) { 10 } else { 20 }", "20"),
            ("if (1 < 2) { 10 } else { 20 }", "10"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_debugger_statements_are_completion_transparent() {
        let test_case = [
            ("debugger;", "null"),
            ("debugger; debugger;", "null"),
            ("1; debugger;", "1"),
            ("debugger; 1;", "1"),
            ("let a = 5; debugger; a;", "5"),
            ("if (true) { debugger; }", "null"),
            ("if (true) { 10; debugger; }", "10"),
            ("fn() { debugger; }()", "null"),
            ("fn() { 1; debugger; }()", "1"),
            ("fn() { let x = 1; debugger; }()", "null"),
            ("fn() { return 2; debugger; }()", "2"),
            ("fn(x) { if (x) { 1; debugger; } }(true)", "1"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_return_statements() {
        let test_case = [
            ("return 10;", "10"),
            ("return 10; 9;", "10"),
            ("return 2 * 5; 9;", "10"),
            ("9; return 2 * 5; 9;", "10"),
            ("if (10 > 1) { return 10; }", "10"),
            (
                "if (10 > 1) { \
                 if (10 > 1) { \
                 return 10; \
                 } \
                 return 1; \
                 }",
                "10",
            ),
            (
                "let f = fn(x) { \
                 return x; \
                 x + 10; \
                 }; \
                 f(10);",
                "10",
            ),
            (
                "let f = fn(x) { \
                 let result = x + 10; \
                 return result; \
                 return 10; \
                 }; \
                 f(10);",
                "20",
            ),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_let_statements() {
        let test_case = [
            ("let a = 5; a;", "5"),
            ("let a = 5 * 5; a;", "25"),
            ("let a = 5; let b = a; b;", "5"),
            ("let a = 5; let b = a; let c = a + b + 5; c;", "15"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_function_object() {
        let test_case = [("fn(x) { x + 2; };", "fn(x) { (x + 2); }")];
        apply_test(&test_case);
    }

    #[test]
    fn test_annotated_function_object() {
        // The tree-walking interpreter keeps the parameter list it was built
        // from, so printing a function shows its annotations. This is the one
        // declared exception to type erasure: see docs/type-system-design.md
        // section 6.1.
        let test_case = [
            ("fn(x: int): int { x + 2; };", "fn(x: int) { (x + 2); }"),
            ("fn(x: [int]?, y): bool { x; };", "fn(x: [int]?, y) { x; }"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_annotations_do_not_change_results() {
        let test_case = [
            ("let add = fn(x: int, y: int): int { x + y; }; add(5, 5);", "10"),
            ("let xs: [int] = [1, 2, 3]; xs[1]", "2"),
            ("let m: {string: int} = {\"a\": 1}; m[\"a\"]", "1"),
            (
                "let apply = fn(f: fn(int): int, v: int): int { f(v) }; apply(fn(n: int): int { n * 2 }, 4)",
                "8",
            ),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_function_application() {
        let test_case = [
            ("let identity = fn(x) { x; }; identity(5);", "5"),
            ("let identity = fn(x) { return x; }; identity(5);", "5"),
            ("let double = fn(x) { x * 2; }; double(5);", "10"),
            ("let add = fn(x, y) { x + y; }; add(5, 5);", "10"),
            ("let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));", "20"),
            ("fn(x) { x; }(5)", "5"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_closures_capture_binding_identity() {
        let test_case = [
            (
                r#"let captured: int = 1;
let read: fn(): int = fn(): int { captured + 1; };
let captured: string = "later";
read();"#,
                "2",
            ),
            (
                r#"let outer_value = 1;
let make_reader = fn() { fn() { outer_value; }; };
let nested_reader = make_reader();
let outer_value = 2;
nested_reader();"#,
                "1",
            ),
            (
                r#"let factorial = fn(n) {
  if (n == 0) { 1 } else { n * factorial(n - 1) }
};
factorial(5);"#,
                "120",
            ),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_string_concatenation() {
        let test_case = [
            (r#""Hello" + " " + "World!""#, "Hello World!"),
            (r#""Hello" == "Hello""#, "true"),
            (r#""Hello" == "Hi""#, "false"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_builtin_functions() {
        let test_case = [
            (r#"len("")"#, "0"),
            (r#"len("four")"#, "4"),
            (r#"len("hello world")"#, "11"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_array_literals() {
        let test_case = [("[1, 2 * 2, 3 + 3]", "[1, 4, 6]")];
        apply_test(&test_case);
    }

    #[test]
    fn test_array_index_expressions() {
        let test_case = [
            ("let i = 0; [1][i];", "1"),
            ("[1, 2, 3][1 + 1];", "3"),
            ("let myArray = [1, 2, 3]; myArray[2];", "3"),
            ("let myArray = [1, 2, 3]; myArray[0] + myArray[1] + myArray[2];", "6"),
            ("let myArray = [1, 2, 3]; let i = myArray[0]; myArray[i]", "2"),
            ("[1, 2, 3][3]", "null"),
            ("[1, 2, 3][-1]", "null"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_array_builtin_functions() {
        let test_case = [
            ("len([1, 2, 3])", "3"),
            ("len([])", "0"),
            (r#"puts("hello", "world!")"#, "null"),
            ("first([1, 2, 3])", "1"),
            ("first([])", "null"),
            ("last([1, 2, 3])", "3"),
            ("last([])", "null"),
            ("rest([1, 2, 3])", "[2, 3]"),
            ("rest([])", "null"),
            ("push([], 1)", "[1]"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_builtin_errors_stop_evaluation() {
        let test_case = [
            ("len(1)", "builtin len not supported for for type 1"),
            (r#"len("one", "two")"#, "builtin len expected 1 argument, got 2"),
            ("first(1)", "builtin first not supported for for type 1"),
            ("last(1)", "builtin last not supported for for type 1"),
            ("rest(1)", "builtin rest not supported for for type 1"),
            ("push(1, 1)", "builtin push not supported for for type 1"),
            // a failed builtin must abort the program instead of flowing on as a value
            ("if (len(1)) { 10 } else { 20 }", "builtin len not supported for for type 1"),
            ("len(1) == len(1)", "builtin len not supported for for type 1"),
            ("len(1) + 1", "builtin len not supported for for type 1"),
            ("[len(1), 2]", "builtin len not supported for for type 1"),
            (
                r#"if (len(1)) { "truthy" } else { "falsy" }"#,
                "builtin len not supported for for type 1",
            ),
            ("len(1); 42", "builtin len not supported for for type 1"),
            ("let broken = len(1); broken", "builtin len not supported for for type 1"),
            (
                "let identity = fn(x) { x }; identity(len(1))",
                "builtin len not supported for for type 1",
            ),
        ];
        apply_error_test(&test_case);
    }

    #[test]
    fn test_integer_arithmetic_errors() {
        // Raw i64 arithmetic used to panic in debug builds (and wrap in
        // release builds) for all of these; `1 / 0` and `i64::MIN / -1`
        // panicked in release builds too. They must be runtime errors, with
        // the same wording the bytecode VM uses.
        let test_case = [
            ("1 / 0;", "division by zero"),
            ("9223372036854775807 + 1;", "integer overflow in addition"),
            ("let m = -9223372036854775807 - 1; m - 1;", "integer overflow in subtraction"),
            ("9223372036854775807 * 2;", "integer overflow in multiplication"),
            ("let m = -9223372036854775807 - 1; m / -1;", "integer overflow in division"),
            ("let m = -9223372036854775807 - 1; -m;", "integer overflow in negation"),
        ];
        apply_error_test(&test_case);
    }

    #[test]
    fn test_integer_arithmetic_boundaries_still_evaluate() {
        let test_case = [
            ("9223372036854775807 + 0;", "9223372036854775807"),
            ("-9223372036854775807 - 1;", "-9223372036854775808"),
            ("(-9223372036854775807 - 1) / 1;", "-9223372036854775808"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_array_builtin_arity_errors() {
        // A wrong argument count must produce an error, not a panic and not a
        // silently truncated call. `push` used to read args.first()/args.last(),
        // so `push([1])` appended the array to itself and `push([1], 2, 3)`
        // dropped the middle argument; `first`/`last`/`rest` indexed args[0]
        // unchecked and panicked when called with none. The gc VM already
        // rejected all of these, with the wording pinned here.
        let test_case = [
            ("first()", "builtin first expected 1 argument, got 0"),
            ("first([1], [2])", "builtin first expected 1 argument, got 2"),
            ("last()", "builtin last expected 1 argument, got 0"),
            ("last([1], [2])", "builtin last expected 1 argument, got 2"),
            ("rest()", "builtin rest expected 1 argument, got 0"),
            ("rest([1], [2])", "builtin rest expected 1 argument, got 2"),
            ("push()", "builtin push expected 2 arguments, got 0"),
            ("push([1])", "builtin push expected 2 arguments, got 1"),
            ("push([1], 2, 3)", "builtin push expected 2 arguments, got 3"),
            ("len()", "builtin len expected 1 argument, got 0"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_hash_index_expressions() {
        let test_case = [
            (r#"{"foo": 5}["foo"]"#, "5"),
            (r#"{"foo": 5}["bar"]"#, "null"),
            (r#"let key = "foo"; {"foo": 5}[key]"#, "5"),
            (r#"{}["foo"]"#, "null"),
            (r#"{5: 5}[5]"#, "5"),
            (r#"{true: 5}[true]"#, "5"),
            (r#"{false: 5}[false]"#, "5"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_class_constructor_methods_and_dynamic_fields() {
        let test_case = [
            (
                r#"class Point {
  constructor(x, y) { this.x = x; this.y = y; }
  sum() { this.x + this.y; }
}
let point = new Point(20, 22);
point.sum();"#,
                "42",
            ),
            (
                r#"class Empty { value() { 42; } }
let empty = new Empty();
empty.value();"#,
                "42",
            ),
            (
                r#"class Value { value() { 1; } }
let value = new Value();
value.value = 42;
value.value;"#,
                "42",
            ),
            (
                r#"class Mutable { constructor(value) { this.value = value; } }
let value = new Mutable(1);
value.value = 42;
value.value;"#,
                "42",
            ),
            (
                r#"class Trace {
  constructor() { this.order = 0; }
  mark(value) { this.order = this.order * 10 + value; value; }
  target() { this.mark(1); this; }
}
class Pair { constructor(left, right) { this.value = left + right; } }
let trace = new Trace();
trace.target().value = trace.mark(2);
let pair = new Pair(trace.mark(3), trace.mark(4));
trace.order;"#,
                "1234",
            ),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_class_methods_capture_the_declared_class_binding() {
        let test_case = [(
            r#"class CapturedClass {
  make() { new CapturedClass(); }
  value() { 1; }
}
let old_instance = new CapturedClass();
class CapturedClass { value() { 2; } }
old_instance.make().value();"#,
            "1",
        )];
        apply_test(&test_case);
    }

    #[test]
    fn test_detached_method_and_lexical_this_capture() {
        let test_case = [
            (
                r#"class Counter {
  constructor(value) { this.value = value; }
  current() { this.value; }
}
let current = new Counter(42).current;
current();"#,
                "42",
            ),
            (
                r#"class Box {
  constructor(value) { this.value = value; }
  reader() { fn() { fn() { this.value; }; }; }
}
let reader = new Box(42).reader();
reader()();"#,
                "42",
            ),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_class_instance_and_bound_method_identity() {
        let test_case = [
            ("class A {} let Alias = A; A == Alias;", "true"),
            ("class A {} new A() == new A();", "false"),
            ("class A { method() { 1; } } let a = new A(); let f = a.method; f == f;", "true"),
            ("class A { method() { 1; } } let a = new A(); a.method == a.method;", "false"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn test_class_errors_and_strict_arity() {
        let test_case = [
            (
                "class Empty {} new Empty(1);",
                "wrong number of arguments for Empty.constructor: want=0, got=1",
            ),
            (
                "class A { method(value) { value; } } new A().method();",
                "wrong number of arguments for method: want=1, got=0",
            ),
            ("class A {} A();", "class A must be constructed with new"),
            ("let f = fn() {}; new f();", "cannot construct fn() {  }"),
            ("class A {} new A().missing;", "property 'missing' does not exist on A"),
            ("1.value;", "cannot read property 'value' of 1"),
            ("let f = fn(a) { a; }; f();", "wrong number of arguments: want=1, got=0"),
            ("class A { constructor() { return 1; } }", "constructor cannot return a value"),
        ];
        apply_test(&test_case);
    }

    #[test]
    fn class_cycle_display_is_opaque() {
        apply_test(&[(
            "class Node {} let node = new Node(); node.next = node; node;",
            "[object Node]",
        )]);
    }

    #[test]
    fn validation_accepts_bindings_from_previous_eval() {
        let env: Env = Rc::new(RefCell::new(Default::default()));
        eval(parse("let answer = 41;").unwrap(), &env).unwrap();

        let result = eval(parse("answer + 1;").unwrap(), &env).unwrap();
        assert_eq!(result.as_ref(), &object::Object::Integer(42));
    }
}
