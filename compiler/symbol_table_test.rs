#[cfg(test)]
mod tests {
    use crate::symbol_table::{shadowed_names, SymbolScope, SymbolTable};
    use parser::ast::{Expression, Node, Statement};
    #[test]
    fn test_define() {
        let mut symbol_table = SymbolTable::new();
        let symbol = symbol_table.define("x".to_string());
        assert_eq!(symbol.name, "x");
        assert_eq!(symbol.scope, SymbolScope::Global);
        assert_eq!(symbol.index, 0);
    }

    #[test]
    fn test_resolve() {
        let mut symbol_table = SymbolTable::new();
        let symbol = symbol_table.define("x".to_string());
        assert_eq!(symbol_table.resolve("x".to_string()), Some(symbol));
    }

    #[test]
    fn definitions_ledger_appends_rebindings() {
        let mut symbol_table = SymbolTable::new();
        symbol_table.define("x".to_string());
        symbol_table.define("y".to_string());
        symbol_table.define("x".to_string());

        let entries: Vec<(String, usize)> = symbol_table
            .definitions
            .iter()
            .map(|symbol| (symbol.name.clone(), symbol.index))
            .collect();
        assert_eq!(
            entries,
            vec![
                ("x".to_string(), 0),
                ("y".to_string(), 1),
                ("x".to_string(), 2),
            ]
        );
        // `symbols` only sees the latest slot; the ledger keeps all three.
        assert_eq!(symbol_table.resolve("x".to_string()).unwrap().index, 2);
    }

    #[test]
    fn every_definition_takes_a_slot_of_its_own() {
        let mut symbol_table = SymbolTable::new();
        let first = symbol_table.define("x".to_string());
        let second = symbol_table.define("x".to_string());

        // A closure created between the two `let`s captured the first slot and
        // has to keep reading it, so a rebinding never writes over it.
        assert_eq!(first.index, 0);
        assert_eq!(second.index, 1);
        assert_eq!(symbol_table.num_definitions, 2);
    }

    #[test]
    fn a_declared_slot_is_not_the_binding_yet() {
        let mut symbol_table = SymbolTable::new();
        let before = symbol_table.define("x".to_string());
        let slot = symbol_table.declare_slot("x");

        // The slot exists — code before the branch can seed it — but `x` still
        // means what it meant, so the condition and everything up to the `let`
        // that rebinds it read the old binding.
        assert_eq!(slot.index, 1);
        assert_eq!(symbol_table.num_definitions, 2);
        assert_eq!(symbol_table.resolve("x".to_string()), Some(before));

        symbol_table.rebind("x", slot.clone());
        assert_eq!(symbol_table.resolve("x".to_string()), Some(slot));
    }

    #[test]
    fn a_declared_slot_appears_in_the_ledger() {
        let mut symbol_table = SymbolTable::new();
        symbol_table.define("x".to_string());
        symbol_table.declare_slot("x");

        // Debug metadata is built from the ledger, and the shadow slot is as
        // real as any other — it is what `x` means after the branch.
        let entries: Vec<(String, usize)> = symbol_table
            .definitions
            .iter()
            .map(|symbol| (symbol.name.clone(), symbol.index))
            .collect();
        assert_eq!(entries, vec![("x".to_string(), 0), ("x".to_string(), 1)]);
    }

    #[test]
    fn builtins_and_function_names_stay_out_of_the_ledger() {
        let mut symbol_table = SymbolTable::new();
        symbol_table.define_builtin(0, "len".to_string());
        symbol_table.define_function_name("f".to_string());
        let symbol = symbol_table.define("x".to_string());

        // Neither builtins nor the self-reference slot allocate storage, so
        // `x` still gets slot 0 and is the only ledger entry.
        assert_eq!(symbol.index, 0);
        assert_eq!(symbol_table.definitions, vec![symbol]);
    }

    #[test]
    fn global_definitions_walk_to_the_outermost_scope() {
        let mut global = SymbolTable::new();
        global.define("a".to_string());
        let mut local = SymbolTable::new_enclosed_symbol_table(global);
        local.define("inner".to_string());

        let names: Vec<&str> = local
            .global_definitions()
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect();
        assert_eq!(names, vec!["a"]);
        assert_eq!(local.definitions.len(), 1);
    }

    /// The names of the sole `if` in `source`.
    fn shadowed(source: &str) -> Vec<String> {
        let Ok(Node::Program(program)) = parser::parse(source) else {
            panic!("expected `{}` to parse", source);
        };
        let Some(Statement::Expr(Expression::IF(if_node))) = program.body.first() else {
            panic!("expected `{}` to be one if expression", source);
        };
        shadowed_names(if_node)
    }

    #[test]
    fn shadowed_names_finds_every_let_an_arm_runs() {
        assert_eq!(shadowed("if (c) { let x = 1; }"), vec!["x"]);
        assert_eq!(shadowed("if (c) { let x = 1; } else { let y = 2; }"), vec!["x", "y"]);
        // One slot per name, however many times the arms bind it.
        assert_eq!(shadowed("if (c) { let x = 1; let x = 2; } else { let x = 3; }"), vec!["x"]);
    }

    #[test]
    fn shadowed_names_reaches_through_nested_ifs() {
        // A nested `if` rebinds inside this one, so its own converged slot is
        // only written when this branch runs too.
        assert_eq!(shadowed("if (c) { if (d) { let x = 1; } }"), vec!["x"]);
        assert_eq!(shadowed("if (c) { let y = if (d) { let x = 1; x }; }"), vec!["y", "x"]);
        assert_eq!(shadowed("if (c) { puts([if (d) { let x = 1; x }]); }"), vec!["x"]);
    }

    #[test]
    fn shadowed_names_stops_at_a_function_body() {
        // A function body is a scope, so its `let` cannot rebind `x` out here.
        assert_eq!(shadowed("if (c) { let f = fn() { let x = 1; x }; }"), vec!["f"]);
        assert_eq!(shadowed("if (c) { let f = fn() { if (d) { let x = 1; } x }; }"), vec!["f"]);
    }
}
