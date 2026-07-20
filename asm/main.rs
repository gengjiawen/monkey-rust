//! monkey-asm CLI (design §4, §9): AOT-compile Monkey to arm64.
//!
//! - `emit`  — print the generated AArch64 assembly (any host, no toolchain)
//! - `build` — assemble + link with the aarch64 runtime static library via
//!             `aarch64-linux-gnu-gcc` (never the host `cc`)
//! - `run`   — build, then execute (directly on arm64, `qemu-aarch64`
//!             elsewhere); `--observe` decodes the fd-3 record to stderr
//!
//! The runtime library is the aarch64 cross build of this same crate:
//! `cargo build -p monkey-asm --lib --release --target aarch64-unknown-linux-gnu`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use monkey_asm::lower::compile_source;

const CROSS_TARGET: &str = "aarch64-unknown-linux-gnu";
/// Documented fallback when the `rustc --print native-static-libs` probe is
/// unavailable (design §9).
const FALLBACK_NATIVE_LIBS: &[&str] = &["-lpthread", "-ldl", "-lm", "-lrt", "-lutil"];

fn usage() -> ! {
    eprintln!(
        "usage:\n  \
         monkey-asm emit <file.monkey> [--observe]\n  \
         monkey-asm build <file.monkey> [-o <output>] [--observe]\n  \
         monkey-asm run <file.monkey> [--observe]\n\n\
         environment:\n  \
         MONKEY_ASM_CC       cross compiler (default aarch64-linux-gnu-gcc)\n  \
         MONKEY_ASM_QEMU     emulator for non-arm64 hosts (default qemu-aarch64)\n  \
         MONKEY_ASM_RUNTIME  path to libmonkey_asm.a for {}",
        CROSS_TARGET
    );
    std::process::exit(2);
}

fn fail(message: &str) -> ! {
    eprintln!("monkey-asm: {}", message);
    std::process::exit(1);
}

struct Options {
    input: PathBuf,
    output: Option<PathBuf>,
    observe: bool,
}

fn parse_options(args: &[String]) -> Options {
    let mut input = None;
    let mut output = None;
    let mut observe = false;
    let mut iter = args.iter();
    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "--observe" => observe = true,
            "-o" => match iter.next() {
                Some(path) => output = Some(PathBuf::from(path)),
                None => usage(),
            },
            other if !other.starts_with('-') && input.is_none() => {
                input = Some(PathBuf::from(other));
            }
            _ => usage(),
        }
    }
    match input {
        Some(input) => Options {
            input,
            output,
            observe,
        },
        None => usage(),
    }
}

fn read_source(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => fail(&format!("cannot read {}: {}", path.display(), error)),
    }
}

fn assembly_for(options: &Options) -> String {
    let source = read_source(&options.input);
    match compile_source(&source, options.observe) {
        Ok(assembly) => assembly.text,
        Err(message) => fail(&message),
    }
}

fn tool(env_var: &str, default: &str) -> String {
    std::env::var(env_var).unwrap_or_else(|_| default.to_string())
}

/// Locates the aarch64 runtime static library: explicit env override first,
/// then the workspace `target/` layout relative to CWD and this executable.
fn runtime_library() -> PathBuf {
    if let Ok(path) = std::env::var("MONKEY_ASM_RUNTIME") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
        fail(&format!("MONKEY_ASM_RUNTIME does not exist: {}", path.display()));
    }
    let mut candidates: Vec<PathBuf> = vec![];
    for profile in &["release", "debug"] {
        candidates.push(
            PathBuf::from("target")
                .join(CROSS_TARGET)
                .join(profile)
                .join("libmonkey_asm.a"),
        );
    }
    if let Ok(exe) = std::env::current_exe() {
        // target/<profile>/monkey-asm → target/<cross>/<profile>/libmonkey_asm.a
        if let Some(target_dir) = exe.parent().and_then(Path::parent) {
            for profile in &["release", "debug"] {
                candidates.push(
                    target_dir
                        .join(CROSS_TARGET)
                        .join(profile)
                        .join("libmonkey_asm.a"),
                );
            }
        }
    }
    for candidate in &candidates {
        if candidate.exists() {
            return candidate.clone();
        }
    }
    fail(&format!(
        "runtime library libmonkey_asm.a not found; build it with\n  \
         rustup target add {target}\n  \
         cargo build -p monkey-asm --lib --release --target {target}\n\
         or point MONKEY_ASM_RUNTIME at it",
        target = CROSS_TARGET
    ));
}

/// Extra C libraries the Rust staticlib needs, read from
/// `rustc --print native-static-libs` with the design §9 fallback.
fn native_static_libs() -> Vec<String> {
    let probe = Command::new("rustc")
        .args(&[
            "--target",
            CROSS_TARGET,
            "--crate-type",
            "staticlib",
            "--print",
            "native-static-libs",
            "--emit",
            "metadata",
            "-o",
        ])
        .arg(std::env::temp_dir().join("monkey-asm-probe.meta"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(b"");
            }
            child.wait_with_output()
        });
    if let Ok(output) = probe {
        for line in String::from_utf8_lossy(&output.stderr).lines() {
            if let Some(libs) = line.split("native-static-libs:").nth(1) {
                let libs: Vec<String> = libs.split_whitespace().map(str::to_string).collect();
                if !libs.is_empty() {
                    return libs;
                }
            }
        }
    }
    FALLBACK_NATIVE_LIBS
        .iter()
        .map(|lib| lib.to_string())
        .collect()
}

fn check_tool(program: &str, hint: &str) {
    let found = Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok();
    if !found {
        fail(&format!("{} not found; {}", program, hint));
    }
}

/// Cross-assembles and links `assembly` into `output` (design §9):
/// `aarch64-linux-gnu-gcc out.s libmonkey_asm.a -o prog <native libs>`,
/// statically linked so qemu needs no sysroot.
fn build_executable(assembly: &str, output: &Path) {
    let cc = tool("MONKEY_ASM_CC", "aarch64-linux-gnu-gcc");
    check_tool(&cc, "install the aarch64 cross toolchain (gcc-aarch64-linux-gnu)");
    let runtime = runtime_library();

    let asm_path = output.with_extension("s");
    if let Err(error) = std::fs::write(&asm_path, assembly) {
        fail(&format!("cannot write {}: {}", asm_path.display(), error));
    }

    let mut link = Command::new(&cc);
    link.arg(&asm_path)
        .arg(&runtime)
        .arg("-o")
        .arg(output)
        .arg("-static");
    for lib in native_static_libs() {
        link.arg(lib);
    }
    match link.status() {
        Ok(status) if status.success() => {}
        Ok(status) => fail(&format!("{} failed with {}", cc, status)),
        Err(error) => fail(&format!("cannot run {}: {}", cc, error)),
    }
}

fn host_is_aarch64() -> bool {
    cfg!(target_arch = "aarch64")
}

/// Reads the single framed observer record: u64 big-endian length + JSON.
fn decode_observer_record(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut length_bytes = [0u8; 8];
    file.read_exact(&mut length_bytes).ok()?;
    let length = u64::from_be_bytes(length_bytes) as usize;
    let mut payload = vec![0u8; length];
    file.read_exact(&mut payload).ok()?;
    String::from_utf8(payload).ok()
}

fn run_executable(program: &Path, observe: bool) -> ! {
    let record_path = program.with_extension("observer");
    let mut direct;
    let mut with_fd3;
    let command = if observe {
        // Install the record file as fd 3 via the shell so the program's
        // stdout stays the untouched puts/print byte stream (design §10.2).
        let runner = if host_is_aarch64() {
            "exec \"$1\" 3>\"$2\"".to_string()
        } else {
            let qemu = tool("MONKEY_ASM_QEMU", "qemu-aarch64");
            check_tool(&qemu, "install qemu-user to run arm64 binaries on this host");
            format!("exec {} \"$1\" 3>\"$2\"", qemu)
        };
        with_fd3 = Command::new("sh");
        with_fd3
            .arg("-c")
            .arg(runner)
            .arg("sh")
            .arg(program)
            .arg(&record_path);
        &mut with_fd3
    } else if host_is_aarch64() {
        direct = Command::new(program);
        &mut direct
    } else {
        let qemu = tool("MONKEY_ASM_QEMU", "qemu-aarch64");
        check_tool(&qemu, "install qemu-user to run arm64 binaries on this host");
        direct = Command::new(qemu);
        direct.arg(program);
        &mut direct
    };

    let status = match command.status() {
        Ok(status) => status,
        Err(error) => fail(&format!("cannot execute {}: {}", program.display(), error)),
    };
    if observe {
        match decode_observer_record(&record_path) {
            Some(record) => eprintln!("observer: {}", record),
            None => eprintln!("observer: <no record>"),
        }
        let _ = std::fs::remove_file(&record_path);
    }
    match status.code() {
        Some(code) => std::process::exit(code),
        None => fail(&format!("program terminated by signal: {}", status)),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let command = args[0].as_str();
    let options = parse_options(&args[1..]);

    match command {
        "emit" => {
            print!("{}", assembly_for(&options));
        }
        "build" => {
            let output = options.output.clone().unwrap_or_else(|| {
                options
                    .input
                    .file_stem()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("a.out"))
            });
            let assembly = assembly_for(&options);
            build_executable(&assembly, &output);
            eprintln!(
                "monkey-asm: wrote {} and {}",
                output.display(),
                output.with_extension("s").display()
            );
        }
        "run" => {
            let assembly = assembly_for(&options);
            let dir = std::env::temp_dir().join(format!("monkey-asm-{}", std::process::id()));
            if let Err(error) = std::fs::create_dir_all(&dir) {
                fail(&format!("cannot create {}: {}", dir.display(), error));
            }
            let program = dir.join(
                options
                    .input
                    .file_stem()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("program")),
            );
            build_executable(&assembly, &program);
            run_executable(&program, options.observe);
        }
        _ => usage(),
    }
}
