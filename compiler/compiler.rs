use std::error::Error;
use std::rc::Rc;

use object::Object;
use parser::ast::{Expression, Literal, Node, Statement};
use parser::lexer::token::{Token, TokenKind};

use crate::op_code::{Instructions, make_instructions, Opcode};
use crate::op_code::Opcode::{*};

pub struct Compiler {
    instructions: Instructions,
    constants: Vec<Rc<Object>>,
}

pub struct Bytecode {
    pub instructions: Instructions,
    pub constants: Vec<Rc<Object>>,
}

type CompileError = String;

impl Compiler {
    pub fn new() -> Compiler {
        return Compiler {
            instructions: Instructions { data: vec![] },
            constants: vec![],
        }
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

        return Ok(self.bytecode())
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
            Expression::LITERAL(l) => {
                match l {
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
                }
            }
            Expression::PREFIX(_) => {}
            Expression::INFIX(infix) => {
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
                    _ => {
                        return Err(format!("unexpected infix op: {}", infix.op));
                    }
                }
            }
            Expression::IF(_) => {}
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
        }
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
        let ins = Instructions {
            data: make_instructions(op, operands)
        };

        return self.add_instructions(&ins);
    }
}

