use super::Repl;

#[test]
fn repl_resolves_builtins() {
    let mut repl = Repl::new();
    let result = repl
        .eval_line("len([1, 2, 3]);")
        .expect("builtin call should succeed");
    assert_eq!(result, "3");
}

#[test]
fn repl_does_not_commit_state_from_failed_lines() {
    // The failed `let` must not leak a ghost `x` binding: the follow-up
    // `x;` has to be rejected instead of evaluating to null.
    let mut repl = Repl::new();
    let error = repl
        .eval_line("let x = 1 / 0;")
        .expect_err("division by zero should fail");
    assert!(
        error.contains("division by zero"),
        "expected runtime error, got: {:?}",
        error
    );

    let error = repl
        .eval_line("x;")
        .expect_err("ghost binding must not survive");
    assert!(
        error.to_lowercase().contains("undefined variable 'x'"),
        "ghost binding survived a failed line, got: {:?}",
        error
    );
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
