mod object;

use parser::ast::*;
use crate::object::{EvalError, Object};
use parser::lexer::token::Token;

pub fn eval(node: Node) -> Result<Object, EvalError> {
    match node {
        Node::Program(p) => eval_block_statements(&p.statements),
        Node::Statement(statements) => eval_statement(&statements),
        Node::Expression(expression) => eval_expression(&expression),
    }
}

fn eval_block_statements(statements: &Vec<Statement>) -> Result<Object, EvalError> {
    let mut result = Object::Null;
    for statement in statements {
        let val = eval_statement(statement)?;
        match val {
            _ => { result = val; }
        }
    }

    return Ok(result);
}

fn eval_statement(statement: &Statement) -> Result<Object, EvalError> {
    match statement {
        Statement::Expr(expr) => eval_expression(expr),
        _ => return Err(String::from("unknown statement"))
    }
}

fn is_truthy(obj: &Object) -> bool {
    match obj {
        Object::Null => return false,
        Object::Boolean(false) => return false,
        _ => true,
    }
}

fn eval_expression(expression: &Expression) -> Result<Object, EvalError> {
    match expression {
        Expression::LITERAL(literal) => eval_literal(literal),
        Expression::PREFIX(op, expr) => {
            let right = eval_expression(expr)?;
            return eval_prefix(op, &right);
        }
        Expression::INFIX(op, left, right) => {
            let left = eval_expression(left)?;
            let right = eval_expression(right)?;
            return eval_infix(op, &left, &right);
        }
        Expression::IF(condition, consequence, alternative) => {
            let condition = eval_expression(condition)?;
            if is_truthy(&condition) {
                eval_block_statements(&(consequence.0))
            } else {
                match alternative {
                    Some(alt) => eval_block_statements(&(alt.0)),
                    None => Ok(Object::Null)
                }
            }
        }
        // Expression::IDENTIFIER(_) => {}
        // Expression::FUNCTION(_, _) => {}
        // Expression::FunctionCall(_, _) => {}
        _ => return Err(String::from("unknown expression"))
    }
}

fn eval_prefix(op: &Token, right: &Object) -> Result<Object, EvalError> {
    match op {
        Token::BANG => eval_prefix_bang(right),
        Token::MINUS => eval_prefix_minus(right),
        _ => Err(format!("unknown prefix operator: {}", op))
    }
}

fn eval_prefix_bang(expr: &Object) -> Result<Object, EvalError> {
    match *expr {
        Object::Null => Ok(Object::Boolean(true)),
        Object::Boolean(b) => Ok(Object::Boolean(!b)),
        _ => Ok(Object::Boolean(false))
    }
}

fn eval_prefix_minus(expr: &Object) -> Result<Object, EvalError> {
    match *expr {
        Object::Integer(i) => Ok(Object::Integer(-i)),
        _ => Err(format!("can't apply prefix minus operator: {}", expr))
    }
}

fn eval_infix(op: &Token, left: &Object, right: &Object) -> Result<Object, EvalError> {
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

fn eval_integer_infix(op: &Token, left: i64, right: i64) -> Result<Object, EvalError> {
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

    Ok(result)
}

fn eval_boolean_infix(op: &Token, left: bool, right: bool) -> Result<Object, EvalError> {
    let result = match op {
        Token::EQ => Object::Boolean(left == right),
        Token::NotEq => Object::Boolean(left != right),
        op => return Err(format!("Invalid infix operator for int: {}", op))
    };

    Ok(result)
}


fn eval_literal(literal: &Literal) -> Result<Object, EvalError> {
    match literal {
        Literal::Integer(i) => Ok(Object::Integer(*i)),
        Literal::Boolean(b) => Ok(Object::Boolean(*b)),
        // Literal::String(s) => Ok(Object::String(s)),
        l => return Err(format!("unknown literal: {}", *l))
    }
}

mod tests {
    use parser::*;
    use crate::eval;

    fn apply_test(test_cases: &[(&str, &str)]) {
        for (input, expected) in test_cases {
            match parse(input) {
                Ok(node) => {
                    match eval(node) {
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
}
