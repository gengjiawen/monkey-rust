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
        let test_case = [("fn(x) { x + 2; };", "fn(x) { (x + 2) }")];
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
        // let illegal_cases = [
        //     "len(1)",
        //     r#"len("one", "two")"#,
        //     "first(1)",
        //     "last(1)",
        //     "push(1, 1)"
        // ];
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
