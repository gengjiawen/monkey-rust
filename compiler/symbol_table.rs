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

    // Resolve a name in the current scope, capturing free variables from outers when needed.
    pub fn resolve(&mut self, name: String) -> Option<Rc<Symbol>> {
        if let Some(sym) = self.symbols.get(&name) {
            return Some(sym.clone());
        }

        // If not found locally, try outer scopes.
        if let Some(outer) = &self.outer {
            // We can't mutate outer here, so use a read-only helper to locate the original symbol.
            if let Some(original) = outer.resolve_readonly(&name) {
                return match original.scope {
                    // Globals and builtins are accessed directly.
                    SymbolScope::Global | SymbolScope::Builtin => Some(original),
                    // Locals (from outer scope) or already-free symbols should be captured as a new free symbol here.
                    SymbolScope::LOCAL | SymbolScope::Free | SymbolScope::Function => {
                        Some(self.define_free(original))
                    }
                };
            }
        }

        None
    }

    // Read-only resolver used internally to search outer scopes without mutating them.
    fn resolve_readonly(&self, name: &str) -> Option<Rc<Symbol>> {
        if let Some(sym) = self.symbols.get(name) {
            return Some(sym.clone());
        }
        if let Some(outer) = &self.outer {
            return outer.resolve_readonly(name);
        }
        None
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
