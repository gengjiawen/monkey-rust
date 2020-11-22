use lexer::token::Token;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Precedence {
    LOWEST,
    EQUALS, // ==
    LessGreater, // > or <
    SUM, // + or =
    PRODUCT, // * or /
    PREFIX, // -X or !X
    CALL, // myFunction(x)
    INDEX, // array[index]
}

pub fn precedences(token: &Token) -> Precedence {
    match token {
        Token::EQ => Precedence::EQUALS,
        Token::NOT_EQ => Precedence::EQUALS,
        Token::LT => Precedence::LessGreater,
        Token::GT => Precedence::LessGreater,
        Token::PLUS => Precedence::SUM,
        Token::MINUS => Precedence::SUM,
        Token::ASTERISK => Precedence::PRODUCT,
        Token::SLASH => Precedence::PRODUCT,
        Token::LPAREN => Precedence::CALL,
        Token::LBRACE => Precedence::INDEX,
        _ => Precedence::LOWEST,
    }
}