use compiler::compiler::Compiler;
use compiler::vm::VM;

use std::io::stdin;

use parser::parse;

fn main() {
    println!("Welcome to monkey compiler by gengjiawen");
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0);
        }

        let program = parse(&input).unwrap();

        let mut compiler = Compiler::new();

        let bytecodes = compiler.compile(&program).unwrap();
        println!(
            "ins {} for input {}",
            bytecodes.instructions.string(),
            input
        );
        let mut vm = VM::new(bytecodes);
        vm.run();
        let got = vm.last_popped_stack_elm().unwrap();
        println!("{}", got);
    }
}
