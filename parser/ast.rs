use core::fmt;
use core::fmt::Result;
use std::fmt::Formatter;
use lexer::token::{Token, TokenKind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub enum Node {
    Program(Program),
    Statement(Statement),
    Expression(Expression)
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Node::Program(p) => write!(f, "{}", p),
            Node::Statement(stmt) => write!(f, "{}", stmt),
            Node::Expression(expr) => write!(f, "{}", expr),
        }
    }
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Program {
    pub body: Vec<Statement>,
}

impl Program {
    pub fn new() -> Self {
        Program { body: vec![] }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", format_statements(&self.body))
    }
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Let {
    pub identifier: Token, // rust can't do precise type with enum
    pub expr: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub enum Statement {
    Let(Let),
    Return(Expression),
    Expr(Expression),
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Statement::Let(Let { identifier: id, expr, .. }) => {
                if let TokenKind::IDENTIFIER {name} = &id.kind {
                    return write!(f, "let {} = {};", name, expr)
                }
                panic!("unreachable")
            },
            Statement::Return(expr) => write!(f, "return {};", expr),
            Statement::Expr(expr) => write!(f, "{}", expr),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Serialize, Deserialize, PartialEq)]
pub struct BlockStatement(pub Vec<Statement>);

impl BlockStatement {
    pub fn new(statements: Vec<Statement>) -> BlockStatement {
        BlockStatement(statements)
    }
}

impl fmt::Display for BlockStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", format_statements(&self.0))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub enum Expression {
    IDENTIFIER(String),
    LITERAL(Literal),
    PREFIX(Token, Box<Expression>),
    INFIX(Token, Box<Expression>, Box<Expression>),
    IF(Box<Expression>, BlockStatement, Option<BlockStatement>),
    FUNCTION(Vec<String>, BlockStatement),
    FunctionCall(Box<Expression>, Vec<Expression>), // function can be Identifier or FunctionLiteral (think iife)
    Index(Box<Expression>, Box<Expression>),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Expression::IDENTIFIER(id) => write!(f, "{}", id),
            Expression::LITERAL(l) => write!(f, "{}",l),
            Expression::PREFIX(op, expr) => write!(f, "({}{})", op.kind, expr),
            Expression::INFIX(op, left, right) => write!(f, "({} {} {})", left, op.kind, right),
            Expression::IF(condition, if_block, else_block) => {
                if let Some(else_block) = else_block {
                    write!(f,
                           "if {} {{ {} }} else {{ {} }}",
                           condition,
                           if_block,
                           else_block
                    )
                } else {
                    write!(f,
                           "if {} {{ {} }}",
                           condition,
                           if_block,
                    )
                }
            }
            Expression::FUNCTION(params, func_body) => {
                write!(f, "fn({}) {{ {} }}", params.join(", "), func_body)
            }
            Expression::FunctionCall(function, args) => {
                write!(f, "{}({})", function, format_expressions(args))
            }
            Expression::Index(left, index) => {
                write!(f, "({}[{}])", left, index)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub enum Literal {
    Integer(Integer),
    Boolean(Boolean),
    String(StringType),
    Array(Array),
    Hash(Hash),
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Integer {
    pub raw: i64,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Boolean {
    pub raw: bool,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct StringType {
    pub raw: String,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Array {
    pub elements: Vec<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Hash {
    pub elements: Vec<(Expression, Expression)>,
    pub span: Span,
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Integer(Integer { raw: i, .. }) => write!(f, "{}", i),
            Literal::Boolean(Boolean { raw: b, .. }) => write!(f, "{}", b),
            Literal::String(StringType { raw: s, .. }) => write!(f, "\"{}\"", s),
            Literal::Array(Array { elements: e, .. }) => write!(f, "[{}]", format_expressions(e)),
            Literal::Hash(Hash { elements: map, .. }) => {
                let to_string = map
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect::<Vec<String>>()
                    .join(", ");

                write!(f, "{{{}}}", to_string)
            }
        }
    }
}

fn format_statements(statements: &Vec<Statement>) -> String {
    return statements
        .iter()
        .map(|stmt| stmt.to_string())
        .collect::<Vec<String>>()
        .join("")
}

fn format_expressions(exprs: &Vec<Expression>) -> String {
    return exprs
        .iter()
        .map(|stmt| stmt.to_string())
        .collect::<Vec<String>>()
        .join(", ")
}
