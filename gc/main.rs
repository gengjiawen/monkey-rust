use compiler::compiler::Compiler;
use compiler::symbol_table::SymbolTable;
use gc::GcVM;
use object::Object;
use parser::parse;
use std::io::stdin;
use std::io::{self, Write};
use std::rc::Rc;

fn main() {
    println!("Welcome to monkey gc by gengjiawen");
    let mut constants: Vec<Rc<Object>> = vec![];
    let mut symbol_table = SymbolTable::new();
    let bootstrap = {
        let mut compiler = Compiler::new();
        compiler
            .compile(&parse("").expect("empty program should parse"))
            .expect("empty program should compile")
    };
    let mut vm = GcVM::new(bootstrap);

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
            Ok(node) => node,
            Err(errors) => {
                eprintln!("parse error: {}", errors[0]);
                continue;
            }
        };

        let mut compiler = Compiler::new_with_state(symbol_table, constants);
        match compiler.compile(&program) {
            Ok(bytecode) => {
                vm.set_global_names(compiler.symbol_table.global_symbols());
                vm.load_bytecode(bytecode);
                match vm.run_with_budget(usize::MAX) {
                    Ok(()) => println!("{}", vm.last_result_string()),
                    Err(error) => eprintln!("{}", error.message),
                }
            }
            Err(error) => {
                eprintln!("{}", error);
            }
        }

        symbol_table = compiler.symbol_table;
        constants = compiler.constants;
    }
}
