use std::io::stdin;
use lexer::token::Token;
use lexer::Lexer;
use parser::Parser;

pub fn main() {
    println!("Welcome to monkey parser by gengjiawen");
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input);

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0)
        }

        let lexer = Lexer::new(&input);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program().unwrap();
        println!("{}", program);
    }
}