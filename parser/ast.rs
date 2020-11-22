use core::fmt;
use core::fmt::Result;
use std::fmt::Formatter;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Node {
    Program(Program),
    Statement(Statement),
    Expression(Expression)
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Node::Program(p) => write!(f, "{}", format_statements(&p.statements)),
            Node::Statement(stmt) => write!(f, "{}", stmt),
            Node::Expression(expr) => write!(f, "{}", expr),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Statement {
    Let(String, Expression),
    Return(Expression),
    Expr(Expression),
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Statement::Let(id, expr) => write!(f, "let {} = {};", id, expr),
            Statement::Return(expr) => write!(f, "return {}", expr),
            Statement::Expr(expr) => write!(f, "{}", expr),
        }
    }
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
    Boolean(bool),
    String(String),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Integer(i) => write!(f, "{}", i),
            Literal::Boolean(b) => write!(f, "{}", b),
            Literal::String(s) => write!(f, "\"{}\"", s),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    pub fn new() -> Self {
        Program { statements: vec![] }
    }
}

fn format_statements(statements: &Vec<Statement>) -> String {
    return statements
        .iter()
        .map(|stmt| stmt.to_string())
        .collect::<Vec<String>>()
        .join("")
}
