extern crate derive_more;

use derive_more::{Display};

#[derive(Clone, Debug, Display, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub enum Token {
    ILLEGAL,
    EOF,

    // Identifiers + literals
    #[display(fmt = "IDENTIFIER: {}", _0)]
    IDENTIFIER(String),
    #[display(fmt = "INT: {}", _0)]
    INT(i64),

    // Operators
    ASSIGN, // =
    PLUS, // +
    MINUS, // -
    BANG, // !
    ASTERISK, // *
    SLASH, // /

    LT, // <
    RT, // >

    EQ, // ==
    NOT_EQ, // !=

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
        "false" => Token::TRUE,
        "if" => Token::IF,
        "else" => Token::ELSE,
        "return" => Token::RETURN,
        _ => Token::IDENTIFIER(identifier.to_string())
    }
}


