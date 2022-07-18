use interpreter::eval;
use object::environment::Env;
use parser::parse;
use std::cell::RefCell;
use std::io::stdin;
use std::rc::Rc;

fn main() {
    println!("Welcome to monkey interpreter by gengjiawen");
    let env: Env = Rc::new(RefCell::new(Default::default()));
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).unwrap();

        if input.trim_end().is_empty() {
            println!("bye");
            std::process::exit(0)
        }

        match parse(&input) {
            Ok(node) => match eval(node, &env) {
                Ok(evaluated) => println!("{}", evaluated),
                Err(e) => eprintln!("{}", e),
            },
            Err(e) => eprintln!("parse error: {}", e[0]),
        }
    }
}
