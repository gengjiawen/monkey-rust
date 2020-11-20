use lexer::token::{Token};
use core::fmt;
use std::fmt::Formatter;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Node {
    Program(Program),
    Statement(Statement),
    Expression(Expression)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Statement {
    Let(String, Expression)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Expression {
    IDENTIFIER(String),
    LITERAL(Literal),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Expression::IDENTIFIER(id) => write!(f, "{}", id),
            Expression::LITERAL(l) => write!(f, "{}",l),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Literal {
    Integer(i64),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Integer(i) => write!(f, "{}", i)
        }
    }
}

impl Program {
    pub fn new() -> Self {
        Program { statements: vec![] }
    }
}
