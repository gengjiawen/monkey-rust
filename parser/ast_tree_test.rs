#[cfg(test)]
mod tests {
    use crate::parse;
    use insta::*;

    pub fn test_ast_tree(name: &str, input: &str) {
        let ast = match parse(input) {
            Ok(node) => serde_json::to_string_pretty(&node).unwrap(),
            Err(e) => panic!("parse error: {}", e[0]),
        };
        assert_snapshot!(name, ast, input);
    }

    // https://astexplorer.net/#/gist/3a8ce5192e08ab973d255db5295671b1/831267b339e5562244c88f88587744153fbcfb6b
    #[test]
    fn test_let() {
        let input = "let a = 3";
        test_ast_tree("test_let", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/f462e81e4940309f2c6a694a3bdedd283e7b035d
    #[test]
    fn test_string() {
        let input = r#""jw""#;
        test_ast_tree("test_string", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/e74ab41b095abe19f2dd0c0398ffae94674d0a8c
    #[test]
    fn test_array() {
        let input = "[1, true]";
        test_ast_tree("test_array", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/9263c8a45953e56d209597e90299547a733622a9
    #[test]
    fn test_hash() {
        let input = r#"{"a": 1}"#;
        test_ast_tree("test_hash", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/ebbaf31807ccdcec6605aaa2d3a222258cac7f28
    #[test]
    fn test_return() {
        let input = "return 3";
        test_ast_tree("test_return", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/5bf612e5f406bc59076f54371671af05022a74d0
    #[test]
    fn test_unary() {
        let input = "-3";
        test_ast_tree("test_unary", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/7a2707a45fffc15d322807ffa2738b16b2690a67
    #[test]
    fn test_binary() {
        let input = "1 + 2 * 3";
        test_ast_tree("test_binary", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/ab013f6feb719e8bece669b375c8b2d43f2231d0
    #[test]
    fn test_binary_nested() {
        let input = "1+2+3";
        test_ast_tree("test_binary_nested", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/25ae0e103e732ace3d443a4bf0c620a44bedcd55
    #[test]
    fn test_if() {
        let input = "if (x < y) { x } else { y }";
        test_ast_tree("test_if", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/9f5b73a95afba5c5300f67ac69f837048da77750
    #[test]
    fn test_func_declaration() {
        let input = "fn(x) { x };";
        test_ast_tree("test_func_declaration", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/3213bab3987673584c55b2f5ff4d723369aecd8d
    #[test]
    fn test_func_call() {
        let input = "add(1, 2)";
        test_ast_tree("test_func_call", input)
    }

    // https://astexplorer.net/#/gist/0911a07ddb31d261074d1d59f6291a7c/3d5d8bfe7ec5b192674f46211408a07d88d14c65
    #[test]
    fn test_index() {
        let input = "a[1]";
        test_ast_tree("test_index", input)
    }
}
