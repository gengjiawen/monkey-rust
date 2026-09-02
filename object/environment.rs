use crate::Object;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub type Env = Rc<RefCell<Environment>>;

#[derive(Debug, Default, Eq, Clone, PartialEq)]
pub struct Environment {
    /// Copy-on-write: a closure `snapshot` shares this map with the live frame,
    /// and whichever side writes next (`set`) copies it first. Creating a
    /// closure therefore costs one `Rc` clone per frame, not a copy of every
    /// binding in scope.
    store: Rc<HashMap<String, Rc<Object>>>,
    outer: Option<Env>,
}

impl Environment {
    pub fn new_enclosed_environment(outer: &Env) -> Self {
        return Environment {
            outer: Some(Rc::clone(outer)),
            ..Default::default()
        };
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
        // `make_mut` only copies the map while a snapshot still shares it.
        Rc::make_mut(&mut self.store).insert(name, val);
    }

    /// Capture the bindings visible at a declaration site.
    ///
    /// Values remain shared, and so are the frames' maps until either side
    /// writes again; a later `let` with the same name therefore cannot rewrite
    /// what an existing closure sees, without copying every binding up front.
    pub fn snapshot(&self) -> Self {
        return Self {
            store: Rc::clone(&self.store),
            outer: self
                .outer
                .as_ref()
                .map(|outer| Rc::new(RefCell::new(outer.borrow().snapshot()))),
        };
    }

    pub fn visible_names(&self) -> Vec<String> {
        let mut names = self
            .outer
            .as_ref()
            .map(|outer| outer.borrow().visible_names())
            .unwrap_or_default();
        names.extend(self.store.keys().cloned());
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_shares_frames_until_either_side_writes() {
        let global = Rc::new(RefCell::new(Environment::default()));
        global
            .borrow_mut()
            .set("x".to_string(), Rc::new(Object::Integer(1)));
        let mut frame = Environment::new_enclosed_environment(&global);
        frame.set("y".to_string(), Rc::new(Object::Integer(2)));

        let snapshot = frame.snapshot();
        assert!(Rc::ptr_eq(&snapshot.store, &frame.store));
        let snapshot_outer = snapshot.outer.as_ref().unwrap();
        assert!(Rc::ptr_eq(&snapshot_outer.borrow().store, &global.borrow().store));

        // A later re-`let` on the live side copies that frame only; the
        // snapshot keeps reading the binding it captured.
        global
            .borrow_mut()
            .set("x".to_string(), Rc::new(Object::Integer(10)));
        frame.set("y".to_string(), Rc::new(Object::Integer(20)));
        assert_eq!(snapshot.get("x"), Some(Rc::new(Object::Integer(1))));
        assert_eq!(snapshot.get("y"), Some(Rc::new(Object::Integer(2))));
        assert_eq!(frame.get("x"), Some(Rc::new(Object::Integer(10))));
        assert_eq!(frame.get("y"), Some(Rc::new(Object::Integer(20))));
        assert!(!Rc::ptr_eq(&snapshot.store, &frame.store));
    }
}
