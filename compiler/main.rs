use compiler::compiler::Compiler;
use compiler::vm::VM;

use compiler::symbol_table::SymbolTable;
use object::Object;
use std::io::stdin;
use std::io::{self, Write};
use std::rc::Rc;

use parser::parse;

fn main() {
    println!("Welcome to monkey compiler by gengjiawen");
    let mut constants = vec![];
    let mut symbol_table = SymbolTable::new();
    let mut globals = vec![Rc::new(Object::Null); compiler::vm::GLOBAL_SIZE];
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

        let mut compiler = Compiler::new_with_state(symbol_table, constants);

        match compiler.compile(&program) {
            Ok(bytecodes) => {
                let mut vm = VM::new_with_global_store(bytecodes, globals);
                vm.run();
                println!("{}", vm.last_popped_stack_elm().unwrap());
                globals = vm.globals;
            },
            Err(e) => {
                println!("{}", e);
            }
        };

        symbol_table = compiler.symbol_table;
        constants = compiler.constants;

    }
}
