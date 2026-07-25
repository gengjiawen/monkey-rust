use compiler::compiler::Compiler;
use compiler::vm::VM;

use compiler::symbol_table::SymbolTable;
use object::Object;
use std::io::stdin;
use std::io::{self, Write};
use std::rc::Rc;

use parser::parse;

struct Repl {
    symbol_table: SymbolTable,
    constants: Vec<Rc<Object>>,
    globals: Vec<Rc<Object>>,
}

impl Repl {
    fn new() -> Self {
        // Seed the persistent state from Compiler::new() so builtins like `len`
        // stay resolvable; new_with_state replaces the symbol table wholesale.
        let bootstrap = Compiler::new();
        let null = Rc::new(Object::Null);
        Self {
            symbol_table: bootstrap.symbol_table,
            constants: bootstrap.constants,
            globals: vec![null; compiler::vm::GLOBAL_SIZE],
        }
    }

    fn eval_line(&mut self, input: &str) -> Result<String, String> {
        let program = parse(input).map_err(|errors| errors[0].clone())?;

        let mut compiler =
            Compiler::new_with_state(self.symbol_table.clone(), self.constants.clone());
        let compiled = compiler.compile(&program).and_then(|bytecode| {
            let mut vm = VM::new_with_global_store(bytecode, std::mem::take(&mut self.globals));
            let run_result = vm.run_checked().map_err(|error| error.to_string());
            let output = vm
                .last_popped_stack_elm()
                .map_or_else(String::new, |o| o.to_string());
            // The VM owns the persistent store while it runs. Take it back even
            // when execution fails so the next REPL line starts from valid state.
            self.globals = vm.globals;
            run_result?;
            Ok(output)
        });

        if compiled.is_ok() {
            self.symbol_table = compiler.symbol_table;
            self.constants = compiler.constants;
        }
        return compiled;
    }
}

fn main() {
    println!("Welcome to monkey compiler by gengjiawen");
    let mut repl = Repl::new();
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0);
        }

        match repl.eval_line(&input) {
            Ok(output) => println!("{}", output),
            Err(e) => println!("{}", e),
        }
    }
}

#[cfg(test)]
mod repl_test;
