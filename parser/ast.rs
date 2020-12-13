use core::fmt;
use core::fmt::Result;
use std::fmt::Formatter;
use lexer::token::Token;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    pub fn new() -> Self {
        Program { statements: vec![] }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", format_statements(&self.statements))
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
            Statement::Return(expr) => write!(f, "return {};", expr),
            Statement::Expr(expr) => write!(f, "{}", expr),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Literal {
    Integer(i64),
    Boolean(bool),
    String(String),
    Array(Vec<Expression>),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Integer(i) => write!(f, "{}", i),
            Literal::Boolean(b) => write!(f, "{}", b),
            Literal::String(s) => write!(f, "\"{}\"", s),
            Literal::Array(e) => write!(f, "[{}]", format_expressions(e)),
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
