use crate::object::{Object, BuiltinFunc};
use std::rc::Rc;
use phf::phf_map;

pub static BUILTINS: phf::Map<&'static str, BuiltinFunc> = phf_map! {
    "len" => len,
    "puts" => puts,
    "print" => puts,
    "first" => first,
    "last" => last,
    "rest" => rest,
    "push" => push,
};


// a failed try
// rust sucks: https://stackoverflow.com/a/27896014/1713757
// pub static BUILTINS: HashMap<String, BuiltinFunc> = vec![(String::from("len"), len as BuiltinFunc) ]
//     .into_iter()
//     .collect();

pub fn len(args: Vec<Rc<Object>>) -> Rc<Object> {
    Rc::from(match &*args[0] {
        Object::String(s) => Object::Integer(s.len() as i64),
        Object::Array(a) => Object::Integer(a.len() as i64),
        o => Object::Error(format!("builtin len not supported for for type {}", o))
    })
}

pub fn puts(args: Vec<Rc<Object>>) -> Rc<Object> {
    args.iter().for_each(|obj| println!("{}", obj));
    Rc::from(Object::Null)
}

pub fn first(args: Vec<Rc<Object>>) -> Rc<Object> {
    match &*args[0] {
        Object::Array(s) => {
            match s.first() {
                Some(obj) => Rc::clone(obj),
                None => Rc::new(Object::Null),
            }
        },
        o => Rc::new(Object::Error(format!("builtin first not supported for for type {}", o)))
    }
}

pub fn last(args: Vec<Rc<Object>>) -> Rc<Object> {
    match &*args[0] {
        Object::Array(s) => {
            match s.last() {
                Some(obj) => Rc::clone(obj),
                None => Rc::new(Object::Null),
            }
        },
        o => Rc::new(Object::Error(format!("builtin last not supported for for type {}", o)))
    }
}

pub fn rest(args: Vec<Rc<Object>>) -> Rc<Object> {
    match &*args[0] {
        Object::Array(s) => {
            let len = s.len();
            if len > 0 {
                let new_array = s[1..len].to_vec();
                return Rc::new(Object::Array(new_array));
            }
            return Rc::new(Object::Null)
        },
        o => Rc::new(Object::Error(format!("builtin rest not supported for for type {}", o)))
    }
}

pub fn push(args: Vec<Rc<Object>>) -> Rc<Object> {
    let array = args.first().unwrap();
    let obj = Rc::clone(args.last().unwrap());
    match &**array {
        Object::Array(s) => {
            let mut new_array = s.clone();
            new_array.push(obj);
            return Rc::new(Object::Array(new_array));
        },
        o => Rc::new(Object::Error(format!("builtin push not supported for for type {}", o)))
    }
}
