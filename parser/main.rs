use std::io::stdin;
use lexer::token::Token;

pub fn main() {
    println!("Welcome to monkey parser");
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input);

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0)
        }
    }
}