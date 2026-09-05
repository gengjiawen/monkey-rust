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
    assert!(error.contains("division by zero"), "expected runtime error, got: {:?}", error);

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

#[test]
fn repl_preserves_escaped_closure_constants_after_runtime_errors() {
    for (assignment, call) in [
        ("box.f = fn() { 42; };", "box.f();"),
        ("box.f = fn() { fn() { 42; }; };", "box.f()();"),
    ] {
        let mut repl = Repl::new();
        repl.eval_line("class Box {} let box = new Box();").unwrap();
        let error = repl
            .eval_line(&format!("{} 1 / 0;", assignment))
            .expect_err("the closure escapes before the runtime error");
        assert!(error.contains("division by zero"), "{}", error);

        assert_eq!(repl.eval_line(&format!("100; {}", call)).unwrap(), "42");
        assert_eq!(repl.eval_line(&format!("200; {}", call)).unwrap(), "42");
    }
}

#[test]
fn repl_preserves_escaped_global_slots_but_rolls_back_name_bindings() {
    let mut repl = Repl::new();
    repl.eval_line("class Box {} let box = new Box(); let answer = 7;")
        .unwrap();
    repl.eval_line("let answer = 42; let hidden = 9; box.f = fn() { answer + hidden; }; 1 / 0;")
        .expect_err("the closure escapes with two globals from a failed line");

    assert_eq!(repl.eval_line("answer;").unwrap(), "7");
    let error = repl.eval_line("hidden;").unwrap_err();
    assert!(error.to_lowercase().contains("undefined variable 'hidden'"), "{}", error);
    assert_eq!(
        repl.eval_line("let hidden = 100; let answer = 200; box.f();")
            .unwrap(),
        "51"
    );
    assert_eq!(repl.eval_line("hidden + answer;").unwrap(), "300");
}
