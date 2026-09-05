use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use parser::ast::{BlockStatement, Expression, Literal, Statement, IF};

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
    /// Every slot-allocating definition of this scope in definition order, so
    /// `definitions[i].index == i`. A rebinding appends a fresh entry instead
    /// of replacing the shadowed slot's entry, unlike `symbols`.
    pub definitions: Vec<Rc<Symbol>>,
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
            definitions: vec![],
            num_definitions: 0,
            outer: None,
        }
    }

    pub fn new_enclosed_symbol_table(outer: SymbolTable) -> SymbolTable {
        SymbolTable {
            symbols: HashMap::new(),
            free_symbols: vec![],
            definitions: vec![],
            num_definitions: 0,
            outer: Some(Rc::new(outer)),
        }
    }

    /// Every `let` takes a slot of its own, so a closure created between two
    /// bindings of one name keeps reading the value it captured.
    pub fn define(&mut self, name: String) -> Rc<Symbol> {
        let symbol = self.allocate(&name);
        self.symbols.insert(name, Rc::clone(&symbol));
        return symbol;
    }

    /// Takes a slot for `name` without making it the name's binding yet.
    ///
    /// Blocks are not scopes in Monkey — `if (true) { let inner = 1; } inner;`
    /// evaluates to `1` — but a block is still something a jump can skip, so a
    /// slot a block binds is one nothing may have written by the time the code
    /// after the block reads it. The binding for after the branch therefore
    /// has to be a slot picked *before* it, seeded with the value in force
    /// there and overwritten by whichever arm runs (see
    /// [`shadowed_names`]). Until the `let` that rebinds it, the name still
    /// means what it meant before, so this deliberately leaves `symbols`
    /// alone; [`SymbolTable::rebind`] performs the switch.
    pub fn declare_slot(&mut self, name: &str) -> Rc<Symbol> {
        return self.allocate(name);
    }

    /// Points `name` at `symbol`, which must be one this scope handed out.
    pub fn rebind(&mut self, name: &str, symbol: Rc<Symbol>) {
        self.symbols.insert(name.to_string(), symbol);
    }

    fn allocate(&mut self, name: &str) -> Rc<Symbol> {
        let scope = if self.outer.is_none() { SymbolScope::Global } else { SymbolScope::LOCAL };

        let symbol = Rc::new(Symbol {
            name: name.to_string(),
            index: self.num_definitions,
            scope,
        });

        self.num_definitions += 1;
        self.definitions.push(Rc::clone(&symbol));
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

    /// The outermost scope's definition ledger, in slot order. Unlike a view
    /// derived from `symbols`, a rebound name appears once per slot it ever
    /// occupied, which is what debug metadata needs.
    pub fn global_definitions(&self) -> &[Rc<Symbol>] {
        let mut table = self;
        while let Some(outer) = table.outer.as_deref() {
            table = outer;
        }
        &table.definitions
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

/// The names `if_node` can rebind for the code that follows it, in the order
/// they appear.
///
/// A backend gives each of these a slot before the branch and copies the arm's
/// own final binding into it at the end of every arm, so the name means the
/// same thing after the `if` whichever way the jump went — and means what it
/// meant before when neither arm runs. The list over-approximates on purpose:
/// a name that turns out not to be rebound just costs a slot and a copy of
/// itself onto itself, whereas missing one leaves a read after the branch on a
/// slot that may never have been written.
///
/// Nested `if`s are included because their own rebinding happens inside this
/// one, and so is conditional on it too. Function and method bodies are not:
/// they open a scope of their own, and a `let` in there cannot rebind a name
/// out here.
pub fn shadowed_names(if_node: &IF) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut collect = |block: &BlockStatement| {
        let mut found = Vec::new();
        collect_block(block, &mut found);
        for name in found {
            if seen.insert(name.clone()) {
                names.push(name);
            }
        }
    };
    collect(&if_node.consequent);
    if let Some(alternate) = &if_node.alternate {
        collect(alternate);
    }
    return names;
}

fn collect_block(block: &BlockStatement, names: &mut Vec<String>) {
    for statement in &block.body {
        collect_statement(statement, names);
    }
}

fn collect_statement(statement: &Statement, names: &mut Vec<String>) {
    match statement {
        Statement::Let(let_statement) => {
            names.push(let_statement.identifier.name.clone());
            collect_expression(&let_statement.expr, names);
        }
        // A class declaration binds its name the way a `let` does. The parser
        // only allows one at top level, so a block cannot hold one today; this
        // is here so the match cannot quietly drop a binder if that changes.
        Statement::Class(class) => names.push(class.name.name.clone()),
        Statement::Return(statement) => collect_expression(&statement.argument, names),
        Statement::SetProperty(statement) => {
            collect_expression(&statement.object, names);
            collect_expression(&statement.value, names);
        }
        Statement::Debugger(_) => {}
        Statement::Expr(expression) => collect_expression(expression, names),
    }
}

fn collect_expression(expression: &Expression, names: &mut Vec<String>) {
    match expression {
        Expression::IF(if_node) => {
            collect_expression(&if_node.condition, names);
            collect_block(&if_node.consequent, names);
            if let Some(alternate) = &if_node.alternate {
                collect_block(alternate, names);
            }
        }
        Expression::PREFIX(unary) => collect_expression(&unary.operand, names),
        Expression::INFIX(binary) => {
            collect_expression(&binary.left, names);
            collect_expression(&binary.right, names);
        }
        Expression::FunctionCall(call) => {
            collect_expression(&call.callee, names);
            for argument in &call.arguments {
                collect_expression(argument, names);
            }
        }
        Expression::Index(index) => {
            collect_expression(&index.object, names);
            collect_expression(&index.index, names);
        }
        Expression::Property(property) => collect_expression(&property.object, names),
        Expression::New(new) => {
            for argument in &new.arguments {
                collect_expression(argument, names);
            }
        }
        Expression::LITERAL(literal) => match literal {
            Literal::Array(array) => {
                for element in &array.elements {
                    collect_expression(element, names);
                }
            }
            Literal::Hash(hash) => {
                for (key, value) in &hash.elements {
                    collect_expression(key, names);
                    collect_expression(value, names);
                }
            }
            Literal::Integer(_) | Literal::Boolean(_) | Literal::String(_) => {}
        },
        // A function body is a scope, so nothing in it rebinds a name here.
        Expression::FUNCTION(_) => {}
        Expression::IDENTIFIER(_) | Expression::This(_) => {}
    }
}
