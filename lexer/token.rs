use std::fmt;
use std::fmt::Formatter;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum Token {
    ILLEGAL,
    EOF,

    // Identifiers + literals
    IDENTIFIER(String),
    INT(i64),
    STRING(String),

    // Operators
    ASSIGN, // =
    PLUS, // +
    MINUS, // -
    BANG, // !
    ASTERISK, // *
    SLASH, // /

    LT, // <
    GT, // >

    EQ, // ==
    NotEq, // !=

    // delimiters
    COMMA,
    SEMICOLON,

    LPAREN,
    RPAREN,
    LBRACE, // {
    RBRACE, // }
    LBRACKET, // [
    RBRACKET, // ]

    // keywords
    FUNCTION,
    LET,
    TRUE,
    FALSE,
    IF,
    ELSE,
    RETURN
}

pub fn lookup_identifier(identifier: &str) -> Token {
    match identifier {
        "fn" => Token::FUNCTION,
        "let" => Token::LET,
        "true" => Token::TRUE,
        "false" => Token::FALSE,
        "if" => Token::IF,
        "else" => Token::ELSE,
        "return" => Token::RETURN,
        _ => Token::IDENTIFIER(identifier.to_string())
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Token::IDENTIFIER(id) => write!(f, "{}", id),
            Token::INT(i) => write!(f, "{}", i),
            Token::STRING(s) => write!(f, "{}", s),
            Token::ASSIGN => write!(f, "="),
            Token::PLUS => write!(f, "+"),
            Token::MINUS => write!(f, "-"),
            Token::BANG => write!(f, "!"),
            Token::ASTERISK => write!(f, "*"),
            Token::SLASH => write!(f, "/"),
            Token::LT => write!(f, "<"),
            Token::GT => write!(f, ">"),
            Token::EQ => write!(f, "=="),
            Token::NotEq => write!(f, "!="),
            Token::COMMA => write!(f, ","),
            Token::SEMICOLON => write!(f, ";"),
            Token::LPAREN => write!(f, "("),
            Token::RPAREN => write!(f, ")"),
            Token::LBRACE => write!(f, "{{"),
            Token::RBRACE => write!(f, "}}"),
            Token::LBRACKET => write!(f, "["),
            Token::RBRACKET => write!(f, "]"),
            Token::FUNCTION => write!(f, "fn"),
            Token::LET => write!(f, "let"),
            Token::TRUE => write!(f, "true"),
            Token::FALSE => write!(f, "false"),
            Token::IF => write!(f, "if"),
            Token::ELSE => write!(f, "else"),
            Token::RETURN => write!(f, "return"),
            Token::ILLEGAL => write!(f, "ILLEGAL"),
            Token::EOF => write!(f, "EOF"),
        }
    }
}
