//! End-to-end tests (design §10): cross-assemble with the real aarch64
//! toolchain, run under qemu (or natively on arm64), and check observable
//! behavior — plus the handwritten ABI probes in `testdata/` that freeze the
//! `.s` ↔ runtime contract independently of the lowering pass.
//!
//! Requirements: `gcc-aarch64-linux-gnu`, `qemu-user` (non-arm64 hosts), and
//! the Rust `aarch64-unknown-linux-gnu` target. The tests are `#[ignore]`d so
//! the default suite stays hermetic; run them with
//!
//! ```text
//! cargo test -p monkey-asm -- --ignored
//! ```
//!
//! When a requirement is missing the test prints why and passes vacuously.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const CROSS_TARGET: &str = "aarch64-unknown-linux-gnu";
/// Mirrors the CLI's documented fallback link set (design §9).
const FALLBACK_NATIVE_LIBS: &[&str] = &["-lpthread", "-ldl", "-lm", "-lrt", "-lutil"];

struct Toolchain {
    cc: String,
    /// `None` when the host itself is arm64.
    qemu: Option<String>,
    runtime: PathBuf,
    cli: PathBuf,
    scratch: PathBuf,
}

fn tool_exists(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("asm/ lives inside the workspace")
}

fn cargo_build(extra: &[&str]) -> Output {
    Command::new(env!("CARGO"))
        .arg("build")
        .args(&["-p", "monkey-asm"])
        .args(extra)
        .current_dir(workspace_root())
        .output()
        .expect("cargo is runnable")
}

/// Locates the tools and builds both halves of the crate (aarch64 runtime
/// staticlib + host CLI); `None` means "environment cannot run these tests".
fn toolchain() -> Option<Toolchain> {
    let cc = std::env::var("MONKEY_ASM_CC").unwrap_or_else(|_| "aarch64-linux-gnu-gcc".to_string());
    if !tool_exists(&cc) {
        eprintln!("skipping e2e: {} not found (install gcc-aarch64-linux-gnu)", cc);
        return None;
    }
    let qemu = if cfg!(target_arch = "aarch64") {
        None
    } else {
        let qemu = std::env::var("MONKEY_ASM_QEMU").unwrap_or_else(|_| "qemu-aarch64".to_string());
        if !tool_exists(&qemu) {
            eprintln!("skipping e2e: {} not found (install qemu-user)", qemu);
            return None;
        }
        Some(qemu)
    };

    // `--lib` only: the staticlib needs no aarch64 linker, while the (unused)
    // cross-built CLI bin would.
    let cross = cargo_build(&["--lib", "--release", "--target", CROSS_TARGET]);
    if !cross.status.success() {
        let stderr = String::from_utf8_lossy(&cross.stderr).into_owned();
        if stderr.contains("may not be installed") || stderr.contains("can't find crate for `core`")
        {
            eprintln!("skipping e2e: rust target missing (rustup target add {})", CROSS_TARGET);
            return None;
        }
        panic!("aarch64 runtime build failed:\n{}", stderr);
    }
    let runtime = workspace_root()
        .join("target")
        .join(CROSS_TARGET)
        .join("release")
        .join("libmonkey_asm.a");
    assert!(runtime.exists(), "missing {}", runtime.display());

    let host = cargo_build(&[]);
    assert!(
        host.status.success(),
        "host CLI build failed:\n{}",
        String::from_utf8_lossy(&host.stderr)
    );
    let cli = workspace_root()
        .join("target")
        .join("debug")
        .join("monkey-asm");
    assert!(cli.exists(), "missing {}", cli.display());

    let scratch = std::env::temp_dir().join(format!("monkey-asm-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("scratch dir");
    Some(Toolchain {
        cc,
        qemu,
        runtime,
        cli,
        scratch,
    })
}

impl Toolchain {
    /// Assembles + links a handwritten `.s` against the runtime staticlib.
    fn link(&self, name: &str, assembly: &str) -> PathBuf {
        let source = self.scratch.join(format!("{}.s", name));
        let program = self.scratch.join(name);
        std::fs::write(&source, assembly).expect("write assembly");
        let output = Command::new(&self.cc)
            .arg(&source)
            .arg(&self.runtime)
            .arg("-o")
            .arg(&program)
            .arg("-static")
            .args(FALLBACK_NATIVE_LIBS)
            .output()
            .expect("run cross gcc");
        assert!(
            output.status.success(),
            "link failed for {}:\n{}",
            name,
            String::from_utf8_lossy(&output.stderr)
        );
        program
    }

    /// Executes an arm64 binary (optionally with `path` installed as fd 3).
    fn execute(&self, program: &Path, fd3: Option<&Path>) -> Output {
        let mut command = match (&self.qemu, fd3) {
            (Some(qemu), None) => {
                let mut command = Command::new(qemu);
                command.arg(program);
                command
            }
            (None, None) => Command::new(program),
            (qemu, Some(record)) => {
                let runner = match qemu {
                    Some(qemu) => format!("exec {} \"$1\" 3>\"$2\"", qemu),
                    None => "exec \"$1\" 3>\"$2\"".to_string(),
                };
                let mut command = Command::new("sh");
                command
                    .arg("-c")
                    .arg(runner)
                    .arg("sh")
                    .arg(program)
                    .arg(record);
                command
            }
        };
        command.output().expect("execute arm64 binary")
    }

    /// Full CLI path: `monkey-asm run <src.monkey> [--observe]`.
    fn cli_run(&self, name: &str, source: &str, observe: bool) -> Output {
        let path = self.scratch.join(format!("{}.monkey", name));
        std::fs::write(&path, source).expect("write monkey source");
        let mut command = Command::new(&self.cli);
        command.arg("run").arg(&path).current_dir(workspace_root());
        if observe {
            command.arg("--observe");
        }
        command.output().expect("run monkey-asm CLI")
    }
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
#[ignore]
fn abi_probe_freezes_the_calling_convention() {
    let toolchain = match toolchain() {
        Some(toolchain) => toolchain,
        None => return,
    };
    let program = toolchain.link("abi_probe", include_str!("testdata/abi_probe.s"));
    let output = toolchain.execute(&program, None);
    assert_eq!(stdout_of(&output), "abi\n3\n28\n7\n", "stderr: {}", stderr_of(&output));
    assert_eq!(output.status.code(), Some(0));
}

#[test]
#[ignore]
fn abi_fatal_probe_reports_overflow_and_exit_1() {
    let toolchain = match toolchain() {
        Some(toolchain) => toolchain,
        None => return,
    };
    let program = toolchain.link("abi_fatal_probe", include_str!("testdata/abi_fatal_probe.s"));
    let output = toolchain.execute(&program, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr_of(&output).contains("monkey: IntegerOverflow"));
    assert_eq!(stdout_of(&output), "");
}

#[test]
#[ignore]
fn e2e_programs_behave_like_the_interpreter() {
    let toolchain = match toolchain() {
        Some(toolchain) => toolchain,
        None => return,
    };
    let corpus: &[(&str, &str, &str)] = &[
        (
            "fib",
            "let fib = fn(n) { if (n < 2) { n } else { fib(n - 1) + fib(n - 2) } };\nputs(fib(10));",
            "55\n",
        ),
        (
            "closures",
            "let adder = fn(x) { fn(y) { x + y } };\nlet add2 = adder(2);\nputs(add2(3));\nputs(add2(40));",
            "5\n42\n",
        ),
        (
            "builtins",
            "let a = [1, 2, 3];\nputs(len(a));\nputs(first(a));\nputs(last(a));\nputs(rest(a));\nputs(push(a, 4));\nputs(a[1] + a[2]);",
            "3\n1\n3\n[2, 3]\n[1, 2, 3, 4]\n5\n",
        ),
        (
            "strings_hashes",
            "let h = {\"name\": \"monkey\", 1: 2, true: 3};\nputs(h[\"name\"] + \"!\");\nputs(h[1] + h[true]);\nputs(h[\"missing\"]);",
            "monkey!\n5\nnull\n",
        ),
        (
            "classes",
            "class Counter {\n  constructor(start) { this.count = start; }\n  inc() { this.count = this.count + 1; this.count }\n}\nlet c = new Counter(5);\nputs(c.inc());\nputs(c.inc());\nputs(c.count);",
            "6\n7\n7\n",
        ),
        (
            "big_integers",
            "puts(9223372036854775807 - 1);\nputs(0 - 9223372036854775807);\nputs(4611686018427387903 + 1);",
            "9223372036854775806\n-9223372036854775807\n4611686018427387904\n",
        ),
    ];
    for (name, source, expected) in corpus {
        let output = toolchain.cli_run(name, source, false);
        assert_eq!(
            &stdout_of(&output),
            expected,
            "program {} stderr: {}",
            name,
            stderr_of(&output)
        );
        assert_eq!(output.status.code(), Some(0), "program {}", name);
    }
}

#[test]
#[ignore]
fn e2e_fatal_errors_exit_1_with_kind() {
    let toolchain = match toolchain() {
        Some(toolchain) => toolchain,
        None => return,
    };
    let corpus: &[(&str, &str, &str)] = &[
        ("div_zero", "puts(1 / 0);", "monkey: DivisionByZero"),
        ("not_callable", "class C { m() { 1 } }\nC();", "monkey: NotCallable"),
        ("arity", "let f = fn(a) { a };\nf(1, 2);", "monkey: ArityError"),
    ];
    for (name, source, expected) in corpus {
        let output = toolchain.cli_run(name, source, false);
        assert_eq!(output.status.code(), Some(1), "program {}", name);
        assert!(
            stderr_of(&output).contains(expected),
            "program {} stderr: {}",
            name,
            stderr_of(&output)
        );
    }
}

#[test]
#[ignore]
fn e2e_observer_record_framing_and_content() {
    let toolchain = match toolchain() {
        Some(toolchain) => toolchain,
        None => return,
    };

    // CLI decode path: success and error records on stderr.
    let ok = toolchain.cli_run("observe_ok", "1 + 2;", true);
    assert_eq!(ok.status.code(), Some(0));
    assert!(
        stderr_of(&ok).contains(
            "observer: {\"status\":\"ok\",\"value\":{\"type\":\"integer\",\"value\":\"3\"}}"
        ),
        "stderr: {}",
        stderr_of(&ok)
    );
    let err = toolchain.cli_run("observe_err", "1 / 0;", true);
    assert_eq!(err.status.code(), Some(1));
    assert!(
        stderr_of(&err).contains("observer: {\"status\":\"error\",\"kind\":\"DivisionByZero\"}"),
        "stderr: {}",
        stderr_of(&err)
    );

    // Raw framing (design §10.2): u64 big-endian length + exact UTF-8 JSON,
    // one record, on fd 3 only — stdout stays the pure puts stream.
    let source_path = toolchain.scratch.join("observe_raw.monkey");
    std::fs::write(&source_path, "puts(40);\n40 + 2;").expect("write source");
    let program = toolchain.scratch.join("observe_raw");
    let build = Command::new(&toolchain.cli)
        .arg("build")
        .arg(&source_path)
        .arg("-o")
        .arg(&program)
        .arg("--observe")
        .current_dir(workspace_root())
        .output()
        .expect("run monkey-asm build");
    assert!(build.status.success(), "build stderr: {}", stderr_of(&build));

    let record_path = toolchain.scratch.join("observe_raw.record");
    let output = toolchain.execute(&program, Some(&record_path));
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout_of(&output), "40\n");

    let record = std::fs::read(&record_path).expect("observer record file");
    assert!(record.len() >= 8, "record too short: {:?}", record);
    let mut length_bytes = [0u8; 8];
    length_bytes.copy_from_slice(&record[..8]);
    let length = u64::from_be_bytes(length_bytes) as usize;
    assert_eq!(length, record.len() - 8, "length prefix must cover the payload exactly");
    let payload = std::str::from_utf8(&record[8..]).expect("payload is UTF-8");
    assert_eq!(payload, "{\"status\":\"ok\",\"value\":{\"type\":\"integer\",\"value\":\"42\"}}");
}
