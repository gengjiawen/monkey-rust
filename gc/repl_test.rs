use std::io::Write;
use std::process::{Command, Stdio};

fn run_repl(input: &str) -> (String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_monkey-gc"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("monkey-gc binary should start");
    child
        .stdin
        .as_mut()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("writing REPL input should succeed");
    let output = child
        .wait_with_output()
        .expect("monkey-gc should exit cleanly");
    (
        String::from_utf8(output.stdout).expect("stdout should be utf-8"),
        String::from_utf8(output.stderr).expect("stderr should be utf-8"),
    )
}

#[test]
fn repl_resolves_builtins() {
    let (stdout, stderr) = run_repl("len([1, 2, 3]);\n\n");
    assert!(
        stdout.contains("3"),
        "expected builtin result in stdout, got: {stdout:?} (stderr: {stderr:?})"
    );
    assert!(
        !stderr.contains("Undefined variable"),
        "builtins should resolve, got stderr: {stderr:?}"
    );
}

#[test]
fn repl_does_not_commit_state_from_failed_lines() {
    // The failed `let` must not leak a ghost `x` binding: the follow-up
    // `x;` has to be rejected instead of evaluating to null.
    let (stdout, stderr) = run_repl("let x = 1 / 0;\nx;\n\n");
    assert!(
        stderr.contains("division by zero"),
        "expected runtime error, got stderr: {stderr:?}"
    );
    assert!(
        stderr.to_lowercase().contains("undefined variable 'x'"),
        "ghost binding survived a failed line, stdout: {stdout:?}, stderr: {stderr:?}"
    );
}

#[test]
fn repl_keeps_state_across_successful_lines() {
    let (stdout, stderr) = run_repl("let answer = 21 * 2;\nanswer;\n\n");
    assert!(
        stdout.contains("42"),
        "expected persisted global in stdout: {stdout:?} (stderr: {stderr:?})"
    );
    assert!(stderr.is_empty(), "unexpected stderr: {stderr:?}");
}
