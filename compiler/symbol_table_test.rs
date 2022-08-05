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
}
