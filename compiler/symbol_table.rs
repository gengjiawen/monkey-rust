use std::collections::HashMap;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SymbolScope {
    Global,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub scope: SymbolScope,
    pub index: usize,
}

pub struct SymbolTable {
    symbols: HashMap<String, Symbol>,
    num_definitions: usize,
}

impl SymbolTable {
    pub fn new() -> SymbolTable {
        SymbolTable { symbols: HashMap::new(), num_definitions: 0 }
    }

    pub fn define(&mut self, name: String) -> Symbol {
        let symbol =
            Symbol { name: name.clone(), scope: SymbolScope::Global, index: self.num_definitions };
        self.num_definitions += 1;
        self.symbols.insert(name.clone(), symbol.clone());
        return symbol;
    }

    pub fn resolve(&self, name: String) -> Option<Symbol> {
        return self.symbols.get(&name).cloned();
    }
}
