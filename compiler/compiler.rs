use object::builtins::BuiltIns;
use serde::Serialize;
use std::collections::HashMap;
use std::rc::Rc;

use object::Object;
use parser::ast::{
    BlockStatement, Expression, Literal, MethodDefinition, MethodKind, Node, Statement,
};
use parser::lexer::token::Span;
use parser::lexer::token::TokenKind;
use parser::validation::validate_program;

use crate::op_code::Opcode::*;
use crate::op_code::{make_instructions, Instructions, Opcode};
use crate::symbol_table::{Symbol, SymbolScope, SymbolTable};

struct CompilationScope {
    instructions: Instructions,
    last_instruction: EmittedInstruction,
    previous_instruction: EmittedInstruction,
    debug_info: DebugInfo,
}

pub struct Compiler {
    pub constants: Vec<Rc<Object>>,
    pub symbol_table: SymbolTable,
    function_debug_info: HashMap<usize, DebugInfo>,
    scopes: Vec<CompilationScope>,
    scope_index: usize,
    callable_kinds: Vec<CallableKind>,
}

#[derive(Debug, PartialEq)]
pub struct Bytecode {
    pub instructions: Instructions,
    pub constants: Vec<Rc<Object>>,
    pub debug_info: DebugInfo,
    pub function_debug_info: HashMap<usize, DebugInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PcSpan {
    pub pc: usize,
    pub span: Span,
}

/// One named slot in a frame's locals or the VM's globals.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingDebugInfo {
    pub name: String,
    pub slot: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugInfo {
    pub pc_spans: Vec<PcSpan>,
    /// Parameters (`this` first for methods) then `let`s, strictly increasing
    /// by slot. Empty for main, whose bindings are the globals.
    pub local_bindings: Vec<BindingDebugInfo>,
    /// Captured names aligned with `GcClosure.free` / `OpGetFree` indices.
    pub free_names: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InstructionScope {
    Main,
    Function { constant_index: usize },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionLineMapping {
    pub line: usize,
    pub pc: usize,
    pub scope: InstructionScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BytecodeDebugView {
    pub detail: String,
    pub main_debug_info: DebugInfo,
    pub function_debug_info: HashMap<usize, DebugInfo>,
    pub instruction_lines: Vec<InstructionLineMapping>,
}

struct ScopedInstructions {
    instructions: Instructions,
    debug_info: DebugInfo,
}

impl Bytecode {
    pub fn string(&self) -> String {
        self.debug_view().detail
    }

    pub fn debug_view(&self) -> BytecodeDebugView {
        let mut builder = BytecodeDisplayBuilder::new();

        builder.write_line("Instructions:");
        for line in self.instructions.string().lines() {
            builder
                .write_instruction_line(line, InstructionScope::Main, |line| format!("{line}\n"));
        }

        builder.write_line("");
        builder.write_line("Constants:");

        if self.constants.is_empty() {
            builder.write_line("(none)");
        } else {
            for (index, constant) in self.constants.iter().enumerate() {
                match constant.as_ref() {
                    Object::CompiledFunction(function) => {
                        let name = if function.name.is_empty() {
                            "<anonymous>"
                        } else {
                            function.name.as_str()
                        };
                        builder.write_line(&format!(
                            "{index:04} CompiledFunction(name={name}, num_locals={}, num_parameters={})",
                            function.num_locals,
                            function.num_parameters
                        ));
                        builder.write_line("     Instructions:");

                        let instructions = Instructions {
                            data: function.instructions.clone(),
                        };
                        let scope = InstructionScope::Function {
                            constant_index: index,
                        };
                        for line in instructions.string().lines() {
                            builder.write_instruction_line(line, scope.clone(), |line| {
                                format!("       {line}\n")
                            });
                        }
                    }
                    value => builder.write_line(&format!("{index:04} {value}")),
                }
            }
        }

        BytecodeDebugView {
            detail: builder.output,
            main_debug_info: self.debug_info.clone(),
            function_debug_info: self.function_debug_info.clone(),
            instruction_lines: builder.instruction_lines,
        }
    }
}

struct BytecodeDisplayBuilder {
    output: String,
    line: usize,
    instruction_lines: Vec<InstructionLineMapping>,
}

impl BytecodeDisplayBuilder {
    fn new() -> Self {
        Self {
            output: String::new(),
            line: 0,
            instruction_lines: vec![],
        }
    }

    fn write_line(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
        self.line += 1;
    }

    fn write_instruction_line(
        &mut self,
        raw_line: &str,
        scope: InstructionScope,
        format_line: impl FnOnce(&str) -> String,
    ) {
        if let Some(pc) = parse_instruction_pc(raw_line) {
            self.instruction_lines.push(InstructionLineMapping {
                line: self.line,
                pc,
                scope,
            });
        }

        self.output.push_str(&format_line(raw_line));
        self.line += 1;
    }
}

/// Splits a statement list at its final run of `debugger` statements, which
/// are completion-transparent and execute after the block's value is decided.
fn split_trailing_debuggers(body: &[Statement]) -> (&[Statement], &[Statement]) {
    let split = body
        .iter()
        .rposition(|statement| !matches!(statement, Statement::Debugger(_)))
        .map_or(0, |index| index + 1);
    body.split_at(split)
}

fn parse_instruction_pc(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if trimmed.len() < 4 {
        return None;
    }

    let pc_part = &trimmed[..4];
    if !pc_part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    pc_part.parse().ok()
}

impl DebugInfo {
    pub fn add_pc_span(&mut self, pc: usize, span: &Span) {
        if self
            .pc_spans
            .last()
            .map(|last| last.span == *span)
            .unwrap_or(false)
        {
            return;
        }

        self.pc_spans.push(PcSpan {
            pc,
            span: span.clone(),
        });
    }

    pub fn span_for_pc(&self, pc: usize) -> Option<&Span> {
        self.pc_spans
            .iter()
            .rev()
            .find(|pc_span| pc_span.pc <= pc)
            .map(|pc_span| &pc_span.span)
    }

    fn truncate_from_pc(&mut self, pc: usize) {
        self.pc_spans.retain(|pc_span| pc_span.pc < pc);
    }
}

#[derive(Clone)]
pub struct EmittedInstruction {
    pub opcode: Opcode,
    pub position: usize,
}

type CompileError = String;

/// Bytecode operand widths are fixed (u8 / u16). Without these checks,
/// `make_instructions` silently truncates oversized operands and the VM
/// reads the wrong slot/constant — a silent miscompile.
///
/// The `_count` and `_index` variants exist so the numbers in the error text
/// are always counts of things the user wrote. Passing a zero-based operand
/// index to a `_count` helper would report `256 exceeds ... 255` for what is
/// really the 257th local.
fn ensure_count(count: usize, max: usize, what: &str) -> Result<(), CompileError> {
    if count > max {
        return Err(format!("too many {}: {} exceeds the maximum of {}", what, count, max));
    }
    return Ok(());
}

/// For operands that hold a count directly (call arguments, array elements).
fn ensure_u8_count(count: usize, what: &str) -> Result<(), CompileError> {
    return ensure_count(count, u8::MAX as usize, what);
}

fn ensure_u16_count(count: usize, what: &str) -> Result<(), CompileError> {
    return ensure_count(count, u16::MAX as usize, what);
}

/// For operands that hold a zero-based index (locals, globals, constants).
/// `index` items already exist, so the one being added is number `index + 1`
/// and the encodable maximum is one more than the largest index.
fn ensure_u8_index(index: usize, what: &str) -> Result<(), CompileError> {
    return ensure_count(index + 1, u8::MAX as usize + 1, what);
}

fn ensure_u16_index(index: usize, what: &str) -> Result<(), CompileError> {
    return ensure_count(index + 1, u16::MAX as usize + 1, what);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableKind {
    Function,
    Method,
    Constructor,
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Compiler {
        let main_scope = CompilationScope {
            instructions: Instructions {
                data: vec![],
            },
            last_instruction: EmittedInstruction {
                opcode: OpNull,
                position: 0,
            },
            previous_instruction: EmittedInstruction {
                opcode: OpNull,
                position: 0,
            },
            debug_info: DebugInfo::default(),
        };

        let mut symbol_table = SymbolTable::new();
        for (key, value) in BuiltIns.iter().enumerate() {
            symbol_table.define_builtin(key, value.name.to_string());
        }

        return Compiler {
            constants: vec![],
            symbol_table,
            function_debug_info: HashMap::new(),
            scopes: vec![main_scope],
            scope_index: 0,
            callable_kinds: vec![],
        };
    }

    pub fn new_with_state(symbol_table: SymbolTable, constants: Vec<Rc<Object>>) -> Compiler {
        let mut compiler = Compiler::new();
        compiler.constants = constants;
        compiler.symbol_table = symbol_table;
        return compiler;
    }

    pub fn compile(&mut self, node: &Node) -> Result<Bytecode, CompileError> {
        match node {
            Node::Program(p) => {
                let mut predefined_names = self.symbol_table.visible_names();
                predefined_names.extend(BuiltIns.iter().map(|builtin| builtin.name.to_string()));
                let predefined_names = predefined_names
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                validate_program(p, &predefined_names).map_err(|error| error.message)?;
                for stmt in &p.body {
                    self.compile_stmt(stmt)?;
                }
            }
            Node::Statement(s) => {
                self.compile_stmt(s)?;
            }
            Node::Expression(e) => {
                self.compile_expr(e)?;
            }
        }

        return Ok(self.bytecode());
    }

    fn compile_stmt(&mut self, s: &Statement) -> Result<(), CompileError> {
        match s {
            Statement::Let(let_statement) => {
                // A rebinding's RHS resolves against the preceding lexical
                // environment. Named recursion is provided by Function scope
                // inside the function body, not by an uninitialized slot.
                self.compile_expr(&let_statement.expr)?;
                let symbol = self.define_symbol(let_statement.identifier.kind.to_string())?;
                if symbol.scope == SymbolScope::Global {
                    self.emit_with_span(Opcode::OpSetGlobal, &[symbol.index], &let_statement.span);
                } else {
                    self.emit_with_span(Opcode::OpSetLocal, &[symbol.index], &let_statement.span);
                }
                return Ok(());
            }
            Statement::Return(r) => {
                if self.callable_kinds.last() == Some(&CallableKind::Constructor) {
                    return Err("constructor cannot return a value".to_string());
                }
                self.compile_expr(&r.argument)?;
                self.emit_with_span(Opcode::OpReturnValue, &[], &r.span);
                return Ok(());
            }
            Statement::Expr(e) => {
                self.compile_expr(e)?;
                self.emit_with_span(OpPop, &[], e.span());
                return Ok(());
            }
            Statement::Class(class) => {
                let symbol = self.define_symbol(class.name.name.clone())?;
                let class_name = self.try_add_constant(Object::String(class.name.name.clone()))?;
                self.emit_with_span(OpClass, &[class_name], &class.span);

                for method in &class.methods {
                    self.compile_method(&class.name.name, method)?;
                    let method_name =
                        self.try_add_constant(Object::String(method.name.name.clone()))?;
                    let kind = match method.kind {
                        MethodKind::Method => 0,
                        MethodKind::Constructor => 1,
                    };
                    self.emit_with_span(OpMethod, &[method_name, kind], &method.span);
                }

                self.emit_with_span(OpSetGlobal, &[symbol.index], &class.span);
                self.emit_with_span(OpNull, &[], &class.span);
                self.emit_with_span(OpPop, &[], &class.span);
                Ok(())
            }
            Statement::SetProperty(statement) => {
                self.compile_expr(&statement.object)?;
                self.compile_expr(&statement.value)?;
                let property =
                    self.try_add_constant(Object::String(statement.property.name.clone()))?;
                self.emit_with_span(OpSetProperty, &[property], &statement.span);
                self.emit_with_span(OpNull, &[], &statement.span);
                self.emit_with_span(OpPop, &[], &statement.span);
                Ok(())
            }
            Statement::Debugger(statement) => {
                self.emit_with_span(OpDebugger, &[], &statement.span);
                Ok(())
            }
        }
    }

    fn compile_expr(&mut self, e: &Expression) -> Result<(), CompileError> {
        match e {
            Expression::IDENTIFIER(identifier) => {
                let symbol = self.symbol_table.resolve(identifier.name.clone());
                match symbol {
                    Some(symbol) => {
                        self.load_symbol(&symbol, &identifier.span)?;
                    }
                    None => {
                        return Err(format!("Undefined variable '{}'", identifier.name));
                    }
                }
            }
            Expression::LITERAL(l) => match l {
                Literal::Integer(i) => {
                    let int = Object::Integer(i.raw);
                    let operands = vec![self.try_add_constant(int)?];
                    self.emit_with_span(OpConst, &operands, &i.span);
                }
                Literal::Boolean(i) => {
                    if i.raw {
                        self.emit_with_span(OpTrue, &[], &i.span);
                    } else {
                        self.emit_with_span(OpFalse, &[], &i.span);
                    }
                }
                Literal::String(s) => {
                    let string_object = Object::String(s.raw.clone());
                    let operands = vec![self.try_add_constant(string_object)?];
                    self.emit_with_span(OpConst, &operands, &s.span);
                }
                Literal::Array(array) => {
                    for element in array.elements.iter() {
                        self.compile_expr(element)?;
                    }
                    ensure_u16_count(array.elements.len(), "array elements")?;
                    self.emit_with_span(OpArray, &[array.elements.len()], &array.span);
                }
                Literal::Hash(hash) => {
                    for (key, value) in hash.elements.iter() {
                        self.compile_expr(key)?;
                        self.compile_expr(value)?;
                    }
                    // OpHash counts keys and values, so the encodable operand is
                    // always even and the last usable one is u16::MAX - 1. Check
                    // the pair count the user actually wrote, not the doubled
                    // operand, or the message reports twice the real limit.
                    ensure_count(hash.elements.len(), u16::MAX as usize / 2, "hash pairs")?;
                    self.emit_with_span(OpHash, &[hash.elements.len() * 2], &hash.span);
                }
            },
            Expression::PREFIX(prefix) => {
                self.compile_expr(&prefix.operand)?;
                match prefix.op.kind {
                    TokenKind::MINUS => {
                        self.emit_with_span(OpMinus, &[], &prefix.span);
                    }
                    TokenKind::BANG => {
                        self.emit_with_span(OpBang, &[], &prefix.span);
                    }
                    _ => {
                        return Err(format!("unexpected prefix op: {}", prefix.op));
                    }
                }
            }
            Expression::INFIX(infix) => {
                self.compile_expr(&infix.left)?;
                self.compile_expr(&infix.right)?;
                match infix.op.kind {
                    TokenKind::PLUS => {
                        self.emit_with_span(OpAdd, &[], &infix.span);
                    }
                    TokenKind::MINUS => {
                        self.emit_with_span(OpSub, &[], &infix.span);
                    }
                    TokenKind::ASTERISK => {
                        self.emit_with_span(OpMul, &[], &infix.span);
                    }
                    TokenKind::SLASH => {
                        self.emit_with_span(OpDiv, &[], &infix.span);
                    }
                    TokenKind::GT => {
                        self.emit_with_span(Opcode::OpGreaterThan, &[], &infix.span);
                    }
                    TokenKind::LT => {
                        self.emit_with_span(Opcode::OpLessThan, &[], &infix.span);
                    }
                    TokenKind::EQ => {
                        self.emit_with_span(Opcode::OpEqual, &[], &infix.span);
                    }
                    TokenKind::NotEq => {
                        self.emit_with_span(Opcode::OpNotEqual, &[], &infix.span);
                    }
                    _ => {
                        return Err(format!("unexpected infix op: {}", infix.op));
                    }
                }
            }
            Expression::IF(if_node) => {
                self.compile_expr(&if_node.condition)?;
                let jump_not_truthy = self.emit_with_span(OpJumpNotTruthy, &[9527], &if_node.span);
                self.compile_block_statement_as_value(&if_node.consequent)?;

                let jump_pos = self.emit_with_span(OpJump, &[9527], &if_node.span);

                let after_consequence_location = self.current_instruction().data.len();
                self.change_operand(jump_not_truthy, after_consequence_location)?;

                if let Some(alternate) = &if_node.alternate {
                    self.compile_block_statement_as_value(alternate)?;
                } else {
                    self.emit_with_span(OpNull, &[], &if_node.span);
                }
                let after_alternative_location = self.current_instruction().data.len();
                self.change_operand(jump_pos, after_alternative_location)?;
            }
            Expression::Index(index) => {
                self.compile_expr(&index.object)?;
                self.compile_expr(&index.index)?;
                self.emit_with_span(OpIndex, &[], &index.span);
            }
            Expression::FUNCTION(f) => {
                let function_span = f.span.clone();
                self.enter_scope();
                self.callable_kinds.push(CallableKind::Function);
                if !f.name.is_empty() {
                    self.symbol_table.define_function_name(f.name.clone());
                }
                for param in f.params.iter() {
                    self.define_symbol(param.name.clone())?;
                }
                self.compile_function_body(&f.body, &function_span)?;
                let num_locals = self.symbol_table.num_definitions;
                let free_symbols = self.symbol_table.free_symbols.clone();
                let scoped_instructions = self.leave_scope();
                self.callable_kinds.pop();
                // Checked before the loads so the count is the only free-variable
                // limit a user can hit. OpGetFree's u8 slot allows one more than
                // OpClosure's u8 count does, and reporting that larger number
                // would name a limit no closure can actually reach.
                ensure_u8_count(free_symbols.len(), "free variables")?;
                for x in free_symbols.clone() {
                    self.load_symbol(&x, &function_span)?;
                }

                let compiled_function = Rc::from(object::CompiledFunction {
                    name: f.name.clone(),
                    instructions: scoped_instructions.instructions.data,
                    num_locals,
                    num_parameters: f.params.len(),
                });

                let constant_index =
                    self.try_add_constant(Object::CompiledFunction(compiled_function))?;
                self.function_debug_info_mut()
                    .insert(constant_index, scoped_instructions.debug_info);
                let operands = vec![constant_index, free_symbols.len()];
                self.emit_with_span(OpClosure, &operands, &function_span);
            }
            Expression::FunctionCall(fc) => {
                self.compile_expr(&fc.callee)?;
                for arg in fc.arguments.iter() {
                    self.compile_expr(arg)?;
                }
                ensure_u8_count(fc.arguments.len(), "call arguments")?;
                self.emit_with_span(OpCall, &[fc.arguments.len()], &fc.span);
            }
            Expression::This(this) => {
                let symbol = self
                    .symbol_table
                    .resolve("this".to_string())
                    .ok_or_else(|| "this is only available inside a method".to_string())?;
                self.load_symbol(&symbol, &this.span)?;
            }
            Expression::Property(property) => {
                self.compile_expr(&property.object)?;
                let name = self.try_add_constant(Object::String(property.property.name.clone()))?;
                self.emit_with_span(OpGetProperty, &[name], &property.span);
            }
            Expression::New(new_expression) => {
                let symbol = self
                    .symbol_table
                    .resolve(new_expression.callee.name.clone())
                    .ok_or_else(|| {
                        format!("Undefined variable '{}'", new_expression.callee.name)
                    })?;
                self.load_symbol(&symbol, &new_expression.callee.span)?;
                for argument in &new_expression.arguments {
                    self.compile_expr(argument)?;
                }
                ensure_u8_count(new_expression.arguments.len(), "constructor arguments")?;
                self.emit_with_span(OpNew, &[new_expression.arguments.len()], &new_expression.span);
            }
        }

        return Ok(());
    }

    /// Only the Free arm can fail: locals and globals were bounded by
    /// `define_symbol`, builtins come from a fixed table, and Function is
    /// always slot 0.
    fn load_symbol(&mut self, symbol: &Rc<Symbol>, span: &Span) -> Result<(), CompileError> {
        match symbol.scope {
            SymbolScope::Global => {
                self.emit_with_span(OpGetGlobal, &[symbol.index], span);
            }
            SymbolScope::LOCAL => {
                self.emit_with_span(OpGetLocal, &[symbol.index], span);
            }
            SymbolScope::Builtin => {
                self.emit_with_span(OpGetBuiltin, &[symbol.index], span);
            }
            SymbolScope::Free => {
                // Free slots are assigned by resolve() while the body compiles,
                // so this fires long before the capture list is emitted. Check
                // it against OpClosure's u8 *count*, which caps at 255, not
                // against OpGetFree's u8 slot, which would allow one more than
                // any closure can actually carry.
                ensure_u8_count(symbol.index + 1, "free variables")?;
                self.emit_with_span(OpGetFree, &[symbol.index], span);
            }
            SymbolScope::Function => {
                self.emit_with_span(OpCurrentClosure, &[], span);
            }
        }
        return Ok(());
    }

    pub fn bytecode(&self) -> Bytecode {
        return Bytecode {
            instructions: self.current_instruction().clone(),
            constants: self.constants.clone(),
            debug_info: self.current_debug_info().clone(),
            function_debug_info: self.function_debug_info.clone(),
        };
    }

    fn define_symbol(&mut self, name: String) -> Result<Rc<Symbol>, CompileError> {
        let symbol = self.symbol_table.define(name);
        match symbol.scope {
            SymbolScope::LOCAL => ensure_u8_index(symbol.index, "locals")?,
            SymbolScope::Global => ensure_u16_index(symbol.index, "globals")?,
            // Builtin indexes come from a fixed compile-time table well under
            // u8::MAX, Function is always 0, and Free is assigned by resolve()
            // rather than here — bounded by the OpClosure capture count.
            SymbolScope::Builtin | SymbolScope::Free | SymbolScope::Function => {}
        }
        Ok(symbol)
    }

    /// Global slots in slot order, one entry per definition — a rebound name
    /// appears once for every slot it ever occupied.
    pub fn global_bindings(&self) -> Vec<BindingDebugInfo> {
        self.symbol_table
            .global_definitions()
            .iter()
            .map(|symbol| BindingDebugInfo {
                name: symbol.name.clone(),
                slot: symbol.index,
            })
            .collect()
    }

    /// Kept infallible for the published 1.1.0 signature. Prefer
    /// [`Compiler::try_add_constant`], which rejects a pool too large for
    /// `OpConst`'s u16 operand instead of handing back an index that truncates.
    pub fn add_constant(&mut self, obj: Object) -> usize {
        self.constants.push(Rc::new(obj));
        return self.constants.len() - 1;
    }

    pub fn try_add_constant(&mut self, obj: Object) -> Result<usize, CompileError> {
        ensure_u16_index(self.constants.len(), "constants")?;
        return Ok(self.add_constant(obj));
    }

    pub fn emit(&mut self, op: Opcode, operands: &[usize]) -> usize {
        let ins = make_instructions(op, operands);
        let pos = self.add_instructions(&ins);
        self.set_last_instruction(op, pos);

        return pos;
    }

    pub fn emit_with_span(&mut self, op: Opcode, operands: &[usize], span: &Span) -> usize {
        let pos = self.emit(op, operands);
        self.add_pc_span(pos, span);
        pos
    }

    fn compile_block_statement(
        &mut self,
        block_statement: &BlockStatement,
    ) -> Result<(), CompileError> {
        for stmt in &block_statement.body {
            self.compile_stmt(stmt)?;
        }
        Ok(())
    }

    fn compile_block_statement_as_value(
        &mut self,
        block_statement: &BlockStatement,
    ) -> Result<(), CompileError> {
        // Trailing `debugger` statements are completion-transparent: the
        // block's value (or null) is decided before they execute, and
        // OpDebugger leaves the stack untouched, so a kept value stays on top.
        let (leading, trailing_debuggers) = split_trailing_debuggers(&block_statement.body);
        let block_start = self.current_instruction().data.len();
        for stmt in leading {
            self.compile_stmt(stmt)?;
        }
        // A block in expression position must leave one value on every
        // fallthrough path. Statement-only and empty blocks evaluate to null.
        let has_value =
            self.current_instruction().data.len() > block_start && self.last_instruction_is(OpPop);
        if has_value {
            self.remove_last_pop();
        }
        for stmt in trailing_debuggers {
            self.compile_stmt(stmt)?;
        }
        if !has_value {
            self.emit_with_span(OpNull, &[], &block_statement.span);
        }
        Ok(())
    }

    /// Compiles a function or method body plus its implicit return. Trailing
    /// `debugger` statements must not break the "last expression statement is
    /// the return value" rule, so the value is unpopped before they execute
    /// and returned after them.
    fn compile_function_body(
        &mut self,
        body: &BlockStatement,
        span: &Span,
    ) -> Result<(), CompileError> {
        let (leading, trailing_debuggers) = split_trailing_debuggers(&body.body);
        if trailing_debuggers.is_empty() {
            self.compile_block_statement(body)?;
            if self.last_instruction_is(OpPop) {
                self.replace_last_pop_with_return();
            }
            if !(self.last_instruction_is(OpReturnValue)) {
                self.emit_with_span(OpReturn, &[], span);
            }
            return Ok(());
        }

        for stmt in leading {
            self.compile_stmt(stmt)?;
        }
        let produced_value = self.last_instruction_is(OpPop);
        if produced_value {
            self.remove_last_pop();
        }
        for stmt in trailing_debuggers {
            self.compile_stmt(stmt)?;
        }
        if produced_value {
            self.emit_with_span(OpReturnValue, &[], span);
        } else {
            self.emit_with_span(OpReturn, &[], span);
        }
        Ok(())
    }

    fn compile_method(
        &mut self,
        class_name: &str,
        method: &MethodDefinition,
    ) -> Result<(), CompileError> {
        let method_span = method.span.clone();
        self.enter_scope();
        let callable_kind = match method.kind {
            MethodKind::Method => CallableKind::Method,
            MethodKind::Constructor => CallableKind::Constructor,
        };
        self.callable_kinds.push(callable_kind);

        self.define_symbol("this".to_string())?;
        for parameter in &method.params {
            self.define_symbol(parameter.name.clone())?;
        }

        match method.kind {
            MethodKind::Constructor => {
                // A trailing debugger needs no special handling here: the
                // constructor's `this` return is appended after the body.
                self.compile_block_statement(&method.body)?;
                self.emit_with_span(OpGetLocal, &[0], &method_span);
                self.emit_with_span(OpReturnValue, &[], &method_span);
            }
            MethodKind::Method => {
                self.compile_function_body(&method.body, &method_span)?;
            }
        }

        let num_locals = self.symbol_table.num_definitions;
        let free_symbols = self.symbol_table.free_symbols.clone();
        let scoped_instructions = self.leave_scope();
        self.callable_kinds.pop();
        ensure_u8_count(free_symbols.len(), "free variables")?;
        for symbol in &free_symbols {
            self.load_symbol(symbol, &method_span)?;
        }

        let compiled_function = Rc::new(object::CompiledFunction {
            name: format!("{}.{}", class_name, method.name.name),
            instructions: scoped_instructions.instructions.data,
            num_locals,
            num_parameters: method.params.len() + 1,
        });
        let constant_index = self.try_add_constant(Object::CompiledFunction(compiled_function))?;
        self.function_debug_info_mut()
            .insert(constant_index, scoped_instructions.debug_info);
        self.emit_with_span(OpClosure, &[constant_index, free_symbols.len()], &method_span);
        Ok(())
    }

    pub fn add_instructions(&mut self, ins: &Instructions) -> usize {
        let pos = self.current_instruction().data.len();
        let updated_ins = self.scopes[self.scope_index]
            .instructions
            .merge_instructions(ins);
        self.scopes[self.scope_index].instructions = updated_ins;
        return pos;
    }

    fn set_last_instruction(&mut self, op: Opcode, pos: usize) {
        let previous_instruction = self.scopes[self.scope_index].last_instruction.clone();
        let last_instruction = EmittedInstruction {
            opcode: op,
            position: pos,
        };
        self.scopes[self.scope_index].last_instruction = last_instruction;
        self.scopes[self.scope_index].previous_instruction = previous_instruction;
    }

    fn last_instruction_is(&self, op: Opcode) -> bool {
        if self.current_instruction().data.is_empty() {
            return false;
        }
        return self.scopes[self.scope_index].last_instruction.opcode == op;
    }

    fn remove_last_pop(&mut self) {
        let last = self.scopes[self.scope_index].last_instruction.clone();
        let previous = self.scopes[self.scope_index].previous_instruction.clone();

        let old = self.current_instruction().data.clone();
        let new = old[..last.position].to_vec();

        self.scopes[self.scope_index].instructions.data = new;
        self.scopes[self.scope_index]
            .debug_info
            .truncate_from_pc(last.position);
        self.scopes[self.scope_index].last_instruction = previous;
    }

    fn replace_instruction(&mut self, pos: usize, new_instruction: &Instructions) {
        let ins = &mut self.scopes[self.scope_index].instructions;
        for i in 0..new_instruction.data.len() {
            ins.data[pos + i] = new_instruction.data[i];
        }
    }

    fn replace_last_pop_with_return(&mut self) {
        let last_pos = self.scopes[self.scope_index].last_instruction.position;
        self.replace_instruction(last_pos, &make_instructions(OpReturnValue, &[]));
        self.scopes[self.scope_index].last_instruction.opcode = OpReturnValue;
    }

    fn change_operand(&mut self, pos: usize, operand: usize) -> Result<(), CompileError> {
        // Jump operands are byte offsets into the enclosing instruction stream,
        // not a count of anything the user wrote, so they get their own message.
        if operand > u16::MAX as usize {
            return Err(format!(
                "compiled code too large: jump target at byte {} is outside the {}-byte range of a jump operand",
                operand,
                u16::MAX
            ));
        }
        let op = Opcode::from_repr(self.current_instruction().data[pos])
            .expect("compiler emitted an unknown opcode");
        let ins = make_instructions(op, &[operand]);
        self.replace_instruction(pos, &ins);
        Ok(())
    }

    fn current_instruction(&self) -> &Instructions {
        return &self.scopes[self.scope_index].instructions;
    }

    fn current_debug_info(&self) -> &DebugInfo {
        return &self.scopes[self.scope_index].debug_info;
    }

    fn function_debug_info_mut(&mut self) -> &mut HashMap<usize, DebugInfo> {
        return &mut self.function_debug_info;
    }

    fn add_pc_span(&mut self, pc: usize, span: &Span) {
        self.scopes[self.scope_index]
            .debug_info
            .add_pc_span(pc, span);
    }

    fn enter_scope(&mut self) {
        let scope = CompilationScope {
            instructions: Instructions {
                data: vec![],
            },
            last_instruction: EmittedInstruction {
                opcode: OpNull,
                position: 0,
            },
            previous_instruction: EmittedInstruction {
                opcode: OpNull,
                position: 0,
            },
            debug_info: DebugInfo::default(),
        };
        self.scopes.push(scope);
        self.scope_index += 1;
        self.symbol_table = SymbolTable::new_enclosed_symbol_table(self.symbol_table.clone());
    }

    fn leave_scope(&mut self) -> ScopedInstructions {
        let instructions = self.current_instruction().clone();
        let mut debug_info = self.current_debug_info().clone();
        // The scope's definition ledger is final here: `definitions[i].index == i`,
        // so the copied bindings come out strictly increasing by slot.
        debug_info.local_bindings = self
            .symbol_table
            .definitions
            .iter()
            .map(|symbol| BindingDebugInfo {
                name: symbol.name.clone(),
                slot: symbol.index,
            })
            .collect();
        debug_info.free_names = self
            .symbol_table
            .free_symbols
            .iter()
            .map(|symbol| symbol.name.clone())
            .collect();
        self.scopes.pop();
        self.scope_index -= 1;
        let s = self.symbol_table.outer.as_ref().unwrap().as_ref().clone();
        self.symbol_table = s;
        return ScopedInstructions {
            instructions,
            debug_info,
        };
    }
}
