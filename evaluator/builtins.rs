use crate::object::{Object, BuiltinFunc};
use std::rc::Rc;
use phf::phf_map;

pub static BUILTINS: phf::Map<&'static str, BuiltinFunc> = phf_map! {
    "len" => len,
};


// a failed try
// rust sucks: https://stackoverflow.com/a/27896014/1713757
// pub static BUILTINS: HashMap<String, BuiltinFunc> = vec![(String::from("len"), len as BuiltinFunc) ]
//     .into_iter()
//     .collect();

pub fn len(args: Vec<Rc<Object>>) -> Object {
    match &*args[0] {
        Object::String(s) => Object::Integer(s.len() as i64),
        o => Object::Error(format!("builtin len not supported for for type {}", o))
    }
}
