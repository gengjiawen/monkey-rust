use crate::lexer::token::Token;

pub mod token;


pub struct Lexer<'a> {
    input: &'a str,
    position: usize,
    read_position: usize,
    ch: char,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut l = Lexer {
            input,
            position: 0,
            read_position: 0,
            ch: 0 as char,
        };

        l.read_char();
        return l;
    }

    fn read_char(&mut self) {
        if self.read_position >= self.input.len() {
            self.ch = 0 as char
        } else {
            if let Some(ch) = self.input.chars().nth(self.read_position) {
                self.ch = ch;
            } else {
                panic!("read out of range")
            }
        }

        self.position = self.read_position;
        self.read_position += 1;
    }

    fn next_token(&mut self) -> Token {
        let t = match self.ch {
            '=' => Token::ASSIGN,
            ';' => Token::SEMICOLON,
            '(' => Token::LPAREN,
            ')' => Token::RPAREN,
            ',' => Token::COMMA,
            '+' => Token::PLUS,
            '{' => Token::LPAREN,
            '}' => Token::RPAREN,
           '\u{0}' => Token::EOF,
            _ => {
                Token::ILLEGAL
            }
        };

        self.read_char();
        return t;
    }
}


#[cfg(test)]
mod tests {
    use crate::lexer::Lexer;
    use crate::lexer::token::Token;
    use insta::assert_debug_snapshot;

    #[test]
    fn test_lexer_simple() {
        let mut l = Lexer::new("=+(){},;");
        let mut token_vs: Vec<Token> = vec![];
        loop {
            let t = l.next_token();
            if t == Token::EOF {
                token_vs.push(t);
                break
            } else {
                token_vs.push(t);
            }
        }

        assert_debug_snapshot!(token_vs)
    }
}
