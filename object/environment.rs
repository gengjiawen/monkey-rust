use crate::Object;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub type Env = Rc<RefCell<Environment>>;

#[derive(Debug, Default, Eq, Clone, PartialEq)]
pub struct Environment {
    store: HashMap<String, Rc<Object>>,
    outer: Option<Env>,
}

impl Environment {
    pub fn new_enclosed_environment(outer: &Env) -> Self {
        let mut env: Environment = Default::default();
        env.outer = Some(Rc::clone(outer));
        return env;
    }

    pub fn get(&self, name: &str) -> Option<Rc<Object>> {
        match self.store.get(name) {
            Some(obj) => Some(Rc::clone(obj)),
            None => {
                if let Some(outer) = &self.outer {
                    return outer.borrow().get(name);
                } else {
                    return None;
                }
            }
        }
    }

    pub fn set(&mut self, name: String, val: Rc<Object>) {
        self.store.insert(name, val);
    }
}
