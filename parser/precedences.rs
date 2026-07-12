use lexer::token::TokenKind;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum Precedence {
    LOWEST,
    EQUALS,      // ==
    LessGreater, // > or <
    SUM,         // + or =
    PRODUCT,     // * or /
    PREFIX,      // -X or !X
    POSTFIX,     // call, index, property
}

pub fn get_token_precedence(token: &TokenKind) -> Precedence {
    match token {
        TokenKind::EQ => Precedence::EQUALS,
        TokenKind::NotEq => Precedence::EQUALS,
        TokenKind::LT => Precedence::LessGreater,
        TokenKind::GT => Precedence::LessGreater,
        TokenKind::PLUS => Precedence::SUM,
        TokenKind::MINUS => Precedence::SUM,
        TokenKind::ASTERISK => Precedence::PRODUCT,
        TokenKind::SLASH => Precedence::PRODUCT,
        TokenKind::LPAREN | TokenKind::LBRACKET | TokenKind::DOT => Precedence::POSTFIX,
        _ => Precedence::LOWEST,
    }
}
