use core::fmt;
use core::fmt::Result;
use lexer::token::{Span, Token};
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;

// still wait for https://github.com/serde-rs/serde/issues/1402
#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
pub enum Node {
    Program(Program),
    Statement(Statement),
    Expression(Expression),
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

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

impl Program {
    pub fn new() -> Self {
        Program {
            body: vec![],
            span: Span {
                start: 0,
                end: 0,
            },
        }
    }
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", format_statements(&self.body))
    }
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(untagged)]
pub enum Statement {
    Let(Let),
    Return(ReturnStatement),
    Class(ClassDeclaration),
    SetProperty(SetPropertyStatement),
    Debugger(DebuggerStatement),
    Expr(Expression),
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct ClassDeclaration {
    pub name: IDENTIFIER,
    pub methods: Vec<MethodDefinition>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct MethodDefinition {
    pub kind: MethodKind,
    pub name: IDENTIFIER,
    pub params: Vec<Param>,
    /// Always `None` for `MethodKind::Constructor` (parser rejects the annotation).
    pub return_type: Option<TypeAnnotation>,
    pub body: BlockStatement,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
pub enum MethodKind {
    Constructor,
    Method,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct SetPropertyStatement {
    pub object: Box<Expression>,
    pub property: IDENTIFIER,
    pub value: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct Let {
    pub identifier: IDENTIFIER,
    pub type_annotation: Option<TypeAnnotation>,
    pub expr: Expression,
    pub span: Span,
}

/// A function or method parameter: name plus its optional type annotation.
#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct Param {
    pub identifier: IDENTIFIER,
    pub type_annotation: Option<TypeAnnotation>,
    pub span: Span,
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self.type_annotation {
            Some(annotation) => write!(f, "{}: {}", self.identifier, annotation),
            None => write!(f, "{}", self.identifier),
        }
    }
}

/// Type annotations are parsed and carried through the AST, but every execution
/// backend erases them: see docs/type-system-design.md section 6.
#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(untagged)]
pub enum TypeAnnotation {
    Named(NamedType),
    Array(ArrayType),
    Hash(HashType),
    Function(FunctionType),
    Optional(OptionalType),
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type", rename = "NamedType")]
pub struct NamedType {
    /// `int` | `bool` | `string` | `any` | `null` | a class name
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type", rename = "ArrayType")]
pub struct ArrayType {
    pub element: Box<TypeAnnotation>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type", rename = "HashType")]
pub struct HashType {
    pub key: Box<TypeAnnotation>,
    pub value: Box<TypeAnnotation>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type", rename = "FunctionType")]
pub struct FunctionType {
    pub params: Vec<TypeAnnotation>,
    pub return_type: Box<TypeAnnotation>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type", rename = "OptionalType")]
pub struct OptionalType {
    pub inner: Box<TypeAnnotation>,
    pub span: Span,
}

impl TypeAnnotation {
    pub fn span(&self) -> &Span {
        match self {
            TypeAnnotation::Named(annotation) => &annotation.span,
            TypeAnnotation::Array(annotation) => &annotation.span,
            TypeAnnotation::Hash(annotation) => &annotation.span,
            TypeAnnotation::Function(annotation) => &annotation.span,
            TypeAnnotation::Optional(annotation) => &annotation.span,
        }
    }
}

impl fmt::Display for TypeAnnotation {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            TypeAnnotation::Named(NamedType {
                name,
                ..
            }) => write!(f, "{}", name),
            TypeAnnotation::Array(ArrayType {
                element,
                ..
            }) => write!(f, "[{}]", element),
            TypeAnnotation::Hash(HashType {
                key,
                value,
                ..
            }) => write!(f, "{{{}: {}}}", key, value),
            TypeAnnotation::Function(FunctionType {
                params,
                return_type,
                ..
            }) => {
                let params = params
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ");
                write!(f, "fn({}): {}", params, return_type)
            }
            TypeAnnotation::Optional(OptionalType {
                inner,
                ..
            }) => match **inner {
                // `fn(int): int?` would parse the `?` as part of the return
                // type, so a nullable function type needs its grouping back.
                TypeAnnotation::Function(_) => write!(f, "({})?", inner),
                _ => write!(f, "{}?", inner),
            },
        }
    }
}

/// Renders `: T` for an optional annotation, or nothing when absent.
fn format_type_annotation(annotation: &Option<TypeAnnotation>) -> String {
    match annotation {
        Some(annotation) => format!(": {}", annotation),
        None => String::new(),
    }
}

fn format_params(params: &[Param]) -> String {
    return params
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>()
        .join(", ");
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct ReturnStatement {
    pub argument: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct DebuggerStatement {
    pub span: Span,
}

impl fmt::Display for Statement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Statement::Let(Let {
                identifier: id,
                type_annotation,
                expr,
                ..
            }) => {
                write!(f, "let {}{} = {};", id.name, format_type_annotation(type_annotation), expr)
            }
            Statement::Return(ReturnStatement {
                argument,
                ..
            }) => {
                write!(f, "return {};", argument)
            }
            Statement::Class(class) => {
                let methods = class
                    .methods
                    .iter()
                    .map(|method| method.to_string())
                    .collect::<Vec<_>>()
                    .join("");
                write!(f, "class {} {{{}}}", class.name, methods)
            }
            Statement::SetProperty(set) => {
                write!(f, "{}.{} = {};", set.object, set.property, set.value)
            }
            Statement::Debugger(_) => write!(f, "debugger;"),
            Statement::Expr(expr) => write!(f, "{}", expr),
        }
    }
}

impl fmt::Display for MethodDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{}({}){} {{{}}}",
            self.name,
            format_params(&self.params),
            format_type_annotation(&self.return_type),
            self.body
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub struct BlockStatement {
    pub body: Vec<Statement>,
    pub span: Span,
}

impl fmt::Display for BlockStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", format_statements(&self.body))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(untagged)]
pub enum Expression {
    IDENTIFIER(IDENTIFIER),
    LITERAL(Literal), // need to flatten
    PREFIX(UnaryExpression),
    INFIX(BinaryExpression),
    IF(IF),
    FUNCTION(FunctionDeclaration),
    FunctionCall(FunctionCall),
    Index(Index),
    This(ThisExpression),
    Property(PropertyExpression),
    New(NewExpression),
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct ThisExpression {
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct PropertyExpression {
    pub object: Box<Expression>,
    pub property: IDENTIFIER,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct NewExpression {
    pub callee: IDENTIFIER,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct IDENTIFIER {
    pub name: String,
    pub span: Span,
}

impl fmt::Display for IDENTIFIER {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct UnaryExpression {
    pub op: Token,
    pub operand: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct BinaryExpression {
    pub op: Token,
    pub left: Box<Expression>,
    pub right: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct IF {
    pub condition: Box<Expression>,
    pub consequent: BlockStatement,
    pub alternate: Option<BlockStatement>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct FunctionDeclaration {
    pub params: Vec<Param>,
    pub return_type: Option<TypeAnnotation>,
    pub body: BlockStatement,
    pub span: Span,
    pub name: String,
}

// function can be Identifier or FunctionLiteral (think iife)
#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct FunctionCall {
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[serde(tag = "type")]
pub struct Index {
    pub object: Box<Expression>,
    pub index: Box<Expression>,
    pub span: Span,
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Expression::IDENTIFIER(IDENTIFIER {
                name: id,
                ..
            }) => write!(f, "{}", id),
            Expression::LITERAL(l) => write!(f, "{}", l),
            Expression::PREFIX(UnaryExpression {
                op,
                operand: expr,
                ..
            }) => {
                write!(f, "({}{})", op.kind, expr)
            }
            Expression::INFIX(BinaryExpression {
                op,
                left,
                right,
                ..
            }) => {
                write!(f, "({} {} {})", left, op.kind, right)
            }
            Expression::IF(IF {
                condition,
                consequent,
                alternate,
                ..
            }) => {
                if let Some(else_block) = alternate {
                    write!(f, "if {} {{ {} }} else {{ {} }}", condition, consequent, else_block,)
                } else {
                    write!(f, "if {} {{ {} }}", condition, consequent,)
                }
            }
            Expression::FUNCTION(FunctionDeclaration {
                name,
                params,
                return_type,
                body,
                ..
            }) => {
                write!(
                    f,
                    "fn {}({}){} {{ {} }}",
                    name,
                    format_params(params),
                    format_type_annotation(return_type),
                    body
                )
            }
            Expression::FunctionCall(FunctionCall {
                callee,
                arguments,
                ..
            }) => {
                write!(f, "{}({})", callee, format_expressions(arguments))
            }
            Expression::Index(Index {
                object,
                index,
                ..
            }) => {
                write!(f, "({}[{}])", object, index)
            }
            Expression::This(_) => write!(f, "this"),
            Expression::Property(PropertyExpression {
                object,
                property,
                ..
            }) => write!(f, "{}.{}", object, property),
            Expression::New(NewExpression {
                callee,
                arguments,
                ..
            }) => write!(f, "new {}({})", callee, format_expressions(arguments)),
        }
    }
}

impl Statement {
    pub fn span(&self) -> &Span {
        match self {
            Statement::Let(statement) => &statement.span,
            Statement::Return(statement) => &statement.span,
            Statement::Class(statement) => &statement.span,
            Statement::SetProperty(statement) => &statement.span,
            Statement::Debugger(statement) => &statement.span,
            Statement::Expr(expression) => expression.span(),
        }
    }
}

impl Expression {
    pub fn span(&self) -> &Span {
        match self {
            Expression::IDENTIFIER(identifier) => &identifier.span,
            Expression::LITERAL(literal) => literal.span(),
            Expression::PREFIX(expression) => &expression.span,
            Expression::INFIX(expression) => &expression.span,
            Expression::IF(expression) => &expression.span,
            Expression::FUNCTION(expression) => &expression.span,
            Expression::FunctionCall(expression) => &expression.span,
            Expression::Index(expression) => &expression.span,
            Expression::This(expression) => &expression.span,
            Expression::Property(expression) => &expression.span,
            Expression::New(expression) => &expression.span,
        }
    }
}

impl Literal {
    pub fn span(&self) -> &Span {
        match self {
            Literal::Integer(literal) => &literal.span,
            Literal::Boolean(literal) => &literal.span,
            Literal::String(literal) => &literal.span,
            Literal::Array(literal) => &literal.span,
            Literal::Hash(literal) => &literal.span,
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
            Literal::Integer(Integer {
                raw: i,
                ..
            }) => write!(f, "{}", i),
            Literal::Boolean(Boolean {
                raw: b,
                ..
            }) => write!(f, "{}", b),
            Literal::String(StringType {
                raw: s,
                ..
            }) => write!(f, "\"{}\"", s),
            Literal::Array(Array {
                elements: e,
                ..
            }) => write!(f, "[{}]", format_expressions(e)),
            Literal::Hash(Hash {
                elements: map,
                ..
            }) => {
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

fn format_statements(statements: &[Statement]) -> String {
    return statements
        .iter()
        .map(|stmt| stmt.to_string())
        .collect::<Vec<String>>()
        .join("");
}

fn format_expressions(exprs: &[Expression]) -> String {
    return exprs
        .iter()
        .map(|stmt| stmt.to_string())
        .collect::<Vec<String>>()
        .join(", ");
}
