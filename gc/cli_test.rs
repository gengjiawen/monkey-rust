use std::fs;
use std::path::{Path, PathBuf};

use compiler::snapshot::read_bytecode;

use super::{compile_command, run_command};

/// Scratch file under a shared temp directory. Tests run in parallel, so
/// every test uses file names unique to itself.
fn scratch_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("monkey-gc-cli-tests");
    fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn write_source(name: &str, source: &str) -> PathBuf {
    let path = scratch_path(name);
    fs::write(&path, source).unwrap();
    path
}

fn arg(path: &Path) -> String {
    path.display().to_string()
}

#[test]
fn run_executes_monkey_source_directly() {
    let source = write_source("direct.monkey", "let add = fn(a, b) { a + b }; add(20, 22)");
    assert_eq!(run_command(&[arg(&source)]).unwrap(), "42");
}

#[test]
fn compile_then_run_matches_direct_execution() {
    let source = write_source("roundtrip.monkey", "let add = fn(a, b) { a + b }; add(20, 22)");
    let output = scratch_path("roundtrip.mbc");
    compile_command(&[arg(&source), "-o".to_string(), arg(&output)]).unwrap();
    assert_eq!(run_command(&[arg(&output)]).unwrap(), run_command(&[arg(&source)]).unwrap());
}

#[test]
fn compile_defaults_to_sibling_mbc_path() {
    let source = write_source("default-output.monkey", "1 + 1");
    compile_command(&[arg(&source)]).unwrap();
    let blob = fs::read(source.with_extension("mbc")).unwrap();
    assert!(read_bytecode(&blob).is_ok());
}

#[test]
fn stripped_snapshots_lose_runtime_error_spans() {
    let source = write_source("spans.monkey", "let not_callable = 5; not_callable()");
    let full = scratch_path("spans-full.mbc");
    let stripped = scratch_path("spans-stripped.mbc");
    compile_command(&[arg(&source), "-o".to_string(), arg(&full)]).unwrap();
    compile_command(&[
        arg(&source),
        "-o".to_string(),
        arg(&stripped),
        "--strip".to_string(),
    ])
    .unwrap();

    let full_error = run_command(&[arg(&full)]).unwrap_err();
    assert_eq!(full_error.exit_code, 1);
    assert!(full_error.message.contains("source offset"), "got: {}", full_error.message);

    let stripped_error = run_command(&[arg(&stripped)]).unwrap_err();
    assert_eq!(stripped_error.exit_code, 1);
    assert!(!stripped_error.message.contains("source offset"), "got: {}", stripped_error.message);
}

#[test]
fn max_instructions_flag_bounds_execution() {
    let source = write_source(
        "budget.monkey",
        "let spin = fn(n) { if (n < 1) { 0 } else { spin(n - 1) } }; spin(100000)",
    );
    let error = run_command(&[
        arg(&source),
        "--max-instructions".to_string(),
        "10".to_string(),
    ])
    .unwrap_err();
    assert_eq!(error.exit_code, 1);
    assert!(error.message.contains("instruction limit exceeded"), "got: {}", error.message);
}

#[test]
fn corrupt_mbc_reports_snapshot_error_not_parse_error() {
    let path = scratch_path("corrupt.mbc");
    fs::write(&path, b"not bytecode").unwrap();
    let error = run_command(&[arg(&path)]).unwrap_err();
    assert_eq!(error.exit_code, 1);
    assert!(error.message.contains("BadMagic"), "got: {}", error.message);
}

#[test]
fn usage_errors_exit_with_code_two() {
    assert_eq!(run_command(&[]).unwrap_err().exit_code, 2);
    assert_eq!(compile_command(&[]).unwrap_err().exit_code, 2);

    let source = write_source("usage.monkey", "1");
    let unknown_flag = run_command(&[arg(&source), "--frobnicate".to_string()]).unwrap_err();
    assert_eq!(unknown_flag.exit_code, 2);

    let missing_count = run_command(&[arg(&source), "--max-instructions".to_string()]).unwrap_err();
    assert_eq!(missing_count.exit_code, 2);

    let bad_count = run_command(&[
        arg(&source),
        "--max-instructions".to_string(),
        "many".to_string(),
    ])
    .unwrap_err();
    assert_eq!(bad_count.exit_code, 2);

    let compiled_input = compile_command(&["foo.mbc".to_string()]).unwrap_err();
    assert_eq!(compiled_input.exit_code, 2);
}

#[test]
fn missing_input_files_fail_with_code_one() {
    let missing = scratch_path("definitely-missing.monkey");
    let error = run_command(&[arg(&missing)]).unwrap_err();
    assert_eq!(error.exit_code, 1);
}
