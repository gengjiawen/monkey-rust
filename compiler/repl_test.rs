use super::Repl;

#[test]
fn repl_resolves_builtins() {
    // Regression: the REPL used to seed its persistent state with a bare
    // SymbolTable::new(), so every builtin failed with "Undefined variable".
    let mut repl = Repl::new();
    let result = repl
        .eval_line("len([1, 2, 3]);")
        .expect("builtin call should succeed");
    assert_eq!(result, "3");
}

#[test]
fn repl_keeps_builtins_resolvable_across_lines() {
    // The symbol table round-trips through new_with_state on every line, so a
    // builtin that resolves on line 1 must still resolve after other lines run.
    let mut repl = Repl::new();
    repl.eval_line("let xs = [1, 2];")
        .expect("let binding should succeed");
    let result = repl
        .eval_line("len(xs);")
        .expect("builtin must survive a committed line");
    assert_eq!(result, "2");
}

#[test]
fn repl_keeps_state_across_successful_lines() {
    let mut repl = Repl::new();
    repl.eval_line("let answer = 21 * 2;")
        .expect("let binding should succeed");
    let result = repl
        .eval_line("answer;")
        .expect("persisted global should resolve");
    assert_eq!(result, "42");
}

#[test]
fn repl_allows_shadowing_a_builtin() {
    // Guards the fix's shape: re-registering builtins on every line would be a
    // tempting alternative, but it would clobber a user binding that shadows one.
    let mut repl = Repl::new();
    repl.eval_line("let len = 5;")
        .expect("shadowing a builtin should succeed");
    let result = repl
        .eval_line("len;")
        .expect("user binding must win over the builtin");
    assert_eq!(result, "5");
}

#[test]
fn repl_reports_undefined_variables() {
    let mut repl = Repl::new();
    let error = repl
        .eval_line("nope;")
        .expect_err("unknown name should fail");
    assert!(
        error.to_lowercase().contains("undefined variable 'nope'"),
        "expected an undefined-variable error, got: {:?}",
        error
    );
}

#[test]
fn repl_survives_a_parse_error() {
    let mut repl = Repl::new();
    repl.eval_line("let =")
        .expect_err("malformed let should fail");
    let result = repl
        .eval_line("1 + 1;")
        .expect("REPL should still evaluate after a parse error");
    assert_eq!(result, "2");
}
