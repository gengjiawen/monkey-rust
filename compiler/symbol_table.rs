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

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    pub fn new() -> SymbolTable {
        SymbolTable {
            symbols: HashMap::new(),
            free_symbols: vec![],
            num_definitions: 0,
            outer: None,
        }
    }

    pub fn new_enclosed_symbol_table(outer: SymbolTable) -> SymbolTable {
        SymbolTable {
            symbols: HashMap::new(),
            free_symbols: vec![],
            num_definitions: 0,
            outer: Some(Rc::new(outer)),
        }
    }

    pub fn define(&mut self, name: String) -> Rc<Symbol> {
        let mut scope = SymbolScope::LOCAL;
        if self.outer.is_none() {
            scope = SymbolScope::Global;
        }

        let symbol = Rc::new(Symbol {
            name: name.clone(),
            index: self.num_definitions,
            scope,
        });

        self.num_definitions += 1;
        self.symbols.insert(name.clone(), Rc::clone(&symbol));
        return symbol;
    }

    pub fn visible_names(&self) -> Vec<String> {
        let mut names = self
            .outer
            .as_ref()
            .map(|outer| outer.visible_names())
            .unwrap_or_default();
        names.extend(self.symbols.keys().cloned());
        names
    }

    /// Names defined in the outermost (global) scope, paired with their slot
    /// index. Sorted by name so reports stay deterministic.
    pub fn global_symbols(&self) -> Vec<(String, usize)> {
        let mut table = self;
        while let Some(outer) = table.outer.as_deref() {
            table = outer;
        }
        let mut globals: Vec<(String, usize)> = table
            .symbols
            .values()
            .filter(|symbol| symbol.scope == SymbolScope::Global)
            .map(|symbol| (symbol.name.clone(), symbol.index))
            .collect();
        globals.sort();
        globals
    }

    // Resolve a name in the current scope, capturing free variables from outers when needed.
    pub fn resolve(&mut self, name: String) -> Option<Rc<Symbol>> {
        if let Some(sym) = self.symbols.get(&name) {
            return Some(sym.clone());
        }

        // Resolve through every intermediate function scope. Each scope must
        // create its own free symbol so closures capture from the immediately
        // enclosing frame rather than reading a grandparent's local slot.
        let outer = self.outer.take()?;
        let mut outer_table = outer.as_ref().clone();
        let original = outer_table.resolve(name);
        self.outer = Some(Rc::new(outer_table));
        let original = original?;
        match original.scope {
            SymbolScope::Global | SymbolScope::Builtin => Some(original),
            SymbolScope::LOCAL | SymbolScope::Free | SymbolScope::Function => {
                Some(self.define_free(original))
            }
        }
    }

    pub fn define_builtin(&mut self, index: usize, name: String) -> Rc<Symbol> {
        let symbol = Rc::new(Symbol {
            name: name.clone(),
            index,
            scope: SymbolScope::Builtin,
        });
        self.symbols.insert(name.clone(), Rc::clone(&symbol));
        return symbol;
    }

    pub fn define_function_name(&mut self, name: String) -> Rc<Symbol> {
        let symbol = Rc::new(Symbol {
            name: name.clone(),
            index: 0,
            scope: SymbolScope::Function,
        });
        self.symbols.insert(name.clone(), Rc::clone(&symbol));
        return symbol;
    }

    pub fn define_free(&mut self, original: Rc<Symbol>) -> Rc<Symbol> {
        self.free_symbols.push(Rc::clone(&original));
        let symbol = Rc::new(Symbol {
            name: original.name.clone(),
            index: self.free_symbols.len() - 1,
            scope: SymbolScope::Free,
        });
        self.symbols
            .insert(original.name.clone(), Rc::clone(&symbol));
        return symbol;
    }
}
