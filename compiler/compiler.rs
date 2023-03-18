use object::builtins::BuiltIns;
use std::rc::Rc;

use object::Object;
use parser::ast::{BlockStatement, Expression, Literal, Node, Statement};
use parser::lexer::token::TokenKind;

use crate::op_code::Opcode::*;
use crate::op_code::{cast_u8_to_opcode, make_instructions, Instructions, Opcode};
use crate::symbol_table::{Symbol, SymbolScope, SymbolTable};

struct CompilationScope {
    instructions: Instructions,
    last_instruction: EmittedInstruction,
    previous_instruction: EmittedInstruction,
}

pub struct Compiler {
    pub constants: Vec<Rc<Object>>,
    pub symbol_table: SymbolTable,
    scopes: Vec<CompilationScope>,
    scope_index: usize,
}

pub struct Bytecode {
    pub instructions: Instructions,
    pub constants: Vec<Rc<Object>>,
}

#[derive(Clone)]
pub struct EmittedInstruction {
    pub opcode: Opcode,
    pub position: usize,
}

type CompileError = String;

impl Compiler {
    pub fn new() -> Compiler {
        let main_scope = CompilationScope {
            instructions: Instructions { data: vec![] },
            last_instruction: EmittedInstruction { opcode: OpNull, position: 0 },
            previous_instruction: EmittedInstruction { opcode: OpNull, position: 0 },
        };

        let mut symbol_table = SymbolTable::new();
        for (key, value) in BuiltIns.iter().enumerate() {
            symbol_table.define_builtin(key, value.0.to_string());
        }

        return Compiler {
            constants: vec![],
            symbol_table,
            scopes: vec![main_scope],
            scope_index: 0,
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
                self.compile_expr(&let_statement.expr)?;
                let symbol = self
                    .symbol_table
                    .define(let_statement.identifier.kind.to_string());
                if symbol.scope == SymbolScope::Global {
                    self.emit(Opcode::OpSetGlobal, &vec![symbol.index]);
                } else {
                    self.emit(Opcode::OpSetLocal, &vec![symbol.index]);
                }
                return Ok(());
            }
            Statement::Return(r) => {
                self.compile_expr(&r.argument)?;
                self.emit(Opcode::OpReturnValue, &vec![]);
                return Ok(());
            }
            Statement::Expr(e) => {
                self.compile_expr(e)?;
                self.emit(OpPop, &vec![]);
                return Ok(());
            }
        }
    }

    fn compile_expr(&mut self, e: &Expression) -> Result<(), CompileError> {
        match e {
            Expression::IDENTIFIER(identifier) => {
                let symbol = self.symbol_table.resolve(identifier.name.clone());
                match symbol {
                    Some(symbol) => {
                        self.load_symbol(&symbol);
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
                    self.emit(OpConst, &operands);
                }
                Literal::Boolean(i) => {
                    if i.raw {
                        self.emit(OpTrue, &vec![]);
                    } else {
                        self.emit(OpFalse, &vec![]);
                    }
                }
                Literal::String(s) => {
                    let string_object = Object::String(s.raw.clone());
                    let operands = vec![self.add_constant(string_object)];
                    self.emit(OpConst, &operands);
                }
                Literal::Array(array) => {
                    for element in array.elements.iter() {
                        self.compile_expr(element)?;
                    }
                    self.emit(OpArray, &vec![array.elements.len()]);
                }
                Literal::Hash(hash) => {
                    for (key, value) in hash.elements.iter() {
                        self.compile_expr(&key)?;
                        self.compile_expr(&value)?;
                    }
                    self.emit(OpHash, &vec![hash.elements.len() * 2]);
                }
            },
            Expression::PREFIX(prefix) => {
                self.compile_expr(&prefix.operand).unwrap();
                match prefix.op.kind {
                    TokenKind::MINUS => {
                        self.emit(OpMinus, &vec![]);
                    }
                    TokenKind::BANG => {
                        self.emit(OpBang, &vec![]);
                    }
                    _ => {
                        return Err(format!("unexpected prefix op: {}", prefix.op));
                    }
                }
            }
            Expression::INFIX(infix) => {
                if infix.op.kind == TokenKind::LT {
                    self.compile_expr(&infix.right).unwrap();
                    self.compile_expr(&infix.left).unwrap();
                    self.emit(Opcode::OpGreaterThan, &vec![]);
                    return Ok(());
                }
                self.compile_expr(&infix.left).unwrap();
                self.compile_expr(&infix.right).unwrap();
                match infix.op.kind {
                    TokenKind::PLUS => {
                        self.emit(OpAdd, &vec![]);
                    }
                    TokenKind::MINUS => {
                        self.emit(OpSub, &vec![]);
                    }
                    TokenKind::ASTERISK => {
                        self.emit(OpMul, &vec![]);
                    }
                    TokenKind::SLASH => {
                        self.emit(OpDiv, &vec![]);
                    }
                    TokenKind::GT => {
                        self.emit(Opcode::OpGreaterThan, &vec![]);
                    }
                    TokenKind::EQ => {
                        self.emit(Opcode::OpEqual, &vec![]);
                    }
                    TokenKind::NotEq => {
                        self.emit(Opcode::OpNotEqual, &vec![]);
                    }
                    _ => {
                        return Err(format!("unexpected infix op: {}", infix.op));
                    }
                }
            }
            Expression::IF(if_node) => {
                self.compile_expr(&if_node.condition)?;
                let jump_not_truthy = self.emit(OpJumpNotTruthy, &vec![9527]);
                self.compile_block_statement(&if_node.consequent)?;
                if self.last_instruction_is(OpPop) {
                    self.remove_last_pop();
                }

                let jump_pos = self.emit(OpJump, &vec![9527]);

                let after_consequence_location = self.current_instruction().data.len();
                self.change_operand(jump_not_truthy, after_consequence_location);

                if if_node.alternate.is_none() {
                    self.emit(OpNull, &vec![]);
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
                self.emit(OpIndex, &vec![]);
            }
            Expression::FUNCTION(f) => {
                self.enter_scope();
                for param in f.params.iter() {
                    self.symbol_table.define(param.name.clone());
                }
                self.compile_block_statement(&f.body)?;
                if self.last_instruction_is(OpPop) {
                    self.replace_last_pop_with_return();
                }
                if !(self.last_instruction_is(OpReturnValue)) {
                    self.emit(OpReturn, &vec![]);
                }
                let num_locals = self.symbol_table.num_definitions;
                let instructions = self.leave_scope();

                let compiled_function = object::CompiledFunction {
                    instructions: instructions.data,
                    num_locals,
                    num_parameters: f.params.len(),
                };

                let operands = vec![self.add_constant(Object::CompiledFunction(compiled_function))];
                self.emit(OpConst, &operands);
            }
            Expression::FunctionCall(fc) => {
                self.compile_expr(&fc.callee)?;
                for arg in fc.arguments.iter() {
                    self.compile_expr(arg)?;
                }
                self.emit(OpCall, &vec![fc.arguments.len()]);
            }
        }

        return Ok(());
    }

    fn load_symbol(&mut self, symbol: &Rc<Symbol>) {
        match symbol.scope {
            SymbolScope::Global => {
                self.emit(OpGetGlobal, &vec![symbol.index]);
            }
            SymbolScope::LOCAL => {
                self.emit(OpGetLocal, &vec![symbol.index]);
            }
            SymbolScope::Builtin => {
                self.emit(OpGetBuiltin, &vec![symbol.index]);
            }
        }
    }

    pub fn bytecode(&self) -> Bytecode {
        return Bytecode {
            instructions: self.current_instruction().clone(),
            constants: self.constants.clone(),
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

    fn compile_block_statement(
        &mut self,
        block_statement: &BlockStatement,
    ) -> Result<(), CompileError> {
        for stmt in &block_statement.body {
            self.compile_stmt(stmt)?;
        }
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
        let last_instruction = EmittedInstruction { opcode: op, position: pos };
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
        let op = cast_u8_to_opcode(self.current_instruction().data[pos]);
        let ins = make_instructions(op, &vec![operand]);
        self.replace_instruction(pos, &ins);
    }

    fn current_instruction(&self) -> &Instructions {
        return &self.scopes[self.scope_index].instructions;
    }

    fn enter_scope(&mut self) {
        let scope = CompilationScope {
            instructions: Instructions { data: vec![] },
            last_instruction: EmittedInstruction { opcode: OpNull, position: 0 },
            previous_instruction: EmittedInstruction { opcode: OpNull, position: 0 },
        };
        self.scopes.push(scope);
        self.scope_index += 1;
        self.symbol_table = SymbolTable::new_enclosed_symbol_table(self.symbol_table.clone());
    }

    fn leave_scope(&mut self) -> Instructions {
        let instructions = self.current_instruction().clone();
        self.scopes.pop();
        self.scope_index -= 1;
        let s = self.symbol_table.outer.as_ref().unwrap().as_ref().clone();
        self.symbol_table = s;
        return instructions;
    }
}
