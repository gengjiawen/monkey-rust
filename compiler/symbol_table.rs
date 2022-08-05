use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolScope {
    Global,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub scope: SymbolScope,
    pub index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolTable {
    symbols: HashMap<String, Rc<Symbol>>,
    num_definitions: usize,
}

impl SymbolTable {
    pub fn new() -> SymbolTable {
        SymbolTable { symbols: HashMap::new(), num_definitions: 0 }
    }

    pub fn define(&mut self, name: String) -> Rc<Symbol> {
        let symbol =
            Rc::new(Symbol { name: name.clone(), scope: SymbolScope::Global, index: self.num_definitions });
        self.num_definitions += 1;
        self.symbols.insert(name.clone(), Rc::clone(&symbol));
        return symbol;
    }

    pub fn resolve(&self, name: String) -> Option<Rc<Symbol>> {
        return self.symbols.get(&name).cloned();
    }
}
