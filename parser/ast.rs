use core::fmt;
use core::fmt::Result;
use std::fmt::Formatter;
use lexer::token::{Token, TokenKind, Span};
use serde::{Deserialize, Serialize};

// still wait for https://github.com/serde-rs/serde/issues/1402
// or https://github.com/serde-rs/serde/issues/760
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
#[serde(tag = "type")]
pub struct Program {
    pub body: Vec<Statement>,
    pub span: Span,
}

impl Program {
    pub fn new() -> Self {
        Program { body: vec![], span: Span {
            start: 0,
            end: 0,
        } }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", format_statements(&self.body))
    }
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub struct Let {
    pub identifier: Token, // rust can't do precise type with enum
    pub expr: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "stmt_type")]
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
#[serde(tag = "expr_type")]
pub enum Expression {
    IDENTIFIER(IDENTIFIER),
    LITERAL(Literal), // need to flatten
    PREFIX(UnaryExpression),
    INFIX(BinaryExpression),
    IF(IF),
    FUNCTION(FunctionDeclaration),
    FunctionCall(FunctionCall),
    Index(Index),
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct IDENTIFIER {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct UnaryExpression {
    pub op: Token,
    pub operand: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct BinaryExpression {
    pub op: Token,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct IF {
    pub condition: Box<Expression>,
    pub consequent: BlockStatement,
    pub alternate: Option<BlockStatement>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct FunctionDeclaration {
    pub params: Vec<String>,
    pub body: BlockStatement,
    pub span: Span,
}

// function can be Identifier or FunctionLiteral (think iife)
#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct FunctionCall {
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub struct Index {
    pub object: Box<Expression>,
    pub index: Box<Expression>,
    pub span: Span,
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Expression::IDENTIFIER(IDENTIFIER { name: id, .. }) => write!(f, "{}", id),
            Expression::LITERAL(l) => write!(f, "{}",l),
            Expression::PREFIX(UnaryExpression { op, operand: expr, .. }) => {
                write!(f, "({}{})", op.kind, expr)
            },
            Expression::INFIX(BinaryExpression { op, left, right, .. }) => {
                write!(f, "({} {} {})", left, op.kind, right)
            },
            Expression::IF(IF { condition, consequent, alternate, .. }) => {
                if let Some(else_block) = alternate {
                    write!(f,
                           "if {} {{ {} }} else {{ {} }}",
                           condition,
                           consequent,
                           else_block,
                    )
                } else {
                    write!(f,
                           "if {} {{ {} }}",
                           condition,
                           consequent,
                    )
                }
            }
            Expression::FUNCTION(FunctionDeclaration { params, body, .. }) => {
                write!(f, "fn({}) {{ {} }}", params.join(", "), body)
            }
            Expression::FunctionCall(FunctionCall { callee, arguments, .. }) => {
                write!(f, "{}({})", callee, format_expressions(arguments))
            }
            Expression::Index(Index { object, index, .. }) => {
                write!(f, "({}[{}])", object, index)
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
