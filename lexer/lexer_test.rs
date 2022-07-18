#[cfg(test)]
mod tests {
    use crate::token::{Token, TokenKind};
    use crate::Lexer;
    use insta::*;

    fn test_token_set(l: &mut Lexer) -> Vec<Token> {
        let mut token_vs: Vec<Token> = vec![];
        loop {
            let t = l.next_token();
            if t.kind == TokenKind::EOF {
                token_vs.push(t);
                break;
            } else {
                token_vs.push(t);
            }
        }
        token_vs
    }

    pub fn test_lexer_common(name: &str, input: &str) {
        let mut l = Lexer::new(input);
        let token_vs = test_token_set(&mut l);

        assert_snapshot!(
            name,
            serde_json::to_string_pretty(&token_vs).unwrap(),
            input
        );
    }

    #[test]
    fn test_lexer_simple() {
        test_lexer_common("simple", "=+(){},:;");
    }

    #[test]
    fn test_lexer_let() {
        test_lexer_common("let", "let x=5");
    }

    #[test]
    fn test_comments() {
        test_lexer_common("comments", "// I am comments");
    }

    #[test]
    fn test_lexer_let_with_space() {
        test_lexer_common("let_with_space", "let x = 5");
    }

    #[test]
    fn test_lexer_string() {
        test_lexer_common("string", r#""a""#);
    }

    #[test]
    fn test_lexer_array() {
        test_lexer_common("array", "[3]");
    }

    #[test]
    fn test_lexer_hash() {
        test_lexer_common("hash", r#"{"one": 1, "two": 2, "three": 3}"#);
    }

    #[test]
    fn test_lexer_bool() {
        test_lexer_common("bool", "let y=true");
    }

    #[test]
    fn test_lexer_complex() {
        test_lexer_common(
            "complex",
            "
// welcome to monkeylang
let five = 5;
let ten = 10;

let add = fn(x, y) {
  x + y;
};

let result = add(five, ten);
!-/*5;
5 < 10 > 5;

if (5 < 10) {
	return true;
} else {
	return false;
}

10 == 10;
10 != 9;",
        );
    }
}
