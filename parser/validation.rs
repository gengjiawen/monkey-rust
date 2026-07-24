use std::collections::HashSet;
use std::fmt;

use lexer::token::{Span, TokenKind};

use crate::ast::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableKind {
    Function,
    Method,
    Constructor,
}

struct Validator {
    scopes: Vec<HashSet<String>>,
    callable_kinds: Vec<CallableKind>,
    receiver_available: bool,
    context: Vec<String>,
}

pub fn validate_program(
    program: &Program,
    predefined_globals: &[&str],
) -> Result<(), ValidationError> {
    let mut globals = HashSet::new();
    globals.extend(predefined_globals.iter().map(|name| (*name).to_string()));
    let mut validator = Validator {
        scopes: vec![globals],
        callable_kinds: Vec::new(),
        receiver_available: false,
        context: Vec::new(),
    };
    validator.validate_statements(&program.body)
}

impl Validator {
    fn validate_statements(&mut self, statements: &[Statement]) -> Result<(), ValidationError> {
        for statement in statements {
            self.validate_statement(statement)?;
        }
        Ok(())
    }

    fn validate_statement(&mut self, statement: &Statement) -> Result<(), ValidationError> {
        match statement {
            Statement::Let(statement) => {
                let name = match &statement.identifier.kind {
                    TokenKind::IDENTIFIER {
                        name,
                    } => name.clone(),
                    _ => unreachable!("parser only creates let statements with identifiers"),
                };
                self.validate_expression(&statement.expr)?;
                self.scopes.last_mut().unwrap().insert(name);
                Ok(())
            }
            Statement::Return(statement) => {
                if self.callable_kinds.last() == Some(&CallableKind::Constructor) {
                    return Err(ValidationError {
                        message: "constructor cannot return a value".to_string(),
                        span: statement.span.clone(),
                    });
                }
                self.validate_expression(&statement.argument)
            }
            Statement::Class(class) => self.validate_class(class),
            Statement::SetProperty(statement) => {
                self.validate_expression(&statement.object)?;
                self.validate_expression(&statement.value)
            }
            Statement::Expr(expression) => self.validate_expression(expression),
        }
    }

    fn validate_class(&mut self, class: &ClassDeclaration) -> Result<(), ValidationError> {
        self.scopes
            .last_mut()
            .unwrap()
            .insert(class.name.name.clone());
        self.context.push(format!("class {}", class.name.name));
        for method in &class.methods {
            self.validate_method(method)?;
        }
        self.context.pop();
        Ok(())
    }

    fn validate_method(&mut self, method: &MethodDefinition) -> Result<(), ValidationError> {
        let callable_kind = match method.kind {
            MethodKind::Constructor => CallableKind::Constructor,
            MethodKind::Method => CallableKind::Method,
        };
        self.context.push(method.name.name.clone());
        self.callable_kinds.push(callable_kind);
        let old_receiver_available = self.receiver_available;
        self.receiver_available = true;
        self.scopes.push(
            method
                .params
                .iter()
                .map(|parameter| parameter.name.clone())
                .collect(),
        );

        let result = self.validate_statements(&method.body.body);

        self.scopes.pop();
        self.receiver_available = old_receiver_available;
        self.callable_kinds.pop();
        self.context.pop();
        result
    }

    fn validate_function(&mut self, function: &FunctionDeclaration) -> Result<(), ValidationError> {
        self.callable_kinds.push(CallableKind::Function);
        let mut scope = function
            .params
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect::<HashSet<_>>();
        if !function.name.is_empty() {
            // A directly let-bound function gets its binding name from the
            // parser. The compiler exposes that name only inside the function
            // body, which permits recursion without exposing an uninitialized
            // let binding to the initializer as a whole.
            scope.insert(function.name.clone());
        }
        self.scopes.push(scope);
        let result = self.validate_statements(&function.body.body);
        self.scopes.pop();
        self.callable_kinds.pop();
        result
    }

    fn validate_expression(&mut self, expression: &Expression) -> Result<(), ValidationError> {
        match expression {
            Expression::IDENTIFIER(identifier) => self.validate_identifier(identifier),
            Expression::LITERAL(literal) => self.validate_literal(literal),
            Expression::PREFIX(expression) => self.validate_expression(&expression.operand),
            Expression::INFIX(expression) => {
                self.validate_expression(&expression.left)?;
                self.validate_expression(&expression.right)
            }
            Expression::IF(expression) => {
                self.validate_expression(&expression.condition)?;
                self.validate_statements(&expression.consequent.body)?;
                if let Some(alternate) = &expression.alternate {
                    self.validate_statements(&alternate.body)?;
                }
                Ok(())
            }
            Expression::FUNCTION(function) => self.validate_function(function),
            Expression::FunctionCall(call) => {
                self.validate_expression(&call.callee)?;
                self.validate_expressions(&call.arguments)
            }
            Expression::Index(index) => {
                self.validate_expression(&index.object)?;
                self.validate_expression(&index.index)
            }
            Expression::This(this) => {
                if self.receiver_available {
                    Ok(())
                } else {
                    Err(ValidationError {
                        message: "this is only available inside a method".to_string(),
                        span: this.span.clone(),
                    })
                }
            }
            Expression::Property(property) => self.validate_expression(&property.object),
            Expression::New(new_expression) => {
                self.validate_identifier(&new_expression.callee)?;
                self.validate_expressions(&new_expression.arguments)
            }
        }
    }

    fn validate_literal(&mut self, literal: &Literal) -> Result<(), ValidationError> {
        match literal {
            Literal::Array(array) => self.validate_expressions(&array.elements),
            Literal::Hash(hash) => {
                for (key, value) in &hash.elements {
                    self.validate_expression(key)?;
                    self.validate_expression(value)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn validate_expressions(&mut self, expressions: &[Expression]) -> Result<(), ValidationError> {
        for expression in expressions {
            self.validate_expression(expression)?;
        }
        Ok(())
    }

    fn validate_identifier(&self, identifier: &IDENTIFIER) -> Result<(), ValidationError> {
        if self
            .scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(&identifier.name))
        {
            return Ok(());
        }

        let context = if self.context.is_empty() {
            String::new()
        } else {
            format!(" in {}", self.context.join("."))
        };
        Err(ValidationError {
            message: format!("undefined variable '{}'{}", identifier.name, context),
            span: identifier.span.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ast::Node, parse};

    fn validate(input: &str) -> Result<(), ValidationError> {
        let Node::Program(program) = parse(input).unwrap() else { panic!("expected program") };
        validate_program(&program, &["len"])
    }

    #[test]
    fn validates_source_order_constructor_return_and_lexical_this() {
        validate(
            r#"class Box {
  constructor(value) { this.value = value; }
  reader() { fn() { fn() { this.value } } }
}
let box = new Box(1);"#,
        )
        .unwrap();

        assert!(validate("this")
            .unwrap_err()
            .message
            .contains("only available"));
        assert!(validate("let f = fn() { this };")
            .unwrap_err()
            .message
            .contains("only available"));
        assert!(validate("class A { constructor() { return 1; } }")
            .unwrap_err()
            .message
            .contains("cannot return"));
        assert!(validate("class A { make() { new B(); } } class B {}")
            .unwrap_err()
            .message
            .contains("undefined variable 'B'"));
        validate("class A { make() { new A(); } }").unwrap();
        validate("len([])").unwrap();
    }

    #[test]
    fn let_initializer_sees_only_previous_bindings_and_named_function_self() {
        assert!(validate("let x = x;")
            .unwrap_err()
            .message
            .contains("undefined variable 'x'"));

        validate("let x = 1; let x = x + 1; x;").unwrap();
        validate("let f = fn(n) { if (n == 0) { 0 } else { f(n - 1) } }; f(1);").unwrap();

        assert!(validate("let f = if (true) { fn() { f(); } };")
            .unwrap_err()
            .message
            .contains("undefined variable 'f'"));
    }
}
