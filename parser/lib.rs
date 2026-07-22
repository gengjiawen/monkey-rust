pub mod ast;
mod ast_tree_test;
mod parser_test;
mod precedences;
pub mod validation;

pub extern crate lexer;

use crate::ast::*;
use crate::precedences::{get_token_precedence, Precedence};
use lexer::token::{Span, Token, TokenKind};
use lexer::Lexer;

type ParseError = String;
type ParseErrors = Vec<ParseError>;

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_token: Token,
    errors: ParseErrors,
    block_depth: usize,
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
            block_depth: 0,
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
        program.span.end = self.current_token.span.end;

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
            TokenKind::CLASS if self.block_depth == 0 => self.parse_class_declaration(),
            TokenKind::CLASS => Err("class declarations are only allowed at top level".to_string()),
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_let_statement(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_token.span.start;
        self.next_token();

        let name = self.current_token.clone();
        let identifier_name = match &self.current_token.kind {
            TokenKind::IDENTIFIER {
                name,
            } => name.to_string(),
            _ => return Err(format!("{} not an identifier", self.current_token)),
        };

        self.expect_peek(&TokenKind::ASSIGN)?;
        self.next_token();

        let mut value = self.parse_expression(Precedence::Lowest)?.0;
        if self.peek_token_is(&TokenKind::ASSIGN) {
            return Err("property assignment is only allowed as a statement".to_string());
        }
        if let Expression::FUNCTION(ref mut f) = value {
            f.name = identifier_name;
        }

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
        let start = self.current_token.span.start;
        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?.0;

        if self.peek_token_is(&TokenKind::ASSIGN) {
            return Err("property assignment is only allowed as a statement".to_string());
        }

        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }
        let end = self.current_token.span.end;

        return Ok(Statement::Return(ReturnStatement {
            argument: value,
            span: Span {
                start,
                end,
            },
        }));
    }

    fn parse_expression_statement(&mut self) -> Result<Statement, ParseError> {
        let (expr, cover_span) = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token_is(&TokenKind::ASSIGN) {
            let property_expression = match expr {
                Expression::Property(property) => property,
                _ => return Err("only instance property assignment is supported".to_string()),
            };

            self.next_token();
            self.next_token();
            let (value, value_span) = self.parse_expression(Precedence::Lowest)?;
            if self.peek_token_is(&TokenKind::ASSIGN) {
                return Err("chained property assignment is not supported".to_string());
            }

            let mut end = value_span.end;
            if self.peek_token_is(&TokenKind::SEMICOLON) {
                self.next_token();
                end = self.current_token.span.end;
            }

            return Ok(Statement::SetProperty(SetPropertyStatement {
                object: property_expression.object,
                property: property_expression.property,
                value,
                span: Span {
                    start: cover_span.start,
                    end,
                },
            }));
        }

        if self.peek_token_is(&TokenKind::SEMICOLON) {
            self.next_token();
        }

        Ok(Statement::Expr(expr))
    }

    fn parse_expression(
        &mut self,
        precedence: Precedence,
    ) -> Result<(Expression, Span), ParseError> {
        let (mut left, mut cover_span) = self.parse_prefix_expression()?;
        while self.peek_token.kind != TokenKind::SEMICOLON
            && precedence < get_token_precedence(&self.peek_token.kind)
        {
            match self.parse_infix_expression(&left, &cover_span) {
                Some(infix) => {
                    (left, cover_span) = infix?;
                }
                None => {
                    return Ok((left, cover_span));
                }
            }
        }

        Ok((left, cover_span))
    }

    fn parse_prefix_expression(&mut self) -> Result<(Expression, Span), ParseError> {
        // this is prefix fn map :)
        match &self.current_token.kind {
            TokenKind::IDENTIFIER {
                name,
            } => {
                let span = self.current_token.span.clone();
                return Ok((
                    Expression::IDENTIFIER(IDENTIFIER {
                        name: name.clone(),
                        span: span.clone(),
                    }),
                    span,
                ));
            }
            TokenKind::INT(i) => {
                let span = self.current_token.span.clone();
                return Ok((
                    Expression::LITERAL(Literal::Integer(Integer {
                        raw: *i,
                        span: span.clone(),
                    })),
                    span,
                ));
            }
            TokenKind::STRING(s) => {
                let span = self.current_token.span.clone();
                return Ok((
                    Expression::LITERAL(Literal::String(StringType {
                        raw: s.to_string(),
                        span: span.clone(),
                    })),
                    span,
                ));
            }
            b @ TokenKind::TRUE | b @ TokenKind::FALSE => {
                let span = self.current_token.span.clone();
                return Ok((
                    Expression::LITERAL(Literal::Boolean(Boolean {
                        raw: *b == TokenKind::TRUE,
                        span: span.clone(),
                    })),
                    span,
                ));
            }
            TokenKind::BANG | TokenKind::MINUS => {
                let start = self.current_token.span.start;
                let prefix_op = self.current_token.clone();
                self.next_token();
                let (expr, span) = self.parse_expression(Precedence::Prefix)?;
                let expression_span = Span {
                    start,
                    end: span.end,
                };
                return Ok((
                    Expression::PREFIX(UnaryExpression {
                        op: prefix_op,
                        operand: Box::new(expr),
                        span: expression_span.clone(),
                    }),
                    expression_span,
                ));
            }
            TokenKind::LPAREN => {
                let start = self.current_token.span.start;
                self.next_token();
                let expr = self.parse_expression(Precedence::Lowest)?.0;
                self.expect_peek(&TokenKind::RPAREN)?;
                let span = Span {
                    start,
                    end: self.current_token.span.end,
                };
                return Ok((expr, span));
            }
            TokenKind::IF => {
                let expression = self.parse_if_expression()?;
                let span = expression.span().clone();
                Ok((expression, span))
            }
            TokenKind::FUNCTION => {
                let expression = self.parse_fn_expression()?;
                let span = expression.span().clone();
                Ok((expression, span))
            }
            TokenKind::LBRACKET => {
                let (elements, span) = self.parse_expression_list(&TokenKind::RBRACKET)?;
                return Ok((
                    Expression::LITERAL(Literal::Array(Array {
                        elements,
                        span: span.clone(),
                    })),
                    span,
                ));
            }
            TokenKind::LBRACE => {
                let expression = self.parse_hash_expression()?;
                let span = expression.span().clone();
                Ok((expression, span))
            }
            TokenKind::THIS => {
                let span = self.current_token.span.clone();
                Ok((
                    Expression::This(ThisExpression {
                        span: span.clone(),
                    }),
                    span,
                ))
            }
            TokenKind::NEW => {
                let expression = self.parse_new_expression()?;
                let span = expression.span().clone();
                Ok((expression, span))
            }
            _ => Err(format!("no prefix function for token: {}", self.current_token)),
        }
    }

    fn parse_infix_expression(
        &mut self,
        left: &Expression,
        left_span: &Span,
    ) -> Option<Result<(Expression, Span), ParseError>> {
        match self.peek_token.kind {
            TokenKind::PLUS
            | TokenKind::MINUS
            | TokenKind::ASTERISK
            | TokenKind::SLASH
            | TokenKind::EQ
            | TokenKind::NotEq
            | TokenKind::LT
            | TokenKind::GT => {
                self.next_token();
                let infix_op = self.current_token.clone();
                let precedence_value = get_token_precedence(&self.current_token.kind);
                self.next_token();
                let result = self
                    .parse_expression(precedence_value)
                    .map(|(right, span)| {
                        let expression_span = Span {
                            start: left_span.start,
                            end: span.end,
                        };
                        (
                            Expression::INFIX(BinaryExpression {
                                op: infix_op,
                                left: Box::new(left.clone()),
                                right: Box::new(right),
                                span: expression_span.clone(),
                            }),
                            expression_span,
                        )
                    });
                return Some(result);
            }
            TokenKind::LPAREN => {
                self.next_token();
                return Some(self.parse_fn_call_expression(left.clone(), left_span.start));
            }
            TokenKind::LBRACKET => {
                self.next_token();
                return Some(self.parse_index_expression(left.clone(), left_span.start));
            }
            TokenKind::DOT => {
                self.next_token();
                return Some(self.parse_property_expression(left.clone(), left_span.start));
            }
            _ => None,
        }
    }

    fn parse_if_expression(&mut self) -> Result<Expression, ParseError> {
        let start = self.current_token.span.start;
        self.expect_peek(&TokenKind::LPAREN)?;
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?.0;
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
            },
        }));
    }

    fn parse_block_statement(&mut self) -> Result<BlockStatement, ParseError> {
        let start = self.current_token.span.start;
        self.block_depth += 1;
        self.next_token();
        let mut block_statement = Vec::new();

        while !self.current_token_is(&TokenKind::RBRACE) && !self.current_token_is(&TokenKind::EOF)
        {
            let statement = match self.parse_statement() {
                Ok(statement) => statement,
                Err(error) => {
                    self.block_depth -= 1;
                    return Err(error);
                }
            };
            block_statement.push(statement);

            self.next_token();
        }

        self.block_depth -= 1;
        if self.current_token_is(&TokenKind::EOF) {
            return Err("expected '}' before end of input".to_string());
        }

        let end = self.current_token.span.end;

        Ok(BlockStatement {
            body: block_statement,
            span: Span {
                start,
                end,
            },
        })
    }

    fn parse_fn_expression(&mut self) -> Result<Expression, ParseError> {
        let start = self.current_token.span.start;
        self.expect_peek(&TokenKind::LPAREN)?;

        let params = self.parse_fn_parameters()?;

        self.expect_peek(&TokenKind::LBRACE)?;

        let function_body = self.parse_block_statement()?;

        let end = self.current_token.span.end;

        Ok(Expression::FUNCTION(FunctionDeclaration {
            params,
            body: function_body,
            span: Span {
                start,
                end,
            },
            name: "".to_string(),
        }))
    }

    fn parse_fn_parameters(&mut self) -> Result<Vec<IDENTIFIER>, ParseError> {
        let mut params = Vec::new();
        if self.peek_token_is(&TokenKind::RPAREN) {
            self.next_token();
            return Ok(params);
        }

        self.next_token();

        match &self.current_token.kind {
            TokenKind::IDENTIFIER {
                name,
            } => params.push(IDENTIFIER {
                name: name.clone(),
                span: self.current_token.span.clone(),
            }),
            token => {
                return Err(format!("expected function params  to be an identifier, got {}", token))
            }
        }

        while self.peek_token_is(&TokenKind::COMMA) {
            self.next_token();
            self.next_token();
            match &self.current_token.kind {
                TokenKind::IDENTIFIER {
                    name,
                } => params.push(IDENTIFIER {
                    name: name.clone(),
                    span: self.current_token.span.clone(),
                }),
                token => {
                    return Err(format!(
                        "expected function params  to be an identifier, got {}",
                        token
                    ))
                }
            }
        }

        self.expect_peek(&TokenKind::RPAREN)?;

        return Ok(params);
    }

    fn parse_fn_call_expression(
        &mut self,
        expr: Expression,
        start: usize,
    ) -> Result<(Expression, Span), ParseError> {
        let (arguments, ..) = self.parse_expression_list(&TokenKind::RPAREN)?;
        let end = self.current_token.span.end;
        let callee = Box::new(expr);
        let span = Span {
            start,
            end,
        };

        Ok((
            Expression::FunctionCall(FunctionCall {
                callee,
                arguments,
                span: span.clone(),
            }),
            span,
        ))
    }

    fn parse_expression_list(
        &mut self,
        end: &TokenKind,
    ) -> Result<(Vec<Expression>, Span), ParseError> {
        let start = self.current_token.span.start;
        let mut expr_list = Vec::new();
        if self.peek_token_is(end) {
            self.next_token();
            let end = self.current_token.span.end;
            return Ok((
                expr_list,
                Span {
                    start,
                    end,
                },
            ));
        }

        self.next_token();

        expr_list.push(self.parse_expression(Precedence::Lowest)?.0);

        while self.peek_token_is(&TokenKind::COMMA) {
            self.next_token();
            self.next_token();
            expr_list.push(self.parse_expression(Precedence::Lowest)?.0);
        }

        self.expect_peek(end)?;
        let end = self.current_token.span.end;

        return Ok((
            expr_list,
            Span {
                start,
                end,
            },
        ));
    }

    fn parse_index_expression(
        &mut self,
        left: Expression,
        start: usize,
    ) -> Result<(Expression, Span), ParseError> {
        self.next_token();
        let index = self.parse_expression(Precedence::Lowest)?.0;

        self.expect_peek(&TokenKind::RBRACKET)?;

        let end = self.current_token.span.end;

        let span = Span {
            start,
            end,
        };
        return Ok((
            Expression::Index(Index {
                object: Box::new(left),
                index: Box::new(index),
                span: span.clone(),
            }),
            span,
        ));
    }

    fn parse_property_expression(
        &mut self,
        object: Expression,
        start: usize,
    ) -> Result<(Expression, Span), ParseError> {
        self.next_token();
        let property = match &self.current_token.kind {
            TokenKind::IDENTIFIER {
                name,
            } => IDENTIFIER {
                name: name.clone(),
                span: self.current_token.span.clone(),
            },
            _ => return Err("expected property name after '.'".to_string()),
        };
        let span = Span {
            start,
            end: property.span.end,
        };
        Ok((
            Expression::Property(PropertyExpression {
                object: Box::new(object),
                property,
                span: span.clone(),
            }),
            span,
        ))
    }

    fn parse_new_expression(&mut self) -> Result<Expression, ParseError> {
        let start = self.current_token.span.start;
        self.next_token();
        let callee = match &self.current_token.kind {
            TokenKind::IDENTIFIER {
                name,
            } => IDENTIFIER {
                name: name.clone(),
                span: self.current_token.span.clone(),
            },
            _ => return Err("expected class name after 'new'".to_string()),
        };

        if !self.peek_token_is(&TokenKind::LPAREN) {
            return Err("new expression requires an argument list".to_string());
        }
        self.next_token();
        let (arguments, arguments_span) = self.parse_expression_list(&TokenKind::RPAREN)?;
        Ok(Expression::New(NewExpression {
            callee,
            arguments,
            span: Span {
                start,
                end: arguments_span.end,
            },
        }))
    }

    fn parse_class_declaration(&mut self) -> Result<Statement, ParseError> {
        let start = self.current_token.span.start;
        self.next_token();
        let class_name = match &self.current_token.kind {
            TokenKind::IDENTIFIER {
                name,
            } => IDENTIFIER {
                name: name.clone(),
                span: self.current_token.span.clone(),
            },
            _ => return Err("expected class name after 'class'".to_string()),
        };

        self.expect_peek(&TokenKind::LBRACE)?;
        let mut methods = Vec::new();
        let mut method_names = std::collections::HashSet::new();
        let mut has_constructor = false;

        while !self.peek_token_is(&TokenKind::RBRACE) {
            self.next_token();
            if self.current_token_is(&TokenKind::EOF) {
                return Err(format!("expected '}}' after class {}", class_name.name));
            }

            let method_name = match &self.current_token.kind {
                TokenKind::IDENTIFIER {
                    name,
                } => IDENTIFIER {
                    name: name.clone(),
                    span: self.current_token.span.clone(),
                },
                _ => return Err("expected method definition in class body".to_string()),
            };
            let method_start = method_name.span.start;
            let kind = if method_name.name == "constructor" {
                if has_constructor {
                    return Err(format!("class {} has more than one constructor", class_name.name));
                }
                has_constructor = true;
                MethodKind::Constructor
            } else {
                if !method_names.insert(method_name.name.clone()) {
                    return Err(format!(
                        "duplicate method {}.{}",
                        class_name.name, method_name.name
                    ));
                }
                MethodKind::Method
            };

            self.expect_peek(&TokenKind::LPAREN)?;
            let params = self.parse_fn_parameters()?;
            self.expect_peek(&TokenKind::LBRACE)?;
            let body = self.parse_block_statement()?;
            let method_end = body.span.end;
            methods.push(MethodDefinition {
                kind,
                name: method_name,
                params,
                body,
                span: Span {
                    start: method_start,
                    end: method_end,
                },
            });
        }

        self.next_token();
        Ok(Statement::Class(ClassDeclaration {
            name: class_name,
            methods,
            span: Span {
                start,
                end: self.current_token.span.end,
            },
        }))
    }

    fn parse_hash_expression(&mut self) -> Result<Expression, ParseError> {
        let mut map = Vec::new();
        let start = self.current_token.span.start;
        while !self.peek_token_is(&TokenKind::RBRACE) {
            self.next_token();

            let key = self.parse_expression(Precedence::Lowest)?.0;

            self.expect_peek(&TokenKind::COLON)?;

            self.next_token();
            let value = self.parse_expression(Precedence::Lowest)?.0;

            map.push((key, value));

            if !self.peek_token_is(&TokenKind::RBRACE) {
                self.expect_peek(&TokenKind::COMMA)?;
            }
        }

        self.expect_peek(&TokenKind::RBRACE)?;
        let end = self.current_token.span.end;

        Ok(Expression::LITERAL(Literal::Hash(Hash {
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

pub fn parse_ast_json_string(input: &str) -> Result<String, ParseErrors> {
    let node = parse(input)?;
    let ast = serde_json::to_string_pretty(&node).unwrap();

    return Ok(ast);
}

/// Serialize the parser AST without routing i64 integer literals through a
/// JavaScript `number`. The JSON shape otherwise stays identical to
/// [`parse_ast_json_string`].
pub fn parse_ast_lossless_json_string(input: &str) -> Result<String, ParseErrors> {
    let node = parse(input)?;
    let mut ast = serde_json::to_value(&node).expect("AST serialization should not fail");
    stringify_integer_literals(&mut ast);
    Ok(serde_json::to_string_pretty(&ast).expect("AST serialization should not fail"))
}

fn stringify_integer_literals(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                stringify_integer_literals(value);
            }
        }
        serde_json::Value::Object(object) => {
            if object.get("type").and_then(serde_json::Value::as_str) == Some("Integer") {
                if let Some(raw) = object.get_mut("raw") {
                    if let Some(integer) = raw.as_i64() {
                        *raw = serde_json::Value::String(integer.to_string());
                    }
                }
            }
            for value in object.values_mut() {
                stringify_integer_literals(value);
            }
        }
        _ => {}
    }
}
