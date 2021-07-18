mod builtins;
mod object;
pub mod environment;

use parser::ast::*;
use crate::environment::*;
use crate::object::{EvalError, Object};
use parser::lexer::token::{TokenKind, Token};
use std::rc::Rc;
use std::cell::RefCell;
use crate::builtins::BUILTINS;
use std::collections::HashMap;

pub fn eval(node: Node, env: &Env) -> Result<Rc<Object>, EvalError> {
    match node {
        Node::Program(p) => eval_block_statements(&p.body, env),
        Node::Statement(statements) => eval_statement(&statements, env),
        Node::Expression(expression) => eval_expression(&expression, env),
    }
}

fn eval_block_statements(statements: &Vec<Statement>, env: &Env) -> Result<Rc<Object>, EvalError> {
    let mut result = Rc::new(Object::Null);
    for statement in statements {
        let val = eval_statement(statement, &Rc::clone(env))?;
        match *val {
            Object::ReturnValue(_) => return Ok(val),
            _ => { result = val; }
        }
    }

    return Ok(result);
}

fn eval_statement(statement: &Statement, env: &Env) -> Result<Rc<Object>, EvalError> {
    match statement {
        Statement::Expr(expr) => eval_expression(expr, env),
        Statement::Return(ReturnStatement { argument, .. }) => {
            let val = eval_expression(argument, env)?;
            return Ok(Rc::new(Object::ReturnValue(val)));
        }
        Statement::Let(Let { identifier: id, expr, .. }) => {
            let val = eval_expression(expr, &Rc::clone(env))?;
            let obj: Rc<Object> = Rc::clone(&val);
            if let TokenKind::IDENTIFIER {name} = &id.kind {
                env.borrow_mut().set(name.clone(), obj);
            }
            return Ok(Rc::new(Object::Null));
        }
    }
}

fn is_truthy(obj: &Object) -> bool {
    match obj {
        Object::Null => return false,
        Object::Boolean(false) => return false,
        _ => true,
    }
}

fn eval_expression(expression: &Expression, env: &Env) -> Result<Rc<Object>, EvalError> {
    match expression {
        Expression::LITERAL(literal) => eval_literal(literal, env),
        Expression::PREFIX(UnaryExpression { op, operand: expr, .. }) => {
            let right = eval_expression(expr, &Rc::clone(env))?;
            return eval_prefix(op, &right);
        }
        Expression::INFIX(BinaryExpression { op, left, right, .. }) => {
            let left = eval_expression(left, &Rc::clone(env))?;
            let right = eval_expression(right, &Rc::clone(env))?;
            return eval_infix(op, &left, &right);
        }
        Expression::IF(IF { condition, consequent, alternate, .. }) => {
            let condition = eval_expression(condition, &Rc::clone(env))?;
            if is_truthy(&condition) {
                eval_block_statements(&(consequent.body), env)
            } else {
                match alternate {
                    Some(alt) => eval_block_statements(&(alt.body), env),
                    None => Ok(Rc::new(Object::Null))
                }
            }
        }
        Expression::IDENTIFIER(IDENTIFIER { name: id, .. }) => eval_identifier(&id, env),
        Expression::FUNCTION(FunctionDeclaration { params, body, .. }) => {
            return Ok(Rc::new(Object::Function(params.clone(), body.clone(), Rc::clone(env))));
        }
        Expression::FunctionCall(FunctionCall { callee, arguments, .. }) => {
            let func = eval_expression(callee, &Rc::clone(env))?;
            let args = eval_expressions(arguments, env)?;
            apply_function(&func, &args)
        }
        Expression::Index(Index { object: left, index, .. }) => {
            let literal = eval_expression(left, &Rc::clone(env))?;
            let index = eval_expression(index, env)?;
            eval_index_expression(&literal, &index)
        }
    }
}

fn eval_index_expression(left: &Rc<Object>, index: &Rc<Object>) -> Result<Rc<Object>, EvalError> {
    match (&**left, &**index) {
        (Object::Array(arr), Object::Integer(idx)) => {
            match arr.get(*idx as usize) {
                Some(obj) => return Ok(Rc::clone(obj)),
                None => return Ok(Rc::new(Object::Null))
            }
        },
        (Object::Hash(map), key) => {
            if !(key.is_hashable()) {
                return Err(format!("not a valid hash key"))
            }

            match map.get(key) {
                Some(obj) => return Ok(Rc::clone(obj)),
                None => return Ok(Rc::new(Object::Null))
            }
        },
        _ => return Err(format!("index operator not supported for {}", left)),
    }
}

fn apply_function(function: &Rc<Object>, args: &Vec<Rc<Object>>) -> Result<Rc<Object>, EvalError> {
    match &**function {
        Object::Function(params, body, env) => {
            let mut env = Environment::new_enclosed_environment(&env);

            params.iter().enumerate().for_each(|(i, param)| {
                env.set(param.clone(), args[i].clone());
            });

            let evaluated = eval_block_statements(&body.body, &Rc::new(RefCell::new(env)))?;
            return unwrap_return(evaluated);

        },
        Object::Builtin(b) => Ok(b(args.to_vec())),
        f => Err(format!("expected {} to be a function", f))
    }
}

fn unwrap_return(obj: Rc<Object>) -> Result<Rc<Object>, EvalError> {
    if let Object::ReturnValue(val) = &*obj {
        Ok(Rc::clone(&val))
    } else {
        Ok(obj)
    }
}

fn eval_expressions(exprs: &Vec<Expression>, env: &Env) -> Result<Vec<Rc<Object>>, EvalError> {
    let mut list = Vec::new();
    for expr in exprs {
        let val = eval_expression(expr, &Rc::clone(env))?;
        list.push(val);
    }

    Ok(list)
}

fn eval_identifier(id: &str, env: &Env) -> Result<Rc<Object>, EvalError> {
    match env.borrow().get(id) {
        Some(obj) => Ok(obj.clone()),
        None => {
            match BUILTINS.get(id) {
                Some(obj) => Ok(Rc::new(Object::Builtin(*obj))),
                None => Err(format!("unknown identifier {}", id)),
            }
        }
    }
}

fn eval_prefix(op: &Token, right: &Object) -> Result<Rc<Object>, EvalError> {
    match op.kind {
        TokenKind::BANG => eval_prefix_bang(right),
        TokenKind::MINUS => eval_prefix_minus(right),
        _ => Err(format!("unknown prefix operator: {}", op))
    }
}

fn eval_prefix_bang(expr: &Object) -> Result<Rc<Object>, EvalError> {
    match *expr {
        Object::Null => Ok(Rc::new(Object::Boolean(true))),
        Object::Boolean(b) => Ok(Rc::new(Object::Boolean(!b))),
        _ => Ok(Rc::new(Object::Boolean(false)))
    }
}

fn eval_prefix_minus(expr: &Object) -> Result<Rc<Object>, EvalError> {
    match *expr {
        Object::Integer(i) => Ok(Rc::from(Object::Integer(-i))),
        _ => Err(format!("can't apply prefix minus operator: {}", expr))
    }
}

fn eval_infix(op: &Token, left: &Object, right: &Object) -> Result<Rc<Object>, EvalError> {
    match (left, right) {
        (Object::Integer(left), Object::Integer(right)) => {
            return eval_integer_infix(op, *left, *right);
        }
        (Object::Boolean(left), Object::Boolean(right)) => {
            return eval_boolean_infix(op, *left, *right);
        }
        (Object::String(left), Object::String(right)) => {
            return eval_string_infix(op, left.to_string(), right.to_string());
        }
        _ => Err(format!("eval infix error for op: {}, left: {}, right: {}", op, left, right))
    }
}

fn eval_integer_infix(op: &Token, left: i64, right: i64) -> Result<Rc<Object>, EvalError> {
    let result = match &op.kind {
        TokenKind::PLUS => Object::Integer(left + right),
        TokenKind::MINUS => Object::Integer(left - right),
        TokenKind::ASTERISK => Object::Integer(left * right),
        TokenKind::SLASH => Object::Integer(left / right),
        TokenKind::LT => Object::Boolean(left < right),
        TokenKind::GT => Object::Boolean(left > right),
        TokenKind::EQ => Object::Boolean(left == right),
        TokenKind::NotEq => Object::Boolean(left != right),
        op => return Err(format!("Invalid infix operator {} for int", op))
    };

    Ok(Rc::from(result))
}

fn eval_boolean_infix(op: &Token, left: bool, right: bool) -> Result<Rc<Object>, EvalError> {
    let result = match &op.kind {
        TokenKind::EQ => Object::Boolean(left == right),
        TokenKind::NotEq => Object::Boolean(left != right),
        op => return Err(format!("Invalid infix operator for int: {}", op))
    };

    Ok(Rc::from(result))
}

fn eval_string_infix(op: &Token, left: String, right: String) -> Result<Rc<Object>, EvalError> {
    let result = match &op.kind {
        TokenKind::EQ => Object::Boolean(left == right),
        TokenKind::NotEq => Object::Boolean(left != right),
        TokenKind::PLUS => Object::String(format!("{}{}", left, right)),
        op => return Err(format!("Invalid infix {} operator for string", op))
    };

    Ok(Rc::from(result))
}

fn eval_literal(literal: &Literal, env: &Env) -> Result<Rc<Object>, EvalError> {
    match literal {
        Literal::Integer(Integer { raw: i, .. }) => Ok(Rc::from(Object::Integer(*i))),
        Literal::Boolean(Boolean { raw: b, .. }) => Ok(Rc::from(Object::Boolean(*b))),
        Literal::String(StringType { raw: s, .. }) => Ok(Rc::from(Object::String(s.clone()))),
        Literal::Array(Array { elements, .. }) => {
            let list = eval_expressions(elements, env)?;
            return Ok(Rc::from(Object::Array(list)));
        }
        Literal::Hash(Hash { elements: map, .. }) => {
            let mut hash_map = HashMap::new();

            for (k, v) in map {
                let key = eval_expression(k, env)?;
                if !key.is_hashable() {
                    return Err(format!("key {} is not hashable", key));
                }
                let value = eval_expression(v, env)?;
                hash_map.insert(key, value);
            }

            return Ok(Rc::new(Object::Hash(hash_map)));
        }
        // l => return Err(format!("unknown literal: {}", *l))
    }
}

#[cfg(test)]
mod tests {
    use parser::*;
    use crate::eval;
    use std::rc::Rc;
    use std::cell::RefCell;
    use crate::environment::*;

    fn apply_test(test_cases: &[(&str, &str)]) {
        let env: Env = Rc::new(RefCell::new(Default::default()));
        for (input, expected) in test_cases {
            match parse(input) {
                Ok(node) => {
                    match eval(node, &env) {
                        Ok(evaluated) => assert_eq!(&format!("{}", evaluated), expected),
                        Err(e) => assert_eq!(&format!("{}", e), expected),
                    }
                }
                Err(e) => panic!("parse error: {}", e[0])
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
            (
                "let add = fn(x, y) { x + y; }; add(5 + 5, add(5, 5));",
                "20",
            ),
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
            (
                "let myArray = [1, 2, 3]; myArray[0] + myArray[1] + myArray[2];",
                "6",
            ),
            (
                "let myArray = [1, 2, 3]; let i = myArray[0]; myArray[i]",
                "2",
            ),
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

}
