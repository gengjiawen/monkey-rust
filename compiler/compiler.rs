use std::rc::Rc;

use object::Object;
use parser::ast::{BlockStatement, Expression, Literal, Node, Statement};
use parser::lexer::token::{TokenKind};

use crate::op_code::Opcode::*;
use crate::op_code::{cast_u8_to_opcode, make_instructions, Instructions, Opcode};

pub struct Compiler {
    instructions: Instructions,
    constants: Vec<Rc<Object>>,
    last_instruction: EmittedInstruction,
    previous_instruction: EmittedInstruction,
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
        return Compiler {
            instructions: Instructions { data: vec![] },
            constants: vec![],
            last_instruction: EmittedInstruction { opcode: Opcode::OpPop, position: 0 },
            previous_instruction: EmittedInstruction { opcode: Opcode::OpPop, position: 0 },
        };
    }

    pub fn compile(&mut self, node: &Node) -> Result<Bytecode, CompileError> {
        match node {
            Node::Program(p) => {
                for stmt in &p.body {
                    self.compile_stmt(stmt);
                }
            }
            Node::Statement(s) => {
                self.compile_stmt(s);
            }
            Node::Expression(e) => {
                self.compile_expr(e);
            }
        }

        return Ok(self.bytecode());
    }

    fn compile_stmt(&mut self, s: &Statement) {
        match s {
            Statement::Let(_) => {}
            Statement::Return(_) => {}
            Statement::Expr(e) => {
                self.compile_expr(e);
                self.emit(OpPop, &vec![]);
            }
        }
    }

    fn compile_expr(&mut self, e: &Expression) -> Result<(), CompileError> {
        match e {
            Expression::IDENTIFIER(_) => {}
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
                Literal::String(_) => {}
                Literal::Array(_) => {}
                Literal::Hash(_) => {}
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
                self.compile_expr(&if_node.condition);
                let jump_not_truthy = self.emit(OpJumpNotTruthy, &vec![9527]);
                self.compile_block_statement(&if_node.consequent);
                if self.last_instruction_is(OpPop) {
                    self.remove_last_pop();
                }

                let jump_pos = self.emit(OpJump, &vec![9527]);

                let after_consequence_location = self.instructions.data.len();
                self.change_operand(jump_not_truthy, after_consequence_location);

                if if_node.alternate.is_none() {
                } else {
                    self.compile_block_statement(&if_node.clone().alternate.unwrap());
                    if self.last_instruction_is(OpPop) {
                        self.remove_last_pop();
                    }
                }
                let after_alternative_location = self.instructions.data.len();
                self.change_operand(jump_pos, after_alternative_location);
            }
            Expression::FUNCTION(_) => {}
            Expression::FunctionCall(_) => {}
            Expression::Index(_) => {}
        }

        return Ok(());
    }

    pub fn bytecode(&self) -> Bytecode {
        return Bytecode {
            instructions: self.instructions.clone(),
            constants: self.constants.clone(),
        };
    }

    pub fn add_constant(&mut self, obj: Object) -> usize {
        self.constants.push(Rc::new(obj));
        return self.constants.len() - 1;
    }

    pub fn add_instructions(&mut self, ins: &Instructions) -> usize {
        let pos = self.instructions.data.len();
        self.instructions = self.instructions.merge_instructions(ins);
        return pos;
    }

    pub fn emit(&mut self, op: Opcode, operands: &Vec<usize>) -> usize {
        let ins = make_instructions(op, operands);
        let pos = self.add_instructions(&ins);
        self.set_last_instruction(op, pos);

        return pos;
    }

    fn compile_block_statement(&mut self, block_statement: &BlockStatement) {
        for stmt in &block_statement.body {
            self.compile_stmt(stmt);
        }
    }

    fn last_instruction_is(&self, op: Opcode) -> bool {
        return self.last_instruction.opcode == op;
    }

    fn remove_last_pop(&mut self) {
        self.instructions.data =
            self.instructions.data[..self.instructions.data.len() - 1].to_vec();
        self.last_instruction = self.previous_instruction.clone();
    }

    fn set_last_instruction(&mut self, op: Opcode, pos: usize) {
        self.previous_instruction = self.last_instruction.clone();
        self.last_instruction = EmittedInstruction { opcode: op, position: pos };
    }

    fn replace_instruction(&mut self, pos: usize, ins: &Instructions) {
        for i in 0..ins.data.len() {
            self.instructions.data[pos + i] = ins.data[i];
        }
    }

    fn change_operand(&mut self, pos: usize, operand: usize) {
        let op = cast_u8_to_opcode(self.instructions.data[pos]);
        let ins = make_instructions(op, &vec![operand]);
        self.replace_instruction(pos, &ins);
    }
}
