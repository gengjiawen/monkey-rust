mod object;
pub mod environment;

use parser::ast::*;
use crate::environment::*;
use crate::object::{EvalError, Object};
use parser::lexer::token::Token;
use std::rc::Rc;
use std::borrow::Borrow;

pub fn eval(node: Node, env: &Env) -> Result<Rc<Object>, EvalError> {
    match node {
        Node::Program(p) => eval_block_statements(&p.statements, env),
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
        Statement::Return(expr) => {
            let val = eval_expression(expr, env)?;
            return Ok(Rc::new(Object::ReturnValue(val)));
        },
        Statement::Let(id, expr) => {
            let val = eval_expression(expr, &Rc::clone(env))?;
            let obj: Rc<Object> = Rc::clone(&val);
            env.borrow_mut().set(id.clone(), obj);
            return Ok(val);
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
        Expression::LITERAL(literal) => eval_literal(literal),
        Expression::PREFIX(op, expr) => {
            let right = eval_expression(expr, &Rc::clone(env))?;
            return eval_prefix(op, &right);
        }
        Expression::INFIX(op, left, right) => {
            let left = eval_expression(left, &Rc::clone(env))?;
            let right = eval_expression(right, &Rc::clone(env))?;
            return eval_infix(op, &left, &right);
        }
        Expression::IF(condition, consequence, alternative) => {
            let condition = eval_expression(condition, &Rc::clone(env))?;
            if is_truthy(&condition) {
                eval_block_statements(&(consequence.0), env)
            } else {
                match alternative {
                    Some(alt) => eval_block_statements(&(alt.0), env),
                    None => Ok(Rc::new(Object::Null))
                }
            }
        }
        Expression::IDENTIFIER(id) => eval_identifier(&id, env),
        // Expression::FUNCTION(_, _) => {}
        // Expression::FunctionCall(_, _) => {}
        _ => return Err(String::from("unknown expression"))
    }
}

fn eval_identifier(id: &str, env: &Env) -> Result<Rc<Object>, EvalError> {
    // todo why borrow not working ?
    match env.borrow_mut().get(id) {
        Some(obj) => Ok(obj.clone()),
        None => Err(format!("unknown identifier {}", id))
    }
}

fn eval_prefix(op: &Token, right: &Object) -> Result<Rc<Object>, EvalError> {
    match op {
        Token::BANG => eval_prefix_bang(right),
        Token::MINUS => eval_prefix_minus(right),
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
        _ => Err(format!("eval infix error for op: {}, left: {}, right: {}", op, left, right))
    }
}

fn eval_integer_infix(op: &Token, left: i64, right: i64) -> Result<Rc<Object>, EvalError> {
    let result = match op {
        Token::PLUS => Object::Integer(left + right),
        Token::MINUS => Object::Integer(left - right),
        Token::ASTERISK => Object::Integer(left * right),
        Token::SLASH => Object::Integer(left / right),
        Token::LT => Object::Boolean(left < right),
        Token::GT => Object::Boolean(left > right),
        Token::EQ => Object::Boolean(left == right),
        Token::NotEq => Object::Boolean(left != right),
        op => return Err(format!("Invalid infix operator for int: {}", op))
    };

    Ok(Rc::from(result))
}

fn eval_boolean_infix(op: &Token, left: bool, right: bool) -> Result<Rc<Object>, EvalError> {
    let result = match op {
        Token::EQ => Object::Boolean(left == right),
        Token::NotEq => Object::Boolean(left != right),
        op => return Err(format!("Invalid infix operator for int: {}", op))
    };

    Ok(Rc::from(result))
}


fn eval_literal(literal: &Literal) -> Result<Rc<Object>, EvalError> {
    match literal {
        Literal::Integer(i) => Ok(Rc::from(Object::Integer(*i))),
        Literal::Boolean(b) => Ok(Rc::from(Object::Boolean(*b))),
        // Literal::String(s) => Ok(Object::String(s)),
        l => return Err(format!("unknown literal: {}", *l))
    }
}

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
            // (
            //     "let f = fn(x) { \
            //      return x; \
            //      x + 10; \
            //      }; \
            //      f(10);",
            //     "10",
            // ),
            // (
            //     "let f = fn(x) { \
            //      let result = x + 10; \
            //      return result; \
            //      return 10; \
            //      }; \
            //      f(10);",
            //     "20",
            // ),
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
}
