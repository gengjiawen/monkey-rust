//! Single-pass AST → AArch64 lowering (design §6, §7): accumulator in `x0`,
//! temporaries on the machine stack, scope analysis via the bytecode
//! compiler's `SymbolTable`, and every dynamic operation through the frozen
//! `rt_*` ABI. No IR, no register allocation, no optimization.

use compiler::symbol_table::{Symbol, SymbolScope, SymbolTable};
use object::builtins::BuiltIns;
use parser::ast::{
    BlockStatement, ClassDeclaration, Expression, FunctionDeclaration, Let, Literal,
    MethodDefinition, MethodKind, Node, Statement,
};
use parser::lexer::token::{Span, TokenKind};
use parser::validation::validate_program;
use std::rc::Rc;

use crate::emitter::{
    call_area_size, scratch_area_size, slot_offset, Assembly, Emitter, FunctionFrame,
    CLOSURE_SLOT_OFFSET,
};
use crate::runtime_core::{
    builtin_id_for_symbol_index, builtin_value, i64_fits_smi, FALSE_VALUE, NULL_VALUE, TRUE_VALUE,
};

/// Hard limits from the calling convention (design §2.2, §7): closures pass
/// user parameters in `x1..x7`; methods spend `x1` on `this`.
pub const MAX_FUNCTION_PARAMETERS: usize = 7;
pub const MAX_METHOD_PARAMETERS: usize = 6;

const MAIN_EPILOGUE_LABEL: &str = ".Lmain_exit";

#[derive(Clone, Debug)]
pub struct LowerError {
    pub message: String,
    pub span: Option<(usize, usize)>,
}

impl std::fmt::Display for LowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

fn error<T>(message: impl Into<String>, span: &Span) -> Result<T, LowerError> {
    Err(LowerError {
        message: message.into(),
        span: Some((span.start, span.end)),
    })
}

/// Lowers a parsed program to a complete assembly module. `observe` selects
/// the differential-testing build: `rt_observer_init(3)` at startup and one
/// `rt_observe_result` before exit (design §10.2).
pub fn lower_node(source: &str, node: &Node, observe: bool) -> Result<Assembly, LowerError> {
    let program = match node {
        Node::Program(program) => program,
        _ => {
            return Err(LowerError {
                message: "lowering expects a full program".to_string(),
                span: None,
            })
        }
    };
    let builtin_names: Vec<&str> = BuiltIns.iter().map(|builtin| builtin.name).collect();
    validate_program(program, &builtin_names).map_err(|validation| LowerError {
        message: validation.message,
        span: Some((validation.span.start, validation.span.end)),
    })?;

    let mut symbols = SymbolTable::new();
    for (index, builtin) in BuiltIns.iter().enumerate() {
        symbols.define_builtin(index, builtin.name.to_string());
    }

    let mut lowerer = Lowerer {
        source,
        emitter: Emitter::new(),
        symbols,
        epilogues: vec![MAIN_EPILOGUE_LABEL.to_string()],
    };

    let mut last_leaves_value = false;
    for statement in &program.body {
        last_leaves_value = lowerer.lower_statement(statement)?;
    }
    if !last_leaves_value {
        // A program ending in a non-expression statement results in `null`
        // (design §10.2).
        lowerer.emitter.without_span(|emitter| {
            emitter.load_imm64("x0", NULL_VALUE, "program result: null");
        });
    }

    let globals_count = lowerer.symbols.num_definitions;
    Ok(lowerer
        .emitter
        .finish(globals_count, MAIN_EPILOGUE_LABEL, observe))
}

/// Parses + lowers in one step for the CLI and tests; parse and lowering
/// failures are joined into printable messages.
pub fn compile_source(source: &str, observe: bool) -> Result<Assembly, String> {
    let node = parser::parse(source).map_err(|errors| errors.join("\n"))?;
    lower_node(source, &node, observe).map_err(|lower| lower.message)
}

struct Lowerer<'a> {
    source: &'a str,
    emitter: Emitter,
    symbols: SymbolTable,
    /// Return statements branch to the top label; bottom is `main`'s
    /// epilogue (top-level `return` ends the program).
    epilogues: Vec<String>,
}

impl<'a> Lowerer<'a> {
    fn snippet(&self, span: &Span) -> String {
        let text = self.source.get(span.start..span.end).unwrap_or("");
        let mut cleaned: String = text
            .chars()
            .take(40)
            .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
            .collect();
        if text.chars().count() > 40 {
            cleaned.push('…');
        }
        cleaned
    }

    fn enter_scope(&mut self) {
        let outer = std::mem::replace(&mut self.symbols, SymbolTable::new());
        self.symbols = SymbolTable::new_enclosed_symbol_table(outer);
    }

    fn leave_scope(&mut self) {
        let outer = self
            .symbols
            .outer
            .as_ref()
            .expect("leave_scope at the global scope")
            .as_ref()
            .clone();
        self.symbols = outer;
    }

    /// Lowers one statement. Returns whether it leaves the statement value
    /// in `x0` (only expression statements do; blocks and the program use
    /// this for their `null` completion rule).
    fn lower_statement(&mut self, statement: &Statement) -> Result<bool, LowerError> {
        match statement {
            Statement::Let(let_statement) => {
                self.lower_let(let_statement)?;
                Ok(false)
            }
            Statement::Return(return_statement) => {
                let comment = format!("return {}", self.snippet(return_statement.argument.span()));
                let target = self
                    .epilogues
                    .last()
                    .expect("epilogue stack is never empty")
                    .clone();
                self.emitter.comment(&comment);
                self.lower_expression(&return_statement.argument)?;
                let span = return_statement.span.clone();
                self.emitter.with_span(&span, |emitter| {
                    emitter.ins(&format!("b {}", target));
                });
                Ok(true)
            }
            Statement::Expr(expression) => {
                self.lower_expression(expression)?;
                Ok(true)
            }
            Statement::Class(class) => {
                self.lower_class(class)?;
                Ok(false)
            }
            Statement::SetProperty(set) => {
                let comment = self.snippet(&set.span);
                self.emitter.comment(&comment);
                self.lower_expression(&set.object)?;
                self.emitter.with_span(&set.span.clone(), |emitter| {
                    emitter.push_acc("object");
                });
                self.lower_expression(&set.value)?;
                let (name_label, name_len) =
                    self.emitter.intern_string(set.property.name.as_bytes());
                let property = set.property.name.clone();
                self.emitter.with_span(&set.span.clone(), |emitter| {
                    emitter.ins_cmt("mov x3, x0", "value");
                    emitter.pop("x0", "object");
                    emitter.load_label_address("x1", &name_label, &property);
                    emitter.load_imm64("x2", name_len, "");
                    emitter.ins("bl rt_set_property");
                });
                Ok(false)
            }
        }
    }

    fn lower_let(&mut self, let_statement: &Let) -> Result<(), LowerError> {
        let name = let_statement.identifier.kind.to_string();
        let comment = self.snippet(&let_statement.span);
        self.emitter.comment(&comment);
        // The right-hand side sees the previous binding, if any. Named
        // recursion does not depend on predeclaring this slot: function
        // bodies resolve their parser-provided name through Function scope.
        self.lower_expression(&let_statement.expr)?;
        let symbol = self.symbols.define(name.clone());
        let span = let_statement.span.clone();
        self.emitter.with_span(&span, |emitter| match symbol.scope {
            SymbolScope::Global => {
                emitter.global_store("x0", symbol.index, &format!("let {}", name))
            }
            _ => emitter.frame_store("x0", slot_offset(symbol.index), &format!("let {}", name)),
        });
        Ok(())
    }

    /// Block completion value (design §10.2 and the interpreter): the value
    /// of the last expression statement, otherwise `null`.
    fn lower_block_value(&mut self, block: &BlockStatement) -> Result<(), LowerError> {
        let mut leaves_value = false;
        for statement in &block.body {
            leaves_value = self.lower_statement(statement)?;
        }
        if !leaves_value {
            self.emitter.without_span(|emitter| {
                emitter.load_imm64("x0", NULL_VALUE, "empty/valueless block: null");
            });
        }
        Ok(())
    }

    fn load_symbol(&mut self, symbol: &Rc<Symbol>, span: &Span) -> Result<(), LowerError> {
        let name = symbol.name.clone();
        match symbol.scope {
            SymbolScope::Global => {
                let index = symbol.index;
                self.emitter.with_span(&span.clone(), |emitter| {
                    emitter.global_load("x0", index, &name);
                });
            }
            SymbolScope::LOCAL => {
                let index = symbol.index;
                self.emitter.with_span(&span.clone(), |emitter| {
                    emitter.frame_load("x0", slot_offset(index), &name);
                });
            }
            SymbolScope::Builtin => {
                let id = match builtin_id_for_symbol_index(symbol.index) {
                    Some(id) => id,
                    None => return error(format!("unknown builtin '{}'", name), span),
                };
                self.emitter.with_span(&span.clone(), |emitter| {
                    emitter.load_imm64("x0", builtin_value(id), &format!("builtin {}", name));
                });
            }
            SymbolScope::Free => {
                let index = symbol.index;
                self.emitter.with_span(&span.clone(), |emitter| {
                    emitter.frame_load("x0", CLOSURE_SLOT_OFFSET, "current closure");
                    emitter.load_imm64("x1", index as u64, &format!("free variable {}", name));
                    emitter.ins("bl rt_get_free");
                });
            }
            SymbolScope::Function => {
                // Named self-reference reads the spilled closure slot
                // (design §7); requires `define_function_name` on scope entry.
                self.emitter.with_span(&span.clone(), |emitter| {
                    emitter.frame_load(
                        "x0",
                        CLOSURE_SLOT_OFFSET,
                        &format!("current closure ({})", name),
                    );
                });
            }
        }
        Ok(())
    }

    fn lower_expression(&mut self, expression: &Expression) -> Result<(), LowerError> {
        match expression {
            Expression::IDENTIFIER(identifier) => {
                let symbol = match self.symbols.resolve(identifier.name.clone()) {
                    Some(symbol) => symbol,
                    None => {
                        return error(
                            format!("undefined variable '{}'", identifier.name),
                            &identifier.span,
                        )
                    }
                };
                self.load_symbol(&symbol, &identifier.span)
            }
            Expression::LITERAL(literal) => self.lower_literal(literal),
            Expression::PREFIX(prefix) => {
                self.lower_expression(&prefix.operand)?;
                let comment = self.snippet(&prefix.span);
                let runtime_call = match prefix.op.kind {
                    TokenKind::MINUS => "bl rt_minus",
                    TokenKind::BANG => "bl rt_bang",
                    _ => {
                        return error(format!("unexpected prefix op: {}", prefix.op), &prefix.span)
                    }
                };
                self.emitter.with_span(&prefix.span.clone(), |emitter| {
                    emitter.ins_cmt(runtime_call, &comment);
                });
                Ok(())
            }
            Expression::INFIX(infix) => self.lower_infix(infix),
            Expression::IF(if_node) => {
                self.lower_expression(&if_node.condition)?;
                let else_label = self.emitter.new_label();
                let end_label = self.emitter.new_label();
                let comment = format!("if ({})", self.snippet(if_node.condition.span()));
                self.emitter.with_span(&if_node.span.clone(), |emitter| {
                    emitter.ins_cmt("bl rt_truthy", &comment);
                    emitter.ins(&format!("cbz x0, {}", else_label));
                });
                self.lower_block_value(&if_node.consequent)?;
                self.emitter.with_span(&if_node.span.clone(), |emitter| {
                    emitter.ins(&format!("b {}", end_label));
                    emitter.label(&else_label);
                });
                match &if_node.alternate {
                    Some(alternate) => self.lower_block_value(alternate)?,
                    None => {
                        self.emitter.without_span(|emitter| {
                            emitter.load_imm64("x0", NULL_VALUE, "if without else: null");
                        });
                    }
                }
                self.emitter.with_span(&if_node.span.clone(), |emitter| {
                    emitter.label(&end_label);
                });
                Ok(())
            }
            Expression::Index(index) => {
                self.lower_expression(&index.object)?;
                self.emitter.with_span(&index.span.clone(), |emitter| {
                    emitter.push_acc("indexed object");
                });
                self.lower_expression(&index.index)?;
                let comment = self.snippet(&index.span);
                self.emitter.with_span(&index.span.clone(), |emitter| {
                    emitter.ins_cmt("mov x1, x0", "index");
                    emitter.pop("x0", "object");
                    emitter.ins_cmt("bl rt_index", &comment);
                });
                Ok(())
            }
            Expression::FUNCTION(function) => self.lower_function(function),
            Expression::FunctionCall(call) => {
                let argc = call.arguments.len();
                let area = call_area_size(argc);
                let comment = self.snippet(&call.span);
                self.emitter.with_span(&call.span.clone(), |emitter| {
                    emitter.comment(&comment);
                    emitter.sp_sub(area);
                });
                self.lower_expression(&call.callee)?;
                self.emitter.with_span(&call.span.clone(), |emitter| {
                    emitter.sp_store("x0", 0, "callee");
                });
                for (index, argument) in call.arguments.iter().enumerate() {
                    self.lower_expression(argument)?;
                    self.emitter.with_span(&call.span.clone(), |emitter| {
                        emitter.sp_store("x0", 8 * (index as u64 + 1), &format!("arg {}", index));
                    });
                }
                self.emitter.with_span(&call.span.clone(), |emitter| {
                    emitter.ins_cmt("ldr x0, [sp]", "callee");
                    emitter.load_imm64("x1", argc as u64, "argc");
                    emitter.sp_address("x2", 8, "argv");
                    emitter.ins("bl rt_call");
                    emitter.sp_add(area);
                });
                Ok(())
            }
            Expression::This(this) => {
                let symbol = match self.symbols.resolve("this".to_string()) {
                    Some(symbol) => symbol,
                    None => return error("this is only available inside a method", &this.span),
                };
                self.load_symbol(&symbol, &this.span)
            }
            Expression::Property(property) => {
                self.lower_expression(&property.object)?;
                let (name_label, name_len) = self
                    .emitter
                    .intern_string(property.property.name.as_bytes());
                let name = property.property.name.clone();
                self.emitter.with_span(&property.span.clone(), |emitter| {
                    emitter.load_label_address("x1", &name_label, &name);
                    emitter.load_imm64("x2", name_len, "");
                    emitter.ins_cmt("bl rt_get_property", &format!(".{}", name));
                });
                Ok(())
            }
            Expression::New(new_expression) => {
                let symbol = match self.symbols.resolve(new_expression.callee.name.clone()) {
                    Some(symbol) => symbol,
                    None => {
                        return error(
                            format!("undefined variable '{}'", new_expression.callee.name),
                            &new_expression.callee.span,
                        )
                    }
                };
                let argc = new_expression.arguments.len();
                let area = call_area_size(argc);
                let comment = self.snippet(&new_expression.span);
                self.emitter
                    .with_span(&new_expression.span.clone(), |emitter| {
                        emitter.comment(&comment);
                        emitter.sp_sub(area);
                    });
                self.load_symbol(&symbol, &new_expression.callee.span)?;
                self.emitter
                    .with_span(&new_expression.span.clone(), |emitter| {
                        emitter.sp_store("x0", 0, "class");
                    });
                for (index, argument) in new_expression.arguments.iter().enumerate() {
                    self.lower_expression(argument)?;
                    self.emitter
                        .with_span(&new_expression.span.clone(), |emitter| {
                            emitter.sp_store(
                                "x0",
                                8 * (index as u64 + 1),
                                &format!("arg {}", index),
                            );
                        });
                }
                // `new` lowers to rt_construct by AST node kind, never by the
                // callee's runtime type (design §7.1).
                self.emitter
                    .with_span(&new_expression.span.clone(), |emitter| {
                        emitter.ins_cmt("ldr x0, [sp]", "class");
                        emitter.load_imm64("x1", argc as u64, "argc");
                        emitter.sp_address("x2", 8, "argv");
                        emitter.ins("bl rt_construct");
                        emitter.sp_add(area);
                    });
                Ok(())
            }
        }
    }

    fn lower_literal(&mut self, literal: &Literal) -> Result<(), LowerError> {
        match literal {
            Literal::Integer(integer) => {
                let raw = integer.raw;
                self.emitter.with_span(&integer.span.clone(), |emitter| {
                    if i64_fits_smi(raw) {
                        emitter.load_imm64("x0", (raw << 1) as u64, &format!("{}", raw));
                    } else {
                        emitter.load_imm64("x0", raw as u64, &format!("{} (beyond SMI)", raw));
                        emitter.ins("bl rt_box_int");
                    }
                });
                Ok(())
            }
            Literal::Boolean(boolean) => {
                let (value, text) =
                    if boolean.raw { (TRUE_VALUE, "true") } else { (FALSE_VALUE, "false") };
                self.emitter.with_span(&boolean.span.clone(), |emitter| {
                    emitter.load_imm64("x0", value, text);
                });
                Ok(())
            }
            Literal::String(string) => {
                let (label, len) = self.emitter.intern_string(string.raw.as_bytes());
                let preview = self.snippet(&string.span);
                self.emitter.with_span(&string.span.clone(), |emitter| {
                    emitter.load_label_address("x0", &label, &preview);
                    emitter.load_imm64("x1", len, "byte length");
                    emitter.ins("bl rt_string_from_bytes");
                });
                Ok(())
            }
            Literal::Array(array) => {
                let len = array.elements.len();
                let area = scratch_area_size(len);
                self.emitter.with_span(&array.span.clone(), |emitter| {
                    emitter.sp_sub(area);
                });
                for (index, element) in array.elements.iter().enumerate() {
                    self.lower_expression(element)?;
                    self.emitter.with_span(&array.span.clone(), |emitter| {
                        emitter.sp_store("x0", 8 * index as u64, &format!("element {}", index));
                    });
                }
                self.emitter.with_span(&array.span.clone(), |emitter| {
                    emitter.sp_address("x0", 0, "element base");
                    emitter.load_imm64("x1", len as u64, "element count");
                    emitter.ins("bl rt_array");
                    emitter.sp_add(area);
                });
                Ok(())
            }
            Literal::Hash(hash) => {
                let pairs = hash.elements.len();
                let area = scratch_area_size(pairs * 2);
                self.emitter.with_span(&hash.span.clone(), |emitter| {
                    emitter.sp_sub(area);
                });
                for (index, (key, value)) in hash.elements.iter().enumerate() {
                    self.lower_expression(key)?;
                    self.emitter.with_span(&hash.span.clone(), |emitter| {
                        emitter.sp_store("x0", 8 * (2 * index as u64), &format!("key {}", index));
                    });
                    self.lower_expression(value)?;
                    self.emitter.with_span(&hash.span.clone(), |emitter| {
                        emitter.sp_store(
                            "x0",
                            8 * (2 * index as u64 + 1),
                            &format!("value {}", index),
                        );
                    });
                }
                self.emitter.with_span(&hash.span.clone(), |emitter| {
                    emitter.sp_address("x0", 0, "pair base");
                    emitter.load_imm64("x1", pairs as u64, "pair count");
                    emitter.ins("bl rt_hash");
                    emitter.sp_add(area);
                });
                Ok(())
            }
        }
    }

    fn lower_infix(&mut self, infix: &parser::ast::BinaryExpression) -> Result<(), LowerError> {
        let comment = self.snippet(&infix.span);
        self.lower_expression(&infix.left)?;
        self.emitter.with_span(&infix.span.clone(), |emitter| {
            emitter.push_acc("left operand");
        });
        self.lower_expression(&infix.right)?;

        // Preserve left-to-right evaluation. There is no rt_lt entry point,
        // so compare `right > left` after evaluating both source operands.
        if infix.op.kind == TokenKind::LT {
            self.emitter.with_span(&infix.span.clone(), |emitter| {
                emitter.pop("x1", "left operand");
                emitter.ins_cmt("bl rt_gt", &comment);
            });
            return Ok(());
        }

        let runtime_call = match infix.op.kind {
            TokenKind::PLUS => "bl rt_add",
            TokenKind::MINUS => "bl rt_sub",
            TokenKind::ASTERISK => "bl rt_mul",
            TokenKind::SLASH => "bl rt_div",
            TokenKind::GT => "bl rt_gt",
            TokenKind::EQ => "bl rt_eq",
            TokenKind::NotEq => "bl rt_neq",
            _ => return error(format!("unexpected infix op: {}", infix.op), &infix.span),
        };

        if infix.op.kind == TokenKind::PLUS {
            // SMI fast path (design §5.2): both bit0 clear, `adds` whose V
            // flag only signals SMI-range overflow; anything else falls back
            // to rt_add for checked i64 + string concat.
            let slow_label = self.emitter.new_label();
            let done_label = self.emitter.new_label();
            self.emitter.with_span(&infix.span.clone(), |emitter| {
                emitter.ins_cmt("mov x1, x0", "right operand");
                emitter.pop("x0", "left operand");
                emitter.ins_cmt("orr x8, x0, x1", "SMI check on both bit0");
                emitter.ins(&format!("tbnz x8, #0, {}", slow_label));
                emitter.ins_cmt("adds x8, x0, x1", "(a<<1)+(b<<1) = (a+b)<<1");
                emitter.ins(&format!("bvs {}", slow_label));
                emitter.ins("mov x0, x8");
                emitter.ins(&format!("b {}", done_label));
                emitter.label(&slow_label);
                emitter.ins_cmt(runtime_call, &comment);
                emitter.label(&done_label);
            });
            return Ok(());
        }

        self.emitter.with_span(&infix.span.clone(), |emitter| {
            emitter.ins_cmt("mov x1, x0", "right operand");
            emitter.pop("x0", "left operand");
            emitter.ins_cmt(runtime_call, &comment);
        });
        Ok(())
    }

    fn lower_function(&mut self, function: &FunctionDeclaration) -> Result<(), LowerError> {
        if function.params.len() > MAX_FUNCTION_PARAMETERS {
            return error(
                format!("functions accept at most {} parameters", MAX_FUNCTION_PARAMETERS),
                &function.span,
            );
        }
        self.enter_scope();
        // Named self-reference (design §7): the name resolves to the closure
        // slot instead of an outer binding.
        if !function.name.is_empty() {
            self.symbols.define_function_name(function.name.clone());
        }
        let mut parameter_names = Vec::with_capacity(function.params.len());
        for parameter in &function.params {
            self.symbols.define(parameter.name.clone());
            parameter_names.push(parameter.name.clone());
        }

        let label = self.emitter.new_function_label();
        let epilogue_label = format!("{}_ret", label);
        let display_name = if function.name.is_empty() {
            "fn".to_string()
        } else {
            format!("fn {}", function.name)
        };
        let signature = format!("{}({})", display_name, parameter_names.join(", "));

        self.epilogues.push(epilogue_label.clone());
        self.emitter.begin_function();
        self.lower_block_value(&function.body)?;
        self.epilogues.pop();

        let num_definitions = self.symbols.num_definitions;
        let free_symbols = self.symbols.free_symbols.clone();
        self.emitter.end_function(FunctionFrame {
            label: label.clone(),
            comment: signature.clone(),
            num_parameters: function.params.len(),
            num_definitions,
            epilogue_label,
            parameter_names,
        });
        self.leave_scope();

        self.emit_closure(&label, function.params.len(), &free_symbols, &signature, &function.span)
    }

    fn lower_method(
        &mut self,
        class_name: &str,
        method: &MethodDefinition,
    ) -> Result<(), LowerError> {
        if method.params.len() > MAX_METHOD_PARAMETERS {
            return error(
                format!("methods accept at most {} parameters", MAX_METHOD_PARAMETERS),
                &method.span,
            );
        }
        self.enter_scope();
        // `this` is symbol 0, before the declared parameters, matching
        // compile_method in the bytecode compiler.
        self.symbols.define("this".to_string());
        let mut parameter_names = vec!["this".to_string()];
        for parameter in &method.params {
            self.symbols.define(parameter.name.clone());
            parameter_names.push(parameter.name.clone());
        }

        let label = self.emitter.new_function_label();
        let epilogue_label = format!("{}_ret", label);
        let signature =
            format!("{}.{}({})", class_name, method.name.name, parameter_names.join(", "));

        self.epilogues.push(epilogue_label.clone());
        self.emitter.begin_function();
        self.lower_block_value(&method.body)?;
        if method.kind == MethodKind::Constructor {
            // Constructors always evaluate to their instance (design §7.2);
            // `return` inside them is already rejected by validation.
            self.emitter.without_span(|emitter| {
                emitter.frame_load("x0", slot_offset(0), "constructor returns this");
            });
        }
        self.epilogues.pop();

        let num_definitions = self.symbols.num_definitions;
        let free_symbols = self.symbols.free_symbols.clone();
        self.emitter.end_function(FunctionFrame {
            label: label.clone(),
            comment: signature.clone(),
            num_parameters: method.params.len() + 1,
            num_definitions,
            epilogue_label,
            parameter_names,
        });
        self.leave_scope();

        self.emit_closure(&label, method.params.len() + 1, &free_symbols, &signature, &method.span)
    }

    /// Builds the closure value in the parent scope: captured values are
    /// loaded in `free_symbols` order into a scratch area, then handed to
    /// `rt_closure` (design §6.1 step 4).
    fn emit_closure(
        &mut self,
        code_label: &str,
        num_parameters: usize,
        free_symbols: &[Rc<Symbol>],
        signature: &str,
        span: &Span,
    ) -> Result<(), LowerError> {
        let num_free = free_symbols.len();
        let area = scratch_area_size(num_free);
        self.emitter.with_span(&span.clone(), |emitter| {
            emitter.sp_sub(area);
        });
        for (index, symbol) in free_symbols.iter().enumerate() {
            self.load_symbol(symbol, span)?;
            let name = symbol.name.clone();
            self.emitter.with_span(&span.clone(), |emitter| {
                emitter.sp_store("x0", 8 * index as u64, &format!("capture {}", name));
            });
        }
        self.emitter.with_span(&span.clone(), |emitter| {
            emitter.load_label_address("x0", code_label, signature);
            emitter.load_imm64("x1", num_parameters as u64, "num_parameters");
            emitter.sp_address("x2", 0, "captured values");
            emitter.load_imm64("x3", num_free as u64, "num_free");
            emitter.ins("bl rt_closure");
            emitter.sp_add(area);
        });
        Ok(())
    }

    fn lower_class(&mut self, class: &ClassDeclaration) -> Result<(), LowerError> {
        let class_name = class.name.name.clone();
        let comment = format!("class {}", class_name);
        self.emitter.comment(&comment);
        // Define first so methods can reference the class (e.g. `new C()`
        // in a method body), matching the bytecode compiler.
        let symbol = self.symbols.define(class_name.clone());
        let (name_label, name_len) = self.emitter.intern_string(class_name.as_bytes());
        self.emitter.with_span(&class.span.clone(), |emitter| {
            emitter.load_label_address("x0", &name_label, &class_name);
            emitter.load_imm64("x1", name_len, "");
            emitter.ins("bl rt_class");
            emitter.push_acc("class value");
        });
        for method in &class.methods {
            self.lower_method(&class_name, method)?;
            let (method_label, method_len) =
                self.emitter.intern_string(method.name.name.as_bytes());
            let is_constructor = method.kind == MethodKind::Constructor;
            let method_name = method.name.name.clone();
            self.emitter.with_span(&method.span.clone(), |emitter| {
                emitter.ins_cmt("mov x3, x0", "method closure");
                emitter.ins_cmt("ldr x0, [sp]", "class value (kept pushed)");
                emitter.load_label_address("x1", &method_label, &method_name);
                emitter.load_imm64("x2", method_len, "");
                emitter.load_imm64("x4", if is_constructor { 1 } else { 0 }, "is_ctor");
                emitter.ins("bl rt_class_add_method");
            });
        }
        self.emitter.with_span(&class.span.clone(), |emitter| {
            emitter.pop("x0", "class value");
        });
        let span = class.span.clone();
        self.emitter.with_span(&span, |emitter| match symbol.scope {
            SymbolScope::Global => {
                emitter.global_store("x0", symbol.index, &format!("class {}", class_name))
            }
            _ => emitter.frame_store(
                "x0",
                slot_offset(symbol.index),
                &format!("class {}", class_name),
            ),
        });
        Ok(())
    }
}
