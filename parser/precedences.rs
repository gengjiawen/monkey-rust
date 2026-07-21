use lexer::token::TokenKind;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest,
    Equals,      // ==
    LessGreater, // > or <
    Sum,         // + or =
    Product,     // * or /
    Prefix,      // -X or !X
    Postfix,     // call, index, property
}

pub fn get_token_precedence(token: &TokenKind) -> Precedence {
    match token {
        TokenKind::EQ => Precedence::Equals,
        TokenKind::NotEq => Precedence::Equals,
        TokenKind::LT => Precedence::LessGreater,
        TokenKind::GT => Precedence::LessGreater,
        TokenKind::PLUS => Precedence::Sum,
        TokenKind::MINUS => Precedence::Sum,
        TokenKind::ASTERISK => Precedence::Product,
        TokenKind::SLASH => Precedence::Product,
        TokenKind::LPAREN | TokenKind::LBRACKET | TokenKind::DOT => Precedence::Postfix,
        _ => Precedence::Lowest,
    }
}
