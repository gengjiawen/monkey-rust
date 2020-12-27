pub mod ast;
mod precedences;
mod ast_tree_test;

pub extern crate lexer;

use lexer::token::{TokenKind, Token, Span};
use lexer::Lexer;
use crate::ast::{Program, Statement, Expression, Node, Literal, BlockStatement, Let, Integer, Boolean, StringType, Array, Hash, UnaryExpression, BinaryExpression, IDENTIFIER, IF, FunctionDeclaration};
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

    fn current_token_is(&mut self, token: &TokenKind) -> bool {
        self.current_token.kind == *token
    }

    fn peek_token_is(&mut self, token: &TokenKind) -> bool {
        self.peek_token.kind == *token
    }

    fn expect_peek(&mut self, token: &TokenKind) -> Result<(), ParseError> {
        self.next_token();
        if self.current_token.kind == *token {
            Ok(())
        } else {
            let e = format!("expected token: {} got: {}", token, self.current_token);
            Err(e)
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseErrors> {
        let mut program = Program::new();
        while !self.current_token_is(&TokenKind::EOF) {
            match self.parse_statement() {
                Ok(stmt) => program.body.push(stmt),
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
        match self.current_token.kind {
            TokenKind::LET => self.parse_let_statement(),
            TokenKind::RETURN => self.parse_return_statement(),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_let_statement(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_token.span.start;
        self.next_token();

        let name = self.current_token.clone();
        match &self.current_token.kind {
            TokenKind::IDENTIFIER { name: _ } => {}
            _ => return Err(format!("{} not an identifier", self.current_token))
        };

        self.expect_peek(&TokenKind::ASSIGN)?;
        self.next_token();

        let value = self.parse_expression(Precedence::LOWEST)?.0;

        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }

        let end = self.current_token.span.end;

        return Ok(Statement::Let(Let {
            identifier: name,
            expr: value,
            span: Span {
                start,
                end,
            },
        }));
    }

    fn parse_return_statement(&mut self) -> Result<Statement, ParseError> {
        self.next_token();

        let value = self.parse_expression(Precedence::LOWEST)?.0;

        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }

        return Ok(Statement::Return(value));
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        let expr = self.parse_expression(Precedence::LOWEST)?.0;
        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }

        Ok(Statement::Expr(expr))
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Result<(Expression, Span), ParseError> {
        let start = self.current_token.span.start;
        let mut left = self.parse_prefix_expression()?;
        while self.peek_token.kind != TokenKind::SEMICOLON && precedence < get_token_precedence(&self.peek_token.kind) {
            let left_start = self.current_token.span.start;
            match self.parse_infix_expression(
                &left,
                left_start
            ) {
                Some(infix) => {
                    left = infix?
                }
                None => return Ok((left, Span {
                    start,
                    end: self.current_token.span.end
                })),
            }
        }

        let end = self.current_token.span.end;

        Ok((left, Span {
            start,
            end,
        }))
    }

    fn parse_prefix_expression(&mut self) -> Result<Expression, ParseError> {
        // this is prefix fn map :)
        match &self.current_token.kind {
            TokenKind::IDENTIFIER { name } => return Ok(Expression::IDENTIFIER(
                IDENTIFIER {
                    name: name.clone(),
                    span: self.current_token.clone().span,
                })),
            TokenKind::INT(i) => return Ok(Expression::LITERAL(Literal::Integer(
                Integer {
                    raw: *i,
                    span: self.current_token.clone().span,
                }))),
            TokenKind::STRING(s) => return Ok(Expression::LITERAL(Literal::String(
                StringType {
                    raw: s.to_string(),
                    span: self.current_token.clone().span,
                }))),
            b @ TokenKind::TRUE | b @ TokenKind::FALSE => return Ok(Expression::LITERAL(Literal::Boolean(
                Boolean {
                    raw: *b == TokenKind::TRUE,
                    span: self.current_token.clone().span,
                }))),
            TokenKind::BANG | TokenKind::MINUS => {
                let start = self.current_token.span.start;
                let prefix_op = self.current_token.clone();
                self.next_token();
                let (expr, span) = self.parse_expression(Precedence::PREFIX)?;
                return Ok(Expression::PREFIX(UnaryExpression {
                    op: prefix_op,
                    operand: Box::new(expr),
                    span: Span {
                        start,
                        end: span.end
                    }
                }));
            }
            TokenKind::LPAREN => {
                self.next_token();
                let expr = self.parse_expression(Precedence::LOWEST)?.0;
                self.expect_peek(&TokenKind::RPAREN)?;
                return Ok(expr);
            }
            TokenKind::IF => self.parse_if_expression(),
            TokenKind::FUNCTION => self.parse_fn_expression(),
            TokenKind::LBRACKET => {
                let (elements, span) = self.parse_expression_list(&TokenKind::RBRACKET)?;
                return Ok(Expression::LITERAL(Literal::Array(
                    Array {
                        elements,
                        span
                    })));
            }
            TokenKind::LBRACE => self.parse_hash_expression(),
            _ => {
                Err(format!("no prefix function for token: {}", self.current_token))
            }
        }
    }

    fn parse_infix_expression(&mut self, left: &Expression, left_start: usize) -> Option<Result<Expression, ParseError>> {
        match self.peek_token.kind {
            TokenKind::PLUS |
            TokenKind::MINUS |
            TokenKind::ASTERISK |
            TokenKind::SLASH |
            TokenKind::EQ |
            TokenKind::NotEq |
            TokenKind::LT |
            TokenKind::GT => {
                self.next_token();
                let infix_op = self.current_token.clone();
                let precedence_value = get_token_precedence(&self.current_token.kind);
                self.next_token();
                let (right, span) = self.parse_expression(precedence_value).unwrap();
                return Some(Ok(Expression::INFIX(BinaryExpression {
                    op: infix_op,
                    left: Box::new(left.clone()),
                    right: Box::new(right),
                    span: Span {
                        start: left_start,
                        end: span.end
                    }
                })));
            }
            TokenKind::LPAREN => {
                self.next_token();
                return Some(self.parse_fn_call_expression(left.clone()));
            }
            TokenKind::LBRACKET => {
                self.next_token();
                return Some(self.parse_index_expression(left.clone()));
            }
            _ => None
        }
    }

    fn parse_if_expression(&mut self) -> Result<Expression, ParseError> {
        let start = self.current_token.span.start;
        self.expect_peek(&TokenKind::LPAREN)?;
        self.next_token();

        let condition = self.parse_expression(Precedence::LOWEST)?.0;
        self.expect_peek(&TokenKind::RPAREN)?;
        self.expect_peek(&TokenKind::LBRACE)?;

        let consequent = self.parse_block_statement()?;

        let alternate = if self.peek_token_is(&TokenKind::ELSE) {
            self.next_token();
            self.expect_peek(&TokenKind::LBRACE)?;
            Some(self.parse_block_statement()?)
        } else {
            None
        };

        let end = self.current_token.span.end;

        return Ok(Expression::IF(IF {
            condition: Box::new(condition),
            consequent,
            alternate,
            span: Span {
                start,
                end,
            }
        }));
    }

    fn parse_block_statement(&mut self) -> Result<BlockStatement, ParseError> {
        self.next_token();
        let mut block_statement = Vec::new();

        while !self.current_token_is(&TokenKind::RBRACE) && !self.current_token_is(&TokenKind::EOF) {
            if let Ok(statement) = self.parse_statement() {
                block_statement.push(statement)
            }

            self.next_token();
        }

        Ok(BlockStatement::new(block_statement))
    }

    fn parse_fn_expression(&mut self) -> Result<Expression, ParseError> {
        let start = self.current_token.span.start;
        self.expect_peek(&TokenKind::LPAREN)?;

        let params = self.parse_fn_parameters()?;

        self.expect_peek(&TokenKind::LBRACE)?;

        let function_body = self.parse_block_statement()?;

        let end = self.current_token.span.end;

        Ok(Expression::FUNCTION(FunctionDeclaration {
            params: params,
            body: function_body,
            span: Span {
                start,
                end,
            }
        }))
    }

    fn parse_fn_parameters(&mut self) -> Result<Vec<String>, ParseError> {
        let mut params = Vec::new();
        if self.peek_token_is(&TokenKind::RPAREN) {
            self.next_token();
            return Ok(params);
        }

        self.next_token();

        match &self.current_token.kind {
            TokenKind::IDENTIFIER { name } => params.push(name.clone()),
            token => return Err(format!("expected function params  to be an identifier, got {}", token))
        }

        while self.peek_token_is(&TokenKind::COMMA) {
            self.next_token();
            self.next_token();
            match &self.current_token.kind {
                TokenKind::IDENTIFIER { name } => params.push(name.clone()),
                token => return Err(format!("expected function params  to be an identifier, got {}", token))
            }
        }

        self.expect_peek(&TokenKind::RPAREN)?;

        return Ok(params);
    }

    fn parse_fn_call_expression(&mut self, expr: Expression) -> Result<Expression, ParseError> {
        let (arguments, ..) = self.parse_expression_list(&TokenKind::RPAREN)?;
        Ok(Expression::FunctionCall(Box::new(expr), arguments))
    }

    fn parse_expression_list(&mut self, end: &TokenKind) -> Result<(Vec<Expression>, Span), ParseError> {
        let start = self.current_token.span.start;
        let mut expr_list = Vec::new();
        if self.peek_token_is(end) {
            self.next_token();
            let end = self.current_token.span.end;
            return Ok((expr_list, Span {
                start,
                end
            }));
        }

        self.next_token();

        expr_list.push(self.parse_expression(Precedence::LOWEST)?.0);

        while self.peek_token_is(&TokenKind::COMMA) {
            self.next_token();
            self.next_token();
            expr_list.push(self.parse_expression(Precedence::LOWEST)?.0);
        }

        self.expect_peek(end)?;
        let end = self.current_token.span.end;

        return Ok((expr_list, Span {
            start,
            end
        }));
    }

    fn parse_index_expression(&mut self, left: Expression) -> Result<Expression, ParseError> {
        self.next_token();
        let index = self.parse_expression(Precedence::LOWEST)?.0;

        self.expect_peek(&TokenKind::RBRACKET)?;

        return Ok(Expression::Index(Box::new(left), Box::new(index)));
    }

    fn parse_hash_expression(&mut self) -> Result<Expression, ParseError> {
        let mut map = Vec::new();
        let start = self.current_token.span.start;
        while !self.peek_token_is(&TokenKind::RBRACE) {
            self.next_token();

            let key = self.parse_expression(Precedence::LOWEST)?.0;

            self.expect_peek(&TokenKind::COLON)?;

            self.next_token();
            let value = self.parse_expression(Precedence::LOWEST)?.0;

            map.push((key, value));

            if !self.peek_token_is(&TokenKind::RBRACE) {
                self.expect_peek(&TokenKind::COMMA)?;
            }
        }

        self.expect_peek(&TokenKind::RBRACE)?;
        let end = self.current_token.span.end;

        Ok(Expression::LITERAL(Literal::Hash(
            Hash {
                elements: map,
                span: Span {
                    start,
                    end,
                },
            })))
    }
}

pub fn parse(input: &str) -> Result<Node, ParseErrors> {
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program()?;

    Ok(Node::Program(program))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify_program(test_cases: &[(&str, &str)]) {
        for (input, expected) in test_cases {
            let ast = parse(input).unwrap();
            let parsed = ast.to_string();
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
    fn test_parse_prefix_expression() {
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
    fn test_parse_infix_expression() {
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

    #[test]
    fn test_fn_call_else_expression() {
        let tt = [
            ("add(1, 2 * 3, 4 + 5);", "add(1, (2 * 3), (4 + 5))")
        ];
        verify_program(&tt);
    }

    #[test]
    fn test_string_literal_expression() {
        let test_case = [(r#""hello world";"#, r#""hello world""#)];
        verify_program(&test_case);
    }

    #[test]
    fn test_array_literal_expression() {
        let test_case = [
            ("[]", "[]"),
            ("[1, 2 * 2, 3 + 3]", "[1, (2 * 2), (3 + 3)]")
        ];
        verify_program(&test_case);
    }

    #[test]
    fn test_index_expression() {
        let test_case = [
            ("a[1]", "(a[1])"),
            ("a[1 + 1]", "(a[(1 + 1)])")
        ];
        verify_program(&test_case);
    }

    #[test]
    fn test_hash_literal_expression() {
        let test_case = [
            (
                r#"{"a": 1}"#,
                r#"{"a": 1}"#,
            ),
            (
                r#"{"one": 1, "two": 2, "three": 3}"#,
                r#"{"one": 1, "two": 2, "three": 3}"#,
            ),
            (r#"{}"#, r#"{}"#),
            (
                r#"{"one": 0 + 1, "two": 10 - 8, "three": 15 / 5}"#,
                r#"{"one": (0 + 1), "two": (10 - 8), "three": (15 / 5)}"#,
            ),
        ];
        verify_program(&test_case);
    }
}
