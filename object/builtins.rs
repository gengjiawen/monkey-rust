use crate::{BuiltinFunc, Object};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BuiltinId {
    Len,
    Puts,
    First,
    Last,
    Rest,
    Push,
}

#[derive(Clone, Copy)]
pub struct BuiltinDefinition {
    pub name: &'static str,
    pub id: BuiltinId,
    pub function: BuiltinFunc,
}

lazy_static! {
    pub static ref BuiltIns: Vec<BuiltinDefinition> = vec![
        BuiltinDefinition {
            name: "len",
            id: BuiltinId::Len,
            function: len
        },
        BuiltinDefinition {
            name: "puts",
            id: BuiltinId::Puts,
            function: puts
        },
        BuiltinDefinition {
            name: "first",
            id: BuiltinId::First,
            function: first
        },
        BuiltinDefinition {
            name: "last",
            id: BuiltinId::Last,
            function: last
        },
        BuiltinDefinition {
            name: "rest",
            id: BuiltinId::Rest,
            function: rest
        },
        BuiltinDefinition {
            name: "push",
            id: BuiltinId::Push,
            function: push
        },
        BuiltinDefinition {
            name: "print",
            id: BuiltinId::Puts,
            function: puts
        },
    ];
}

// a failed try
// rust sucks: https://stackoverflow.com/a/27896014/1713757
// pub static BUILTINS: HashMap<String, BuiltinFunc> = vec![(String::from("len"), len as BuiltinFunc) ]
//     .into_iter()
//     .collect();

/// Reject a call whose argument count does not match the builtin's arity,
/// mirroring the checks the gc runtime performs in `gc/value.rs`. Returns the
/// `Object::Error` the builtin should hand back, or `None` when the call is
/// well-formed. Every fixed-arity builtin must call this before touching
/// `args[..]`, otherwise a short call indexes out of bounds and panics.
fn check_arity(name: &str, args: &[Rc<Object>], expected: usize) -> Option<Rc<Object>> {
    if args.len() == expected {
        return None;
    }
    Some(Rc::from(Object::Error(format!(
        "builtin {} expected {} argument{}, got {}",
        name,
        expected,
        if expected == 1 { "" } else { "s" },
        args.len()
    ))))
}

pub fn len(args: Vec<Rc<Object>>) -> Rc<Object> {
    if let Some(error) = check_arity("len", &args, 1) {
        return error;
    }
    Rc::from(match &*args[0] {
        Object::String(s) => Object::Integer(s.len() as i64),
        Object::Array(a) => Object::Integer(a.len() as i64),
        o => Object::Error(format!("builtin len not supported for for type {}", o)),
    })
}

pub fn puts(args: Vec<Rc<Object>>) -> Rc<Object> {
    args.iter().for_each(|obj| println!("{}", obj));
    Rc::from(Object::Null)
}

pub fn first(args: Vec<Rc<Object>>) -> Rc<Object> {
    if let Some(error) = check_arity("first", &args, 1) {
        return error;
    }
    match &*args[0] {
        Object::Array(s) => match s.first() {
            Some(obj) => Rc::clone(obj),
            None => Rc::new(Object::Null),
        },
        o => Rc::new(Object::Error(format!("builtin first not supported for for type {}", o))),
    }
}

pub fn last(args: Vec<Rc<Object>>) -> Rc<Object> {
    if let Some(error) = check_arity("last", &args, 1) {
        return error;
    }
    match &*args[0] {
        Object::Array(s) => match s.last() {
            Some(obj) => Rc::clone(obj),
            None => Rc::new(Object::Null),
        },
        o => Rc::new(Object::Error(format!("builtin last not supported for for type {}", o))),
    }
}

pub fn rest(args: Vec<Rc<Object>>) -> Rc<Object> {
    if let Some(error) = check_arity("rest", &args, 1) {
        return error;
    }
    match &*args[0] {
        Object::Array(s) => {
            let len = s.len();
            if len > 0 {
                let new_array = s[1..len].to_vec();
                return Rc::new(Object::Array(new_array));
            }
            return Rc::new(Object::Null);
        }
        o => Rc::new(Object::Error(format!("builtin rest not supported for for type {}", o))),
    }
}

pub fn push(args: Vec<Rc<Object>>) -> Rc<Object> {
    if let Some(error) = check_arity("push", &args, 2) {
        return error;
    }
    match &*args[0] {
        Object::Array(s) => {
            let mut new_array = s.clone();
            new_array.push(Rc::clone(&args[1]));
            return Rc::new(Object::Array(new_array));
        }
        o => Rc::new(Object::Error(format!("builtin push not supported for for type {}", o))),
    }
}
