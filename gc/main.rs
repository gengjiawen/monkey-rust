use compiler::compiler::Compiler;
use compiler::symbol_table::SymbolTable;
use gc::GcVM;
use object::Object;
use parser::parse;
use std::io::stdin;
use std::io::{self, Write};
use std::rc::Rc;

struct Repl {
    symbol_table: SymbolTable,
    constants: Vec<Rc<Object>>,
    vm: GcVM,
}

impl Repl {
    fn new() -> Self {
        // Seed the persistent state from Compiler::new() so builtins like `len`
        // stay resolvable; new_with_state replaces the symbol table wholesale.
        let mut bootstrap_compiler = Compiler::new();
        let bootstrap = bootstrap_compiler
            .compile(&parse("").expect("empty program should parse"))
            .expect("empty program should compile");
        Self {
            symbol_table: bootstrap_compiler.symbol_table,
            constants: bootstrap_compiler.constants,
            vm: GcVM::new(bootstrap),
        }
    }

    fn eval_line(&mut self, input: &str) -> Result<String, String> {
        let program = parse(input).map_err(|errors| format!("parse error: {}", errors[0]))?;

        // Compile against clones and commit only after a successful run, so a
        // failed line cannot leak a half-defined binding into the next one.
        let mut compiler = Compiler::new_with_state(self.symbol_table.clone(), self.constants.clone());
        let bytecode = compiler.compile(&program)?;
        self.vm
            .set_global_names(compiler.symbol_table.global_symbols());
        self.vm.load_bytecode(bytecode);
        self.vm
            .run_with_budget(usize::MAX)
            .map_err(|error| error.message)?;
        self.symbol_table = compiler.symbol_table;
        self.constants = compiler.constants;
        Ok(self.vm.last_result_string())
    }
}

fn main() {
    println!("Welcome to monkey gc by gengjiawen");
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
            Ok(result) => println!("{}", result),
            Err(error) => eprintln!("{}", error),
        }
    }
}

#[cfg(test)]
mod repl_test;
