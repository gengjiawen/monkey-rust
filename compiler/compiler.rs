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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugInfo {
    pub pc_spans: Vec<PcSpan>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableKind {
    Function,
    Method,
    Constructor,
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
                let symbol = self
                    .symbol_table
                    .define(let_statement.identifier.kind.to_string());
                self.compile_expr(&let_statement.expr)?;
                if symbol.scope == SymbolScope::Global {
                    self.emit_with_span(
                        Opcode::OpSetGlobal,
                        &vec![symbol.index],
                        &let_statement.span,
                    );
                } else {
                    self.emit_with_span(
                        Opcode::OpSetLocal,
                        &vec![symbol.index],
                        &let_statement.span,
                    );
                }
                return Ok(());
            }
            Statement::Return(r) => {
                if self.callable_kinds.last() == Some(&CallableKind::Constructor) {
                    return Err("constructor cannot return a value".to_string());
                }
                self.compile_expr(&r.argument)?;
                self.emit_with_span(Opcode::OpReturnValue, &vec![], &r.span);
                return Ok(());
            }
            Statement::Expr(e) => {
                self.compile_expr(e)?;
                self.emit_with_span(OpPop, &vec![], e.span());
                return Ok(());
            }
            Statement::Class(class) => {
                let symbol = self.symbol_table.define(class.name.name.clone());
                let class_name = self.add_constant(Object::String(class.name.name.clone()));
                self.emit_with_span(OpClass, &vec![class_name], &class.span);

                for method in &class.methods {
                    self.compile_method(&class.name.name, method)?;
                    let method_name = self.add_constant(Object::String(method.name.name.clone()));
                    let kind = match method.kind {
                        MethodKind::Method => 0,
                        MethodKind::Constructor => 1,
                    };
                    self.emit_with_span(OpMethod, &vec![method_name, kind], &method.span);
                }

                self.emit_with_span(OpSetGlobal, &vec![symbol.index], &class.span);
                self.emit_with_span(OpNull, &vec![], &class.span);
                self.emit_with_span(OpPop, &vec![], &class.span);
                Ok(())
            }
            Statement::SetProperty(statement) => {
                self.compile_expr(&statement.object)?;
                self.compile_expr(&statement.value)?;
                let property = self.add_constant(Object::String(statement.property.name.clone()));
                self.emit_with_span(OpSetProperty, &vec![property], &statement.span);
                self.emit_with_span(OpNull, &vec![], &statement.span);
                self.emit_with_span(OpPop, &vec![], &statement.span);
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
                        self.load_symbol(&symbol, &identifier.span);
                    }
                    None => {
                        return Err(format!("Undefined variable '{}'", identifier.name));
                    }
                }
            }
            Expression::LITERAL(l) => match l {
                Literal::Integer(i) => {
                    let int = Object::Integer(i.raw);
                    let operands = vec![self.add_constant(int)];
                    self.emit_with_span(OpConst, &operands, &i.span);
                }
                Literal::Boolean(i) => {
                    if i.raw {
                        self.emit_with_span(OpTrue, &vec![], &i.span);
                    } else {
                        self.emit_with_span(OpFalse, &vec![], &i.span);
                    }
                }
                Literal::String(s) => {
                    let string_object = Object::String(s.raw.clone());
                    let operands = vec![self.add_constant(string_object)];
                    self.emit_with_span(OpConst, &operands, &s.span);
                }
                Literal::Array(array) => {
                    for element in array.elements.iter() {
                        self.compile_expr(element)?;
                    }
                    self.emit_with_span(OpArray, &vec![array.elements.len()], &array.span);
                }
                Literal::Hash(hash) => {
                    for (key, value) in hash.elements.iter() {
                        self.compile_expr(&key)?;
                        self.compile_expr(&value)?;
                    }
                    self.emit_with_span(OpHash, &vec![hash.elements.len() * 2], &hash.span);
                }
            },
            Expression::PREFIX(prefix) => {
                self.compile_expr(&prefix.operand)?;
                match prefix.op.kind {
                    TokenKind::MINUS => {
                        self.emit_with_span(OpMinus, &vec![], &prefix.span);
                    }
                    TokenKind::BANG => {
                        self.emit_with_span(OpBang, &vec![], &prefix.span);
                    }
                    _ => {
                        return Err(format!("unexpected prefix op: {}", prefix.op));
                    }
                }
            }
            Expression::INFIX(infix) => {
                if infix.op.kind == TokenKind::LT {
                    self.compile_expr(&infix.right)?;
                    self.compile_expr(&infix.left)?;
                    self.emit_with_span(Opcode::OpGreaterThan, &vec![], &infix.span);
                    return Ok(());
                }
                self.compile_expr(&infix.left)?;
                self.compile_expr(&infix.right)?;
                match infix.op.kind {
                    TokenKind::PLUS => {
                        self.emit_with_span(OpAdd, &vec![], &infix.span);
                    }
                    TokenKind::MINUS => {
                        self.emit_with_span(OpSub, &vec![], &infix.span);
                    }
                    TokenKind::ASTERISK => {
                        self.emit_with_span(OpMul, &vec![], &infix.span);
                    }
                    TokenKind::SLASH => {
                        self.emit_with_span(OpDiv, &vec![], &infix.span);
                    }
                    TokenKind::GT => {
                        self.emit_with_span(Opcode::OpGreaterThan, &vec![], &infix.span);
                    }
                    TokenKind::EQ => {
                        self.emit_with_span(Opcode::OpEqual, &vec![], &infix.span);
                    }
                    TokenKind::NotEq => {
                        self.emit_with_span(Opcode::OpNotEqual, &vec![], &infix.span);
                    }
                    _ => {
                        return Err(format!("unexpected infix op: {}", infix.op));
                    }
                }
            }
            Expression::IF(if_node) => {
                self.compile_expr(&if_node.condition)?;
                let jump_not_truthy =
                    self.emit_with_span(OpJumpNotTruthy, &vec![9527], &if_node.span);
                self.compile_block_statement(&if_node.consequent)?;
                if self.last_instruction_is(OpPop) {
                    self.remove_last_pop();
                }

                let jump_pos = self.emit_with_span(OpJump, &vec![9527], &if_node.span);

                let after_consequence_location = self.current_instruction().data.len();
                self.change_operand(jump_not_truthy, after_consequence_location);

                if if_node.alternate.is_none() {
                    self.emit_with_span(OpNull, &vec![], &if_node.span);
                } else {
                    self.compile_block_statement(&if_node.clone().alternate.unwrap())?;
                    if self.last_instruction_is(OpPop) {
                        self.remove_last_pop();
                    }
                }
                let after_alternative_location = self.current_instruction().data.len();
                self.change_operand(jump_pos, after_alternative_location);
            }
            Expression::Index(index) => {
                self.compile_expr(&index.object)?;
                self.compile_expr(&index.index)?;
                self.emit_with_span(OpIndex, &vec![], &index.span);
            }
            Expression::FUNCTION(f) => {
                let function_span = f.span.clone();
                self.enter_scope();
                self.callable_kinds.push(CallableKind::Function);
                for param in f.params.iter() {
                    self.symbol_table.define(param.name.clone());
                }
                self.compile_block_statement(&f.body)?;
                if self.last_instruction_is(OpPop) {
                    self.replace_last_pop_with_return();
                }
                if !(self.last_instruction_is(OpReturnValue)) {
                    self.emit_with_span(OpReturn, &vec![], &function_span);
                }
                let num_locals = self.symbol_table.num_definitions;
                let free_symbols = self.symbol_table.free_symbols.clone();
                let scoped_instructions = self.leave_scope();
                self.callable_kinds.pop();
                for x in free_symbols.clone() {
                    self.load_symbol(&x, &function_span);
                }

                let compiled_function = Rc::from(object::CompiledFunction {
                    name: f.name.clone(),
                    instructions: scoped_instructions.instructions.data,
                    num_locals,
                    num_parameters: f.params.len(),
                });

                let constant_index = self.add_constant(Object::CompiledFunction(compiled_function));
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
                self.emit_with_span(OpCall, &vec![fc.arguments.len()], &fc.span);
            }
            Expression::This(this) => {
                let symbol = self
                    .symbol_table
                    .resolve("this".to_string())
                    .ok_or_else(|| "this is only available inside a method".to_string())?;
                self.load_symbol(&symbol, &this.span);
            }
            Expression::Property(property) => {
                self.compile_expr(&property.object)?;
                let name = self.add_constant(Object::String(property.property.name.clone()));
                self.emit_with_span(OpGetProperty, &vec![name], &property.span);
            }
            Expression::New(new_expression) => {
                let symbol = self
                    .symbol_table
                    .resolve(new_expression.callee.name.clone())
                    .ok_or_else(|| {
                        format!("Undefined variable '{}'", new_expression.callee.name)
                    })?;
                self.load_symbol(&symbol, &new_expression.callee.span);
                for argument in &new_expression.arguments {
                    self.compile_expr(argument)?;
                }
                self.emit_with_span(
                    OpNew,
                    &vec![new_expression.arguments.len()],
                    &new_expression.span,
                );
            }
        }

        return Ok(());
    }

    fn load_symbol(&mut self, symbol: &Rc<Symbol>, span: &Span) {
        match symbol.scope {
            SymbolScope::Global => {
                self.emit_with_span(OpGetGlobal, &vec![symbol.index], span);
            }
            SymbolScope::LOCAL => {
                self.emit_with_span(OpGetLocal, &vec![symbol.index], span);
            }
            SymbolScope::Builtin => {
                self.emit_with_span(OpGetBuiltin, &vec![symbol.index], span);
            }
            SymbolScope::Free => {
                self.emit_with_span(OpGetFree, &vec![symbol.index], span);
            }
            SymbolScope::Function => {
                self.emit_with_span(OpCurrentClosure, &vec![], span);
            }
        }
    }

    pub fn bytecode(&self) -> Bytecode {
        return Bytecode {
            instructions: self.current_instruction().clone(),
            constants: self.constants.clone(),
            debug_info: self.current_debug_info().clone(),
            function_debug_info: self.function_debug_info.clone(),
        };
    }

    pub fn add_constant(&mut self, obj: Object) -> usize {
        self.constants.push(Rc::new(obj));
        return self.constants.len() - 1;
    }

    pub fn emit(&mut self, op: Opcode, operands: &Vec<usize>) -> usize {
        let ins = make_instructions(op, operands);
        let pos = self.add_instructions(&ins);
        self.set_last_instruction(op, pos);

        return pos;
    }

    pub fn emit_with_span(&mut self, op: Opcode, operands: &Vec<usize>, span: &Span) -> usize {
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

        self.symbol_table.define("this".to_string());
        for parameter in &method.params {
            self.symbol_table.define(parameter.name.clone());
        }
        self.compile_block_statement(&method.body)?;

        match method.kind {
            MethodKind::Constructor => {
                self.emit_with_span(OpGetLocal, &vec![0], &method_span);
                self.emit_with_span(OpReturnValue, &vec![], &method_span);
            }
            MethodKind::Method => {
                if self.last_instruction_is(OpPop) {
                    self.replace_last_pop_with_return();
                }
                if !self.last_instruction_is(OpReturnValue) {
                    self.emit_with_span(OpReturn, &vec![], &method_span);
                }
            }
        }

        let num_locals = self.symbol_table.num_definitions;
        let free_symbols = self.symbol_table.free_symbols.clone();
        let scoped_instructions = self.leave_scope();
        self.callable_kinds.pop();
        for symbol in &free_symbols {
            self.load_symbol(symbol, &method_span);
        }

        let compiled_function = Rc::new(object::CompiledFunction {
            name: format!("{}.{}", class_name, method.name.name),
            instructions: scoped_instructions.instructions.data,
            num_locals,
            num_parameters: method.params.len() + 1,
        });
        let constant_index = self.add_constant(Object::CompiledFunction(compiled_function));
        self.function_debug_info_mut()
            .insert(constant_index, scoped_instructions.debug_info);
        self.emit_with_span(OpClosure, &vec![constant_index, free_symbols.len()], &method_span);
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
        if self.current_instruction().data.len() == 0 {
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
        self.replace_instruction(last_pos, &make_instructions(OpReturnValue, &vec![]));
        self.scopes[self.scope_index].last_instruction.opcode = OpReturnValue;
    }

    fn change_operand(&mut self, pos: usize, operand: usize) {
        let op = Opcode::from_repr(self.current_instruction().data[pos])
            .expect("compiler emitted an unknown opcode");
        let ins = make_instructions(op, &vec![operand]);
        self.replace_instruction(pos, &ins);
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
        let debug_info = self.current_debug_info().clone();
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
