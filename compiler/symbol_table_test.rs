#[cfg(test)]
mod tests {
    use crate::symbol_table::{SymbolScope, SymbolTable};
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
}
