mod ast;
mod precedences;

use lexer::token::{Token};
use lexer::Lexer;
use crate::ast::{Program, Statement, Expression, Node, Literal, BlockStatement};
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

        let p = Parser {
            lexer,
            current_token: cur,
            peek_token: next,
            errors,
        };

        return p;
    }

    fn next_token(&mut self) {
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
            let e = format!("expected token: {}, got: {}", token, self.current_token);
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
        let mut left = self.parse_prefix_expression()?;
        while self.peek_token != Token::SEMICOLON && precedence < get_token_precedence(&self.peek_token) {
            match self.parse_infix_expression(&left) {
                Some(infix) => left = infix?,
                None => return Ok(left),
            }
        }

        Ok(left)
    }

    fn parse_prefix_expression(&mut self) -> Result<Expression, ParseError> {
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
            Token::LPAREN => {
                self.next_token();
                let expr = self.parse_expression(Precedence::LOWEST);
                self.expect_peek(&Token::RPAREN)?;
                return expr
            },
            Token::IF => self.parse_if_expression(),
            Token::FUNCTION => self.parse_fn_expression(),
            _ => {
                Err(format!("no prefix function for token: {}", self.current_token))
            }
        }
    }

    fn parse_infix_expression(&mut self, left: &Expression) -> Option<Result<Expression, ParseError>> {
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
                let right: Expression = self.parse_expression(precedence_value).unwrap();
                return Some(Ok(Expression::INFIX(infix_op, Box::new(left.clone()), Box::new(right))));
            },
            _ => None,

        }
    }

    fn parse_if_expression(&mut self) -> Result<Expression, ParseError> {
        self.expect_peek(&Token::LPAREN)?;
        self.next_token();

        let condition = self.parse_expression(Precedence::LOWEST)?;
        self.expect_peek(&Token::RPAREN)?;
        self.expect_peek(&Token::LBRACE)?;

        let consequence = self.parse_block_statement()?;

        let alternative = if self.peek_token_is(&Token::ELSE) {
            self.next_token();
            self.expect_peek(&Token::LBRACE)?;
            Some(self.parse_block_statement()?)
        } else {
            None
        };

        return Ok(Expression::IF(Box::new(condition), consequence, alternative))
    }

    fn parse_block_statement(&mut self) -> Result<BlockStatement, ParseError> {
        self.next_token();
        let mut block_statement = Vec::new();

        while !self.current_token_is(&Token::RBRACE) && !self.current_token_is(&Token::EOF) {
            if let Ok(statement) = self.parse_statement() {
                block_statement.push(statement)
            }

            self.next_token();
        }

        Ok(BlockStatement::new(block_statement))
    }

    fn parse_fn_expression(&mut self) -> Result<Expression, ParseError> {
        self.expect_peek(&Token::LPAREN)?;

        let params = self.parse_fn_parameters()?;

        self.expect_peek(&Token::LBRACE)?;

        let function_body = self.parse_block_statement()?;

        Ok(Expression::FUNCTION(params, function_body))
    }
    
    fn parse_fn_parameters(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();
        if self.peek_token_is(&Token::RPAREN) {
            self.next_token();
            return Ok(params);
        }

        self.next_token();

        match &self.current_token {
            Token::IDENTIFIER(ref id) => params.push(id.clone()),
            token => return Err(format!("expected function params  to be an identifier, got {}", token))
        }

        while self.peek_token_is(&Token::COMMA) {
           self.next_token();
           self.next_token();
            match &self.current_token {
                Token::IDENTIFIER(ref id) => params.push(id.clone()),
                token => return Err(format!("expected function params  to be an identifier, got {}", token))
            }
        }

        self.expect_peek(&Token::RPAREN)?;

        return Ok(params)
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

    #[test]
    fn parse_op_expression() {
        let tt = [
            ("-a * b", "((-a) * b)"),
            ("!-a", "(!(-a))"),
            ("a + b + c", "((a + b) + c)"),
            ("a + b - c", "((a + b) - c)"),
            ("a * b * c", "((a * b) * c)"),
            ("a * b / c", "((a * b) / c)"),
            ("a + b / c", "(a + (b / c))"),
            ("a + b * c + d / e - f", "(((a + (b * c)) + (d / e)) - f)"),
            ("3 + 4; -5 * 5", "(3 + 4)((-5) * 5)"),
            ("5 > 4 == 3 < 4", "((5 > 4) == (3 < 4))"),
            ("5 < 4 != 3 > 4", "((5 < 4) != (3 > 4))"),
            (
                "3 + 4 * 5 == 3 * 1 + 4 * 5",
                "((3 + (4 * 5)) == ((3 * 1) + (4 * 5)))",
            ),
            ("true", "true"),
            ("false", "false"),
            ("3 > 5 == false", "((3 > 5) == false)"),
            ("3 < 5 == true", "((3 < 5) == true)"),
        ];

        verify_program(&tt);
    }

    #[test]
    fn parse_brace_expression() {
        let tt = [
            ("1 + (2 + 3) + 4", "((1 + (2 + 3)) + 4)"),
            ("(5 + 5) * 2", "((5 + 5) * 2)"),
            ("2 / (5 + 5)", "(2 / (5 + 5))"),
            ("(5 + 5) * 2 * (5 + 5)", "(((5 + 5) * 2) * (5 + 5))"),
            ("-(5 + 5)", "(-(5 + 5))"),
            ("!(true == true)", "(!(true == true))"),
        ];

        verify_program(&tt);
    }

    #[test]
    fn test_if_expression() {
        let tt = [("if (x < y) { x }", "if (x < y) { x }")];
        verify_program(&tt);
    }

    #[test]
    fn test_if_else_expression() {
        let tt = [("if (x < y) { x } else { y }", "if (x < y) { x } else { y }")];
        verify_program(&tt);
    }

    #[test]
    fn test_fn_else_expression() {
        let tt = [
            ("fn() {};", "fn() {  }"),
            ("fn(x) {};", "fn(x) {  }"),
            ("fn(x, y, z) { x };", "fn(x, y, z) { x }"),
        ];
        verify_program(&tt);
    }

}
