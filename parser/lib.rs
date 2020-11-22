mod ast;
mod precedences;

use lexer::token::{Token};
use lexer::Lexer;
use crate::ast::{Program, Statement, Expression, Node, Literal};

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
        Parser {
            lexer,
            current_token: cur,
            peek_token: next,
            errors,
        }
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
        let name = match self.parse_ident_name() {
            Ok(name) => name,
            Err(e) => return Err(e)
        };
        self.expect_peek(&Token::ASSIGN)?;
        self.next_token();

        // todo
        // let value = self.parse_expression()?;
        let value = match self.current_token {
            Token::INT(i) => Expression::LITERAL(Literal::Integer(i)),
            _ => {
                return Err("unexpected token here".to_string());
            }
        };


        if self.peek_token_is(&Token::SEMICOLON) {
            self.next_token();
        }

        return Ok(Statement::Let(name, value));
    }
    fn parse_ident_name(&self) -> Result<String, ParseError> {
        if let Token::IDENTIFIER(ref id) = self.current_token {
            Ok(id.to_string())
        } else {
            Err(format!("expected identifier, got: {}", &self.current_token))
        }
    }
    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        unimplemented!()
    }
    fn parse_expression_statement(&self) -> Result<Statement, ParseError> {
        unimplemented!()
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        self.next_token();

        // todo
        // let value = self.parse_expression()?;
        let value = match self.current_token {
            Token::INT(i) => Expression::LITERAL(Literal::Integer(i)),
            _ => {
                return Err("unexpected token here".to_string());
            }
        };


        if self.peek_token_is(&Token::SEMICOLON) {
            self.next_token();
        }

        return Ok(Statement::Return(value));
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
            // ("let y=5;", "let y = true;"),
            // ("let foo=y;", "lex foo = y;"),
        ];

        verify_program(&let_tests);
    }

    #[test]
    fn parse_return_statement() {
        let let_tests = [
            ("return 5", "return 5"),
        ];

        verify_program(&let_tests);
    }
}
