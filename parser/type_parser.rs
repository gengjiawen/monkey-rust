//! The type sub-grammar.
//!
//! These are methods on the one and only [`Parser`]: they share the lexer,
//! token cursor, `ParseError` channel and span infrastructure with expression
//! parsing, but they never re-enter the Pratt expression parser. Monkey has no
//! typed/untyped mode switch — annotations are optional syntax inside ordinary
//! `.monkey` source, and every type position is introduced by a preceding `:`
//! (or by the structure of an enclosing type), so no backtracking is needed.

use crate::ast::*;
use crate::{ParseError, Parser};
use lexer::token::{Span, TokenKind};

impl Parser<'_> {
    /// Parses `: T` starting from the token *before* the `:`.
    pub(crate) fn parse_type_annotation(&mut self) -> Result<TypeAnnotation, ParseError> {
        self.expect_peek(&TokenKind::COLON)?;
        self.next_token();
        return self.parse_type();
    }

    /// Consumes `: T` when the peek token is a `:`, otherwise yields `None`.
    pub(crate) fn parse_optional_type_annotation(
        &mut self,
    ) -> Result<Option<TypeAnnotation>, ParseError> {
        if !self.peek_token_is(&TokenKind::COLON) {
            return Ok(None);
        }
        return Ok(Some(self.parse_type_annotation()?));
    }

    /// `type ::= postfix_type`. Assumes the current token starts the type.
    pub(crate) fn parse_type(&mut self) -> Result<TypeAnnotation, ParseError> {
        let mut annotation = self.parse_primary_type()?;

        // `?` is postfix and binds tightest; `T??` normalizes to `T?`.
        while self.peek_token_is(&TokenKind::QUESTION) {
            self.next_token();
            let span = Span {
                start: annotation.span().start,
                end: self.current_token.span.end,
            };
            annotation = match annotation {
                already @ TypeAnnotation::Optional(_) => already,
                inner => TypeAnnotation::Optional(OptionalType {
                    inner: Box::new(inner),
                    span,
                }),
            };
        }

        return Ok(annotation);
    }

    fn parse_primary_type(&mut self) -> Result<TypeAnnotation, ParseError> {
        match &self.current_token.kind {
            TokenKind::IDENTIFIER {
                name,
            } => {
                return Ok(TypeAnnotation::Named(NamedType {
                    name: name.clone(),
                    span: self.current_token.span.clone(),
                }))
            }
            TokenKind::LBRACKET => {
                let start = self.current_token.span.start;
                self.next_token();
                let element = self.parse_type()?;
                self.expect_peek(&TokenKind::RBRACKET)?;
                return Ok(TypeAnnotation::Array(ArrayType {
                    element: Box::new(element),
                    span: Span {
                        start,
                        end: self.current_token.span.end,
                    },
                }));
            }
            TokenKind::LBRACE => {
                let start = self.current_token.span.start;
                self.next_token();
                let key = self.parse_type()?;
                self.expect_peek(&TokenKind::COLON)?;
                self.next_token();
                let value = self.parse_type()?;
                self.expect_peek(&TokenKind::RBRACE)?;
                return Ok(TypeAnnotation::Hash(HashType {
                    key: Box::new(key),
                    value: Box::new(value),
                    span: Span {
                        start,
                        end: self.current_token.span.end,
                    },
                }));
            }
            TokenKind::FUNCTION => {
                let start = self.current_token.span.start;
                self.expect_peek(&TokenKind::LPAREN)?;
                let params = self.parse_type_list()?;
                // A function *type* must spell out its return type; that is what
                // keeps `{fn(int): int: bool}` unambiguous without backtracking.
                if !self.peek_token_is(&TokenKind::COLON) {
                    return Err("function type requires a return type".to_string());
                }
                let return_type = self.parse_type_annotation()?;
                return Ok(TypeAnnotation::Function(FunctionType {
                    params,
                    span: Span {
                        start,
                        end: return_type.span().end,
                    },
                    return_type: Box::new(return_type),
                }));
            }
            TokenKind::LPAREN => {
                // Grouping produces no node of its own; it only widens the cover
                // span of the type it wraps.
                let start = self.current_token.span.start;
                self.next_token();
                let inner = self.parse_type()?;
                self.expect_peek(&TokenKind::RPAREN)?;
                let span = Span {
                    start,
                    end: self.current_token.span.end,
                };
                return Ok(with_span(inner, span));
            }
            _ => {
                return Err(format!("expected a type, got: {}", self.current_token));
            }
        }
    }

    /// `type_list ::= ( type ( "," type )* )?`, consuming through the `)`.
    /// Assumes the current token is the opening `(`.
    fn parse_type_list(&mut self) -> Result<Vec<TypeAnnotation>, ParseError> {
        let mut types = Vec::new();
        if self.peek_token_is(&TokenKind::RPAREN) {
            self.next_token();
            return Ok(types);
        }

        self.next_token();
        types.push(self.parse_type()?);

        while self.peek_token_is(&TokenKind::COMMA) {
            self.next_token();
            self.next_token();
            types.push(self.parse_type()?);
        }

        self.expect_peek(&TokenKind::RPAREN)?;

        return Ok(types);
    }
}

/// Replaces a type's span with the cover span of its enclosing parentheses.
fn with_span(annotation: TypeAnnotation, span: Span) -> TypeAnnotation {
    match annotation {
        TypeAnnotation::Named(mut inner) => {
            inner.span = span;
            TypeAnnotation::Named(inner)
        }
        TypeAnnotation::Array(mut inner) => {
            inner.span = span;
            TypeAnnotation::Array(inner)
        }
        TypeAnnotation::Hash(mut inner) => {
            inner.span = span;
            TypeAnnotation::Hash(inner)
        }
        TypeAnnotation::Function(mut inner) => {
            inner.span = span;
            TypeAnnotation::Function(inner)
        }
        TypeAnnotation::Optional(mut inner) => {
            inner.span = span;
            TypeAnnotation::Optional(inner)
        }
    }
}
