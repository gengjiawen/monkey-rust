use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Formatter;

#[derive(Clone, Debug, Eq, Hash, Ord, Serialize, Deserialize, PartialOrd, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, Hash, Ord, Serialize, Deserialize, PartialOrd, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "start: {}, end: {}, kind: {}",
            self.span.start, self.span.end, self.kind
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, Serialize, Deserialize, PartialOrd, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum TokenKind {
    ILLEGAL,
    EOF,

    // Identifiers + literals
    IDENTIFIER { name: String },
    INT(i64),
    STRING(String),

    // Operators
    ASSIGN,   // =
    PLUS,     // +
    MINUS,    // -
    BANG,     // !
    ASTERISK, // *
    SLASH,    // /

    LT, // <
    GT, // >

    EQ,    // ==
    NotEq, // !=

    // delimiters
    COMMA,
    SEMICOLON,
    COLON,

    LPAREN,
    RPAREN,
    LBRACE,   // {
    RBRACE,   // }
    LBRACKET, // [
    RBRACKET, // ]

    // keywords
    FUNCTION,
    LET,
    TRUE,
    FALSE,
    IF,
    ELSE,
    RETURN,
}

pub fn lookup_identifier(identifier: &str) -> TokenKind {
    match identifier {
        "fn" => TokenKind::FUNCTION,
        "let" => TokenKind::LET,
        "true" => TokenKind::TRUE,
        "false" => TokenKind::FALSE,
        "if" => TokenKind::IF,
        "else" => TokenKind::ELSE,
        "return" => TokenKind::RETURN,
        _ => TokenKind::IDENTIFIER {
            name: identifier.to_string(),
        },
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::IDENTIFIER { name } => write!(f, "{}", name),
            TokenKind::INT(i) => write!(f, "{}", i),
            TokenKind::STRING(s) => write!(f, "{}", s),
            TokenKind::ASSIGN => write!(f, "="),
            TokenKind::PLUS => write!(f, "+"),
            TokenKind::MINUS => write!(f, "-"),
            TokenKind::BANG => write!(f, "!"),
            TokenKind::ASTERISK => write!(f, "*"),
            TokenKind::SLASH => write!(f, "/"),
            TokenKind::LT => write!(f, "<"),
            TokenKind::GT => write!(f, ">"),
            TokenKind::EQ => write!(f, "=="),
            TokenKind::NotEq => write!(f, "!="),
            TokenKind::COMMA => write!(f, ","),
            TokenKind::SEMICOLON => write!(f, ";"),
            TokenKind::LPAREN => write!(f, "("),
            TokenKind::RPAREN => write!(f, ")"),
            TokenKind::LBRACE => write!(f, "{{"),
            TokenKind::RBRACE => write!(f, "}}"),
            TokenKind::LBRACKET => write!(f, "["),
            TokenKind::RBRACKET => write!(f, "]"),
            TokenKind::FUNCTION => write!(f, "fn"),
            TokenKind::LET => write!(f, "let"),
            TokenKind::TRUE => write!(f, "true"),
            TokenKind::FALSE => write!(f, "false"),
            TokenKind::IF => write!(f, "if"),
            TokenKind::ELSE => write!(f, "else"),
            TokenKind::RETURN => write!(f, "return"),
            TokenKind::ILLEGAL => write!(f, "ILLEGAL"),
            TokenKind::EOF => write!(f, "EOF"),
            TokenKind::COLON => write!(f, ":"),
        }
    }
}
