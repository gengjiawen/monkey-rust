use compiler::compiler::Compiler;
use compiler::vm::VM;

use std::io::stdin;
use std::io::{self, Write};
use std::rc::Rc;
use object::Object;

use parser::parse;

fn main() {
    println!("Welcome to monkey compiler by gengjiawen");
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0);
        }

        let program = match parse(&input) {
            Ok(x) => x,
            Err(e) => {
                println!("{}", e[0]);
                continue;
            }
        };

        let mut compiler = Compiler::new();

        let bytecodes = match compiler.compile(&program) {
            Ok(x) => x,
            Err(e) => {
                println!("{}", e);
                continue;
            },
        };
        let mut vm = VM::new(bytecodes);
        vm.run();
        match vm.last_popped_stack_elm() {
            None => {
            }
            Some(got) => {
                println!("{}", got);
            }
        };
    }
}
