use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SymbolScope {
    LOCAL,
    Global,
    Builtin,
    Free,
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub scope: SymbolScope,
    pub index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymbolTable {
    pub outer: Option<Rc<SymbolTable>>,
    symbols: HashMap<String, Rc<Symbol>>,
    pub free_symbols: Vec<Rc<Symbol>>,
    pub num_definitions: usize,
}

impl SymbolTable {
    pub fn new() -> SymbolTable {
        SymbolTable { symbols: HashMap::new(), free_symbols: vec![], num_definitions: 0, outer: None }
    }

    pub fn new_enclosed_symbol_table(outer: SymbolTable) -> SymbolTable {
        SymbolTable { symbols: HashMap::new(), free_symbols: vec![], num_definitions: 0, outer: Some(Rc::new(outer)) }
    }

    pub fn define(&mut self, name: String) -> Rc<Symbol> {
        let mut scope = SymbolScope::LOCAL;
        if self.outer.is_none() {
            scope = SymbolScope::Global;
        }

        let symbol = Rc::new(Symbol { name: name.clone(), index: self.num_definitions, scope });

        self.num_definitions += 1;
        self.symbols.insert(name.clone(), Rc::clone(&symbol));
        return symbol;
    }

    pub fn resolve(&self, name: String) -> Option<Rc<Symbol>> {
        let symbol = self.symbols.get(&name);
        if symbol.is_none() && self.outer.is_some() {
            return self.outer.as_ref().unwrap().resolve(name);
        }
        return symbol.cloned();
    }

    pub fn define_builtin(&mut self, index: usize, name: String) -> Rc<Symbol> {
        let symbol = Rc::new(Symbol { name: name.clone(), index, scope: SymbolScope::Builtin });
        self.symbols.insert(name.clone(), Rc::clone(&symbol));
        return symbol;
    }

    pub fn define_function_name(&mut self, name: String) -> Rc<Symbol> {
        let symbol = Rc::new(Symbol { name: name.clone(), index: 0, scope: SymbolScope::Function });
        self.symbols.insert(name.clone(), Rc::clone(&symbol));
        return symbol;
    }

    pub fn define_free(&mut self, original: Rc<Symbol>) -> Rc<Symbol> {
        self.free_symbols.push(Rc::clone(&original));
        let symbol = Rc::new(Symbol { name: original.name.clone(), index: self.free_symbols.len() - 1, scope: SymbolScope::Free });
        self.symbols.insert(original.name.clone(), Rc::clone(&symbol));
        return symbol;
    }
}
