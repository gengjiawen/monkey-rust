mod object;

use parser::ast::*;
use crate::object::{EvalError, Object};

pub fn eval(node: Node) -> Result<Object, EvalError> {
    match node {
        Node::Program(p) => eval_program(&p),
        Node::Statement(statements) => eval_statement(&statements),
        Node::Expression(expression) => eval_expression(&expression),
    }
}

fn eval_program(p: &Program) -> Result<Object, EvalError> {
    let mut result = Object::Null;
    for statement in &p.statements {
        let val = eval_statement(statement)?;
        match val {
            _ =>  { result = val; },
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

fn eval_expression(expression: &Expression) -> Result<Object, EvalError> {
    match expression {
        Expression::LITERAL(literal) => eval_literal(literal),
        _ => return Err(String::from("unknown expression"))
    }
}

fn eval_literal(literal: &Literal) -> Result<Object, EvalError> {
    match literal {
        Literal::Integer(i) => Ok(Object::Integer(*i)),
        _ => return Err(String::from("unknown literal"))
    }
}

mod tests {
    use parser::*;
    use crate::eval;
    use crate::object::EvalError;
    use parser::ast::Node;

    fn apply_test(test_cases: &[(&str, &str)]) {

        for (input, expected) in test_cases {
            match parse(input) {
                Ok(node) => {
                    match eval(node) {
                        Ok(evaluated) => assert_eq!(&format!("{}", evaluated), expected),
                        Err(e) => assert_eq!(&format!("{}", e), expected),
                    }
                },
                Err(e) => panic!("parse error: {}", e[0])
            }
        }
    }

    #[test]
    fn test_integer_expressions() {
        let test_case = [
            ("1", "1"),
            // ("-10", "-10"),
            // ("5 + 5 + 5 + 5 - 10", "10"),
            // ("2 * 2 * 2 * 2 * 2", "32"),
            // ("(5 + 10 * 2 + 15 / 3) * 2 + -10", "50"),
        ];
        apply_test(&test_case);
    }



}
