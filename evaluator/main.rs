use parser::parse;
use std::io::stdin;
use evaluator::eval;

fn main() {
    println!("Welcome to monkey evaluator by gengjiawen");
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input);

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0)
        }

        match parse(&input) {
            Ok(node) => {
                match eval(node) {
                    Ok(evaluated) =>  println!("{}", evaluated),
                    Err(e) => eprintln!("{}", e),
                }
            },
            Err(e) => eprintln!("parse error: {}", e[0])
        }
    }
}
