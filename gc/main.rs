use compiler::compiler::Compiler;
use gc::GcVM;
use parser::parse;
use std::io::stdin;
use std::io::{self, Write};

fn main() {
    println!("Welcome to monkey gc by gengjiawen");
    // Seed the persistent state from Compiler::new() so builtins like `len`
    // stay resolvable; new_with_state replaces the symbol table wholesale.
    let mut bootstrap_compiler = Compiler::new();
    let bootstrap = bootstrap_compiler
        .compile(&parse("").expect("empty program should parse"))
        .expect("empty program should compile");
    let mut symbol_table = bootstrap_compiler.symbol_table;
    let mut constants = bootstrap_compiler.constants;
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

        // Compile against clones and commit only after a successful run, so a
        // failed line cannot leak a half-defined binding into the next one.
        let mut compiler = Compiler::new_with_state(symbol_table.clone(), constants.clone());
        match compiler.compile(&program) {
            Ok(bytecode) => {
                vm.set_global_names(compiler.symbol_table.global_symbols());
                vm.load_bytecode(bytecode);
                match vm.run_with_budget(usize::MAX) {
                    Ok(()) => {
                        println!("{}", vm.last_result_string());
                        symbol_table = compiler.symbol_table;
                        constants = compiler.constants;
                    }
                    Err(error) => eprintln!("{}", error.message),
                }
            }
            Err(error) => {
                eprintln!("{}", error);
            }
        }
    }
}
