//! monkey-asm CLI (design §4, §9): AOT-compile Monkey to arm64.
//!
//! - `emit`  — print the generated AArch64 assembly (any host, no toolchain)
//! - `build` — assemble + link with the aarch64 runtime static library via
//!   `aarch64-linux-gnu-gcc` (never the host `cc`)
//! - `run`   — build, then execute (directly on Linux arm64,
//!   `qemu-aarch64` elsewhere); `--observe` strictly validates the fd-3
//!   record before printing it to stderr
//!
//! The runtime library is the aarch64 cross build of this same crate:
//! `cargo build -p monkey-asm --lib --release --target aarch64-unknown-linux-gnu`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use monkey_asm::lower::compile_source;
use monkey_asm::runtime_core::RuntimeErrorKind;
use serde_json::{Map as JsonMap, Value as JsonValue};

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
         MONKEY_ASM_QEMU     emulator outside Linux arm64 (default qemu-aarch64)\n  \
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

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("asm/ lives inside the workspace")
}

fn target_directory() -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace_root().join(path),
        None => workspace_root().join("target"),
    }
}

/// Locates the aarch64 runtime static library. An explicit override is used
/// verbatim; otherwise Cargo runs on every build so its freshness checks can
/// never silently select a stale debug/release archive.
fn runtime_library() -> PathBuf {
    if let Ok(path) = std::env::var("MONKEY_ASM_RUNTIME") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return path;
        }
        fail(&format!("MONKEY_ASM_RUNTIME is not a file: {}", path.display()));
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(&cargo)
        .args([
            "build",
            "-p",
            "monkey-asm",
            "--lib",
            "--release",
            "--target",
            CROSS_TARGET,
        ])
        .current_dir(workspace_root())
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => fail(&format!("runtime cargo build failed with {}", status)),
        Err(error) => fail(&format!("cannot run {} to build the runtime: {}", cargo, error)),
    }

    let runtime = target_directory()
        .join(CROSS_TARGET)
        .join("release")
        .join("libmonkey_asm.a");
    if !runtime.is_file() {
        fail(&format!("cargo succeeded but did not produce {}", runtime.display()));
    }
    runtime
}

/// Extra C libraries the Rust staticlib needs, read from
/// `rustc --print native-static-libs` with the design §9 fallback.
fn static_link_libs(libs: &str) -> Vec<String> {
    // `rustc` reports `-lgcc_s` for this GNU target, but the executable is
    // deliberately linked with `-static` and many cross toolchains do not
    // ship a static libgcc_s. The GCC driver supplies its static
    // libgcc/libgcc_eh pair itself.
    libs.split_whitespace()
        .filter(|lib| *lib != "-lgcc_s")
        .map(str::to_string)
        .collect()
}

fn native_static_libs() -> Vec<String> {
    let probe_path =
        std::env::temp_dir().join(format!("monkey-asm-native-libs-{}.a", std::process::id()));
    let probe = Command::new("rustc")
        .args([
            "--crate-name",
            "monkey_asm_native_lib_probe",
            "--target",
            CROSS_TARGET,
            "--crate-type",
            "staticlib",
            "--print",
            "native-static-libs",
            "-o",
        ])
        .arg(&probe_path)
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
    let _ = std::fs::remove_file(&probe_path);
    if let Ok(output) = probe {
        if !output.status.success() {
            return FALLBACK_NATIVE_LIBS
                .iter()
                .map(|lib| lib.to_string())
                .collect();
        }
        for line in String::from_utf8_lossy(&output.stderr).lines() {
            if let Some(libs) = line.split("native-static-libs:").nth(1) {
                let libs = static_link_libs(libs);
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
        .map(|status| status.success())
        .unwrap_or(false);
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

fn host_is_linux_aarch64() -> bool {
    cfg!(all(target_arch = "aarch64", target_os = "linux"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ObserverRecordStatus {
    Success,
    Error,
}

#[derive(Debug, Eq, PartialEq)]
struct ObserverRecord {
    payload: String,
    status: ObserverRecordStatus,
}

fn expect_fields(
    object: &JsonMap<String, JsonValue>,
    fields: &[&str],
    path: &str,
) -> Result<(), String> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(format!("{} must contain exactly fields {:?}", path, fields));
    }
    Ok(())
}

fn required_string<'a>(
    object: &'a JsonMap<String, JsonValue>,
    field: &str,
    path: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(JsonValue::as_str)
        .ok_or_else(|| format!("{}.{} must be a string", path, field))
}

fn validate_canonical_integer(raw: &str, path: &str) -> Result<(), String> {
    let parsed = raw
        .parse::<i64>()
        .map_err(|_| format!("{} must be a decimal i64 string", path))?;
    if parsed.to_string() != raw {
        return Err(format!("{} is not a canonical decimal i64 string", path));
    }
    Ok(())
}

fn validate_canonical_value(value: &JsonValue, path: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{} must be an object", path))?;
    let value_type = required_string(object, "type", path)?;
    match value_type {
        "integer" => {
            expect_fields(object, &["type", "value"], path)?;
            validate_canonical_integer(
                required_string(object, "value", path)?,
                &format!("{}.value", path),
            )
        }
        "boolean" => {
            expect_fields(object, &["type", "value"], path)?;
            if !matches!(object.get("value"), Some(JsonValue::Bool(_))) {
                return Err(format!("{}.value must be a boolean", path));
            }
            Ok(())
        }
        "null" | "function" => expect_fields(object, &["type"], path),
        "builtin" => {
            expect_fields(object, &["type", "id"], path)?;
            let id = required_string(object, "id", path)?;
            if !["len", "puts", "first", "last", "rest", "push"].contains(&id) {
                return Err(format!("{}.id is not a canonical builtin id", path));
            }
            Ok(())
        }
        "string" => {
            expect_fields(object, &["type", "value"], path)?;
            required_string(object, "value", path)?;
            Ok(())
        }
        "array" => {
            expect_fields(object, &["type", "elements"], path)?;
            let elements = object
                .get("elements")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| format!("{}.elements must be an array", path))?;
            for (index, element) in elements.iter().enumerate() {
                validate_canonical_value(element, &format!("{}.elements[{}]", path, index))?;
            }
            Ok(())
        }
        "hash" => {
            expect_fields(object, &["type", "entries"], path)?;
            let entries = object
                .get("entries")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| format!("{}.entries must be an array", path))?;
            let mut previous_key: Option<(u8, Vec<u8>)> = None;
            for (index, entry) in entries.iter().enumerate() {
                let entry_path = format!("{}.entries[{}]", path, index);
                let entry = entry
                    .as_object()
                    .ok_or_else(|| format!("{} must be an object", entry_path))?;
                expect_fields(entry, &["key", "value"], &entry_path)?;
                let key = validate_canonical_hash_key(
                    entry.get("key").expect("field checked"),
                    &format!("{}.key", entry_path),
                )?;
                if previous_key
                    .as_ref()
                    .map(|previous| previous >= &key)
                    .unwrap_or(false)
                {
                    return Err(format!(
                        "{}.key is duplicated or out of canonical order",
                        entry_path
                    ));
                }
                previous_key = Some(key);
                validate_canonical_value(
                    entry.get("value").expect("field checked"),
                    &format!("{}.value", entry_path),
                )?;
            }
            Ok(())
        }
        "class" => {
            expect_fields(object, &["type", "name"], path)?;
            required_string(object, "name", path)?;
            Ok(())
        }
        "instance" => {
            expect_fields(object, &["type", "class"], path)?;
            required_string(object, "class", path)?;
            Ok(())
        }
        "bound_method" => {
            expect_fields(object, &["type", "class", "method"], path)?;
            required_string(object, "class", path)?;
            required_string(object, "method", path)?;
            Ok(())
        }
        other => Err(format!("{}.type has unknown canonical value type {:?}", path, other)),
    }
}

fn validate_canonical_hash_key(value: &JsonValue, path: &str) -> Result<(u8, Vec<u8>), String> {
    validate_canonical_value(value, path)?;
    let object = value.as_object().expect("canonical value is an object");
    match required_string(object, "type", path)? {
        "integer" => Ok((0, required_string(object, "value", path)?.as_bytes().to_vec())),
        "boolean" => {
            let raw = object
                .get("value")
                .and_then(JsonValue::as_bool)
                .expect("validated boolean");
            Ok((1, raw.to_string().into_bytes()))
        }
        "string" => Ok((2, required_string(object, "value", path)?.as_bytes().to_vec())),
        _ => Err(format!("{} must be an integer, boolean, or string", path)),
    }
}

fn is_runtime_error_kind(kind: &str) -> bool {
    (RuntimeErrorKind::InternalError as u64..=RuntimeErrorKind::ResourceLimit as u64)
        .filter_map(RuntimeErrorKind::from_u64)
        .any(|candidate| candidate.name() == kind)
}

/// Decodes exactly one record: u64 big-endian length followed by UTF-8 JSON.
/// Any truncation, trailing byte, second frame, or schema deviation fails the
/// observer protocol.
fn decode_observer_bytes(bytes: &[u8]) -> Result<ObserverRecord, String> {
    if bytes.len() < 8 {
        return Err(format!("record is too short: {} bytes", bytes.len()));
    }
    let mut length_bytes = [0u8; 8];
    length_bytes.copy_from_slice(&bytes[..8]);
    let declared = u64::from_be_bytes(length_bytes);
    let actual = (bytes.len() - 8) as u64;
    if declared != actual {
        return Err(format!("payload length is {}, frame contains {} bytes", declared, actual));
    }
    let payload = std::str::from_utf8(&bytes[8..])
        .map_err(|error| format!("payload is not UTF-8: {}", error))?;
    let json: JsonValue =
        serde_json::from_str(payload).map_err(|error| format!("payload is not JSON: {}", error))?;
    let object = json
        .as_object()
        .ok_or_else(|| "observer payload must be an object".to_string())?;
    let status = required_string(object, "status", "observer payload")?;
    let status = match status {
        "ok" => {
            expect_fields(object, &["status", "value"], "observer payload")?;
            validate_canonical_value(
                object.get("value").expect("field checked"),
                "observer payload.value",
            )?;
            ObserverRecordStatus::Success
        }
        "error" => {
            expect_fields(object, &["status", "kind"], "observer payload")?;
            let kind = required_string(object, "kind", "observer payload")?;
            if !is_runtime_error_kind(kind) {
                return Err(format!("observer payload.kind is unknown: {:?}", kind));
            }
            ObserverRecordStatus::Error
        }
        other => return Err(format!("observer payload.status is invalid: {:?}", other)),
    };
    Ok(ObserverRecord {
        payload: payload.to_string(),
        status,
    })
}

fn decode_observer_record(path: &Path) -> Result<ObserverRecord, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read {}: {}", path.display(), error))?;
    decode_observer_bytes(&bytes)
}

fn validate_observer_exit(record: &ObserverRecord, code: Option<i32>) -> Result<(), String> {
    match (code, record.status) {
        (Some(0), ObserverRecordStatus::Success)
        | (Some(1..=i32::MAX), ObserverRecordStatus::Error) => Ok(()),
        (Some(0), ObserverRecordStatus::Error) => {
            Err("successful process emitted an error observer record".to_string())
        }
        (Some(code), ObserverRecordStatus::Success) if code != 0 => {
            Err(format!("process exited with {} but emitted a successful observer record", code))
        }
        (Some(code), ObserverRecordStatus::Error) => {
            Err(format!("process exited with unsupported negative code {}", code))
        }
        (None, _) => Err("process terminated by signal".to_string()),
        _ => unreachable!("all exit/status combinations are covered"),
    }
}

fn run_executable(program: &Path, observe: bool) -> ! {
    let record_path = program.with_extension("observer");
    let mut direct;
    let mut with_fd3;
    let command = if observe {
        // Install the record file as fd 3 via the shell so the program's
        // stdout stays the untouched puts/print byte stream (design §10.2).
        with_fd3 = Command::new("sh");
        if host_is_linux_aarch64() {
            with_fd3
                .arg("-c")
                .arg("exec \"$1\" 3>\"$2\"")
                .arg("sh")
                .arg(program)
                .arg(&record_path);
        } else {
            let qemu = tool("MONKEY_ASM_QEMU", "qemu-aarch64");
            check_tool(&qemu, "install qemu-user to run Linux arm64 ELF binaries on this host");
            with_fd3
                .arg("-c")
                .arg("exec \"$1\" \"$2\" 3>\"$3\"")
                .arg("sh")
                .arg(qemu)
                .arg(program)
                .arg(&record_path);
        }
        &mut with_fd3
    } else if host_is_linux_aarch64() {
        direct = Command::new(program);
        &mut direct
    } else {
        let qemu = tool("MONKEY_ASM_QEMU", "qemu-aarch64");
        check_tool(&qemu, "install qemu-user to run Linux arm64 ELF binaries on this host");
        direct = Command::new(qemu);
        direct.arg(program);
        &mut direct
    };

    let status = match command.status() {
        Ok(status) => status,
        Err(error) => fail(&format!("cannot execute {}: {}", program.display(), error)),
    };
    if observe {
        let record = decode_observer_record(&record_path)
            .and_then(|record| validate_observer_exit(&record, status.code()).map(|_| record));
        let _ = std::fs::remove_file(&record_path);
        match record {
            Ok(record) => eprintln!("observer: {}", record.payload),
            Err(error) => fail(&format!("observer protocol failure: {}", error)),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut record = (payload.len() as u64).to_be_bytes().to_vec();
        record.extend_from_slice(payload);
        record
    }

    fn decode(payload: &str) -> Result<ObserverRecord, String> {
        decode_observer_bytes(&frame(payload.as_bytes()))
    }

    #[test]
    fn observer_accepts_valid_success_and_error_records() {
        let success = decode(
            r#"{"status":"ok","value":{"type":"hash","entries":[{"key":{"type":"integer","value":"-1"},"value":{"type":"array","elements":[{"type":"null"}]}},{"key":{"type":"boolean","value":false},"value":{"type":"function"}},{"key":{"type":"string","value":"name"},"value":{"type":"bound_method","class":"C","method":"m"}}]}}"#,
        )
        .unwrap();
        assert_eq!(success.status, ObserverRecordStatus::Success);

        let error = decode(r#"{"status":"error","kind":"DivisionByZero"}"#).unwrap();
        assert_eq!(error.status, ObserverRecordStatus::Error);
    }

    #[test]
    fn observer_rejects_damaged_or_multiple_frames() {
        assert!(decode_observer_bytes(&[]).is_err());

        let valid = frame(br#"{"status":"ok","value":{"type":"null"}}"#);
        let mut truncated = valid.clone();
        truncated.pop();
        assert!(decode_observer_bytes(&truncated).is_err());

        let mut trailing = valid.clone();
        trailing.push(0);
        assert!(decode_observer_bytes(&trailing).is_err());

        let mut multiple = valid.clone();
        multiple.extend_from_slice(&valid);
        assert!(decode_observer_bytes(&multiple).is_err());

        assert!(decode_observer_bytes(&frame(&[0xff])).is_err());
        assert!(decode_observer_bytes(&frame(b"not json")).is_err());
    }

    #[test]
    fn observer_rejects_noncanonical_schemas() {
        let invalid = [
            "[]",
            r#"{"status":"ok"}"#,
            r#"{"status":"ok","value":{"type":"null"},"extra":true}"#,
            r#"{"status":"ok","value":{"type":"integer","value":"01"}}"#,
            r#"{"status":"ok","value":{"type":"integer","value":"9223372036854775808"}}"#,
            r#"{"status":"ok","value":{"type":"boolean","value":"true"}}"#,
            r#"{"status":"ok","value":{"type":"builtin","id":"print"}}"#,
            r#"{"status":"ok","value":{"type":"array","elements":[1]}}"#,
            r#"{"status":"ok","value":{"type":"hash","entries":[{"key":{"type":"null"},"value":{"type":"null"}}]}}"#,
            r#"{"status":"ok","value":{"type":"hash","entries":[{"key":{"type":"boolean","value":false},"value":{"type":"null"}},{"key":{"type":"integer","value":"0"},"value":{"type":"null"}}]}}"#,
            r#"{"status":"error","kind":"UnknownError"}"#,
        ];
        for payload in invalid {
            assert!(decode(payload).is_err(), "payload should fail: {}", payload);
        }
    }

    #[test]
    fn observer_status_must_match_process_exit() {
        let success = decode(r#"{"status":"ok","value":{"type":"null"}}"#).unwrap();
        let error = decode(r#"{"status":"error","kind":"TypeError"}"#).unwrap();

        assert_eq!(validate_observer_exit(&success, Some(0)), Ok(()));
        assert_eq!(validate_observer_exit(&error, Some(1)), Ok(()));
        assert_eq!(validate_observer_exit(&error, Some(2)), Ok(()));
        assert!(validate_observer_exit(&success, Some(1)).is_err());
        assert!(validate_observer_exit(&error, Some(0)).is_err());
        assert!(validate_observer_exit(&success, None).is_err());
    }

    #[test]
    fn static_native_lib_probe_drops_dynamic_libgcc() {
        let libs = static_link_libs("-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc");
        assert_eq!(libs, ["-lutil", "-lrt", "-lpthread", "-lm", "-ldl", "-lc"]);
    }
}
