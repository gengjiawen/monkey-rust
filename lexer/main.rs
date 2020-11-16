use std::io::stdin;
use lexer::Lexer;
use lexer::token::Token;

pub fn main() {
    println!("Welcome to monkey lexer");
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input);

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0)
        }

        let mut l = Lexer::new(&input);
        loop {
            let t = l.next_token();
            if t == Token::EOF {
                break
            } else {
                println!("{}", t)
            }
        }
    }
}