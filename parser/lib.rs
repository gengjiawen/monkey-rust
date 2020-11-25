mod ast;
mod precedences;

use lexer::token::{Token};
use lexer::Lexer;
use crate::ast::{Program, Statement, Expression, Node, Literal};
use crate::precedences::{Precedence, get_token_precedence};

type ParseError = String;
type ParseErrors = Vec<ParseError>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_token: Token,
    errors: ParseErrors,
}

impl<'a> Parser<'a> {
    pub fn new(mut lexer: Lexer<'a>) -> Parser<'a> {
        let cur = lexer.next_token();
        let next = lexer.next_token();
        let errors = Vec::new();
        // in strict sense, rust can be as classic go pattern, but it requires more work
        // so let's just use pattern matching
        // ```rust
        // type PrefixParseFn = fn() -> Result<Expression, ParseError>;
        // type InfixParseFn = fn(Expression) -> Result<Expression, ParseError>;
        // let prefix_parse_fns = HashMap::new();
        // let infix_parse_fns = HashMap::new();
        // ```

        let mut p = Parser {
            lexer,
            current_token: cur,
            peek_token: next,
            errors,
        };

        return p;
    }

    fn next_token(&mut self) {
        // todo remove clone
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next_token();
    }

    fn current_token_is(&mut self, token: &Token) -> bool {
        self.current_token == *token
    }

    fn peek_token_is(&mut self, token: &Token) -> bool {
        self.peek_token == *token
    }

    fn expect_peek(&mut self, token: &Token) -> Result<(), ParseError> {
        self.next_token();
        if self.current_token == *token {
            Ok(())
        } else {
            let e = format!("expected token: {}, got: {}", token, self.peek_token);
            Err(e)
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseErrors> {
        let mut program = Program::new();
        while !self.current_token_is(&Token::EOF) {
            match self.parse_statement() {
                Ok(stmt) => program.statements.push(stmt),
                Err(e) => self.errors.push(e),
            }
            self.next_token();
        }

        if self.errors.is_empty() {
            return Ok(program);
        } else {
            return Err(self.errors.clone());
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.current_token {
            Token::LET => self.parse_let_statement(),
            Token::RETURN => self.parse_return_statement(),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_let_statement(&mut self) -> Result<Statement, ParseError> {
        self.next_token();
        let name = match self.current_token {
            Token::IDENTIFIER(ref id) => id.to_string(),
            _ => return Err(format!("not an identifier"))
        };
        self.expect_peek(&Token::ASSIGN)?;
        self.next_token();

        let value = self.parse_expression(Precedence::LOWEST)?;

        if self.peek_token_is(&Token::SEMICOLON) {
            self.next_token();
        }

        return Ok(Statement::Let(name, value));
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        self.next_token();

        let value = self.parse_expression(Precedence::LOWEST)?;

        if self.peek_token_is(&Token::SEMICOLON) {
            self.next_token();
        }

        return Ok(Statement::Return(value));
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        let expr = self.parse_expression(Precedence::LOWEST)?;
        if self.peek_token_is(&Token::SEMICOLON) {
            self.next_token();
        }

        Ok(Statement::Expr(expr))
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Result<Expression, ParseError> {
        let left = self.parse_prefix_expression(precedence)?;
        while self.peek_token != Token::SEMICOLON && precedence < get_token_precedence(&self.peek_token) {
            return Ok(self.parse_infix_expression(left)?);
        }

        Ok(left)
    }

    fn parse_prefix_expression(&mut self, precedence: Precedence) -> Result<Expression, ParseError> {
        // this is prefix fn map :)
        match &self.current_token {
            Token::IDENTIFIER(ref id) => return Ok(Expression::IDENTIFIER(id.clone())),
            Token::INT(i) => return Ok(Expression::LITERAL(Literal::Integer(*i))),
            Token::STRING(s) => return Ok(Expression::LITERAL(Literal::String(s.to_string()))),
            b @ Token::TRUE| b @ Token::FALSE => return Ok(Expression::LITERAL(Literal::Boolean(*b == Token::TRUE))),
            Token::BANG | Token::MINUS => {
                let prefix_op = self.current_token.clone();
                self.next_token();
                let expr = self.parse_expression(Precedence::PREFIX)?;
                return Ok(Expression::PREFIX(prefix_op, Box::new(expr)));
            },
            _ => {
                Err(format!("no prefix function for token: {}", self.current_token))
            }
        }
    }

    fn parse_infix_expression(&mut self, left: Expression) -> Result<Expression, ParseError> {
        match self.peek_token {
            Token::PLUS |
            Token::MINUS |
            Token::ASTERISK |
            Token::SLASH |
            Token::EQ |
            Token::NotEq |
            Token::LT |
            Token::GT => {
                self.next_token();
                let infix_op = self.current_token.clone();
                let precedence_value = get_token_precedence(&self.current_token);
                self.next_token();
                let right = self.parse_expression(precedence_value)?;
                return Ok(Expression::INFIX(infix_op, Box::new(left), Box::new(right)));
            }
            _ => {
                return Ok(left);
            }

        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // helper function
    pub fn parse(input: &str) -> Result<Node, ParseErrors> {
        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program()?;

        Ok(Node::Program(program))
    }

    fn verify_program(test_cases: &[(&str, &str)]) {

        for (input, expected) in test_cases {
            let parsed = parse(input).unwrap().to_string();
            assert_eq!(&format!("{}", parsed), expected);
        }
    }


    #[test]
    fn parse_let_statement() {
        let let_tests = [
            ("let x=5;", "let x = 5;"),
            ("let y=true;", "let y = true;"),
            ("let foo=y;", "let foo = y;"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_return_statement() {
        let let_tests = [
            ("return 5", "return 5;"),
            ("return true;", "return true;"),
            ("return foobar;", "return foobar;"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_prefix_expression() {
        let let_tests = [
            ("-15;", "(-15)"),
            ("!5;", "(!5)"),
            ("!foobar;", "(!foobar)"),
            ("-foobar;", "(-foobar)"),
            ("!true;", "(!true)"),
            ("!false;", "(!false)"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_infix_expression() {
        let let_tests = [
            ("5 + 5;", "(5 + 5)"),
            ("5 - 5;", "(5 - 5)"),
            ("5 * 5;", "(5 * 5)"),
            ("5 / 5;", "(5 / 5)"),
            ("5 > 5;", "(5 > 5)"),
            ("5 < 5;", "(5 < 5)"),
            ("5 == 5;", "(5 == 5)"),
            ("5 != 5;", "(5 != 5)"),
            ("foobar + barfoo;", "(foobar + barfoo)"),
            ("foobar - barfoo;", "(foobar - barfoo)"),
            ("foobar * barfoo;", "(foobar * barfoo)"),
            ("foobar / barfoo;", "(foobar / barfoo)"),
            ("foobar > barfoo;", "(foobar > barfoo)"),
            ("foobar < barfoo;", "(foobar < barfoo)"),
            ("foobar == barfoo;", "(foobar == barfoo)"),
            ("foobar != barfoo;", "(foobar != barfoo)"),
            ("true == true", "(true == true)"),
            ("true != false", "(true != false)"),
            ("false == false", "(false == false)"),
        ];

        verify_program(&let_tests);
    }
}
