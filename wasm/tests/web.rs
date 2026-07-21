//! Test suite for the Web and headless browsers.

#![cfg(target_arch = "wasm32")]

extern crate wasm_bindgen_test;
use monkey_wasm::{compile_to_arm64, compile_to_snapshot, parse, run_gc_with_report, run_snapshot};
use serde_json::Value;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn pass() {
    let input = "let a = 3";
    let r = parse(input);
    println!("{}", r);
}

fn run_gc(source: &str) -> Value {
    serde_json::from_str(&run_gc_with_report(source)).expect("valid GC envelope JSON")
}

#[wasm_bindgen_test]
fn gc_success_envelope_reports_collected_instances() {
    let envelope = run_gc(
        r#"
class Node {
  constructor(value) { this.value = value; }
  connect(other) { this.next = other; }
}
let makeCycle = fn() {
  let a = new Node("a");
  let b = new Node("b");
  a.connect(b);
  b.connect(a);
};
makeCycle();
"#,
    );

    assert_eq!(envelope["status"], "ok");
    assert_eq!(envelope["result"], "null");
    assert_eq!(envelope["report"]["before"]["byValueKind"]["instance"], 2);
    assert_eq!(envelope["report"]["after"]["byValueKind"]["instance"], 0);
    assert_eq!(envelope["report"]["before"]["byValueKind"]["string"], 9);
    assert_eq!(envelope["report"]["before"]["byValueKind"]["null"], 1);
    assert_eq!(envelope["report"]["before"]["byValueKind"]["compiledFunction"], 4);
    assert_eq!(envelope["report"]["before"]["byValueKind"]["other"], 0);
    assert_eq!(envelope["report"]["collectedByValueKind"]["instance"], 2);
    assert!(envelope["report"]["phases"]["trialDeletion"]["edgesVisited"].is_number());
    assert!(envelope["report"]["phases"]["scan"]["garbageCandidates"].is_number());
    let restored = envelope["report"]["phases"]["scan"]["restoredObjects"]
        .as_array()
        .expect("restored object summaries");
    assert!(restored.iter().any(|object| {
        object["kind"] == "closure"
            && object["label"]
                .as_str()
                .is_some_and(|label| label.starts_with("Closure(Node.connect)#"))
    }));
    let garbage = envelope["report"]["phases"]["scan"]["garbageCandidateObjects"]
        .as_array()
        .expect("garbage candidate summaries");
    assert_eq!(garbage.len(), 2);
    assert!(garbage.iter().all(|object| {
        object["kind"] == "instance"
            && object["label"]
                .as_str()
                .is_some_and(|label| label.starts_with("Instance(Node)#"))
    }));
    assert!(envelope["report"]["phases"]["freeCycles"]["freed"].is_number());
    assert!(envelope["report"]["objects"].is_array());
    let global_roots = envelope["report"]["globalRoots"]
        .as_array()
        .expect("global roots");
    let root_names: Vec<&str> = global_roots
        .iter()
        .filter_map(|root| root["name"].as_str())
        .collect();
    assert_eq!(root_names, vec!["Node", "makeCycle"]);
    assert!(global_roots.iter().all(|root| root["objectId"].is_number()));
    assert!(envelope["report"]["phases"]["trialDeletion"]["objectDecisions"].is_array());
    assert!(envelope["report"]["phases"]["trialDeletion"]["visitedEdges"].is_array());
    assert!(envelope["report"]["phases"]["scan"]["restorationWitnesses"].is_array());
    let decisions = envelope["report"]["phases"]["trialDeletion"]["objectDecisions"]
        .as_array()
        .expect("object decisions");
    assert!(decisions
        .iter()
        .any(|decision| { decision["decision"] == "candidate" && decision["final"] == "freed" }));
    let edges = envelope["report"]["phases"]["trialDeletion"]["visitedEdges"]
        .as_array()
        .expect("visited edges");
    assert!(edges.iter().any(|edge| {
        edge["relation"]["kind"] == "instanceField" && edge["relation"]["name"] == "next"
    }));
}

#[wasm_bindgen_test]
fn gc_hash_edges_preserve_key_kinds_and_unicode_boundaries() {
    let unicode_key = format!("{}中", "a".repeat(63));
    let source = format!(
        r#"
let values = {{1: 1, "1": 2, true: 3, "true": 4, "{}": 5}};
values;
"#,
        unicode_key
    );
    let envelope = run_gc(&source);

    assert_eq!(envelope["status"], "ok");
    let edges = envelope["report"]["phases"]["trialDeletion"]["visitedEdges"]
        .as_array()
        .expect("visited edges");
    let has_hash_key = |key_kind: &str, key: &str| {
        edges.iter().any(|edge| {
            edge["relation"]["kind"] == "hashValue"
                && edge["relation"]["keyKind"] == key_kind
                && edge["relation"]["key"] == key
        })
    };

    assert!(has_hash_key("integer", "1"));
    assert!(has_hash_key("string", "1"));
    assert!(has_hash_key("boolean", "true"));
    assert!(has_hash_key("string", "true"));
    assert!(has_hash_key("string", &format!("{}…", "a".repeat(63))));
}

#[wasm_bindgen_test]
fn gc_error_envelope_distinguishes_all_stages_and_instruction_limit() {
    let parse_error = run_gc("let =");
    assert_eq!(parse_error["status"], "error");
    assert_eq!(parse_error["stage"], "parse");
    assert!(parse_error["span"].is_null());

    let compile_error = run_gc("this;");
    assert_eq!(compile_error["status"], "error");
    assert_eq!(compile_error["stage"], "compile");
    assert!(compile_error["span"].is_null());

    let runtime_error = run_gc("1.value;");
    assert_eq!(runtime_error["status"], "error");
    assert_eq!(runtime_error["stage"], "runtime");
    assert_eq!(runtime_error["span"]["start"], 0);
    assert_eq!(runtime_error["span"]["end"], 7);

    let instruction_error = run_gc(&"1;".repeat(5_001));
    assert_eq!(instruction_error["status"], "error");
    assert_eq!(instruction_error["stage"], "runtime");
    assert!(instruction_error["message"]
        .as_str()
        .unwrap()
        .contains("instruction limit exceeded"));
}

fn build_snapshot(source: &str, strip_debug: bool) -> Value {
    serde_json::from_str(&compile_to_snapshot(source, strip_debug))
        .expect("valid snapshot envelope JSON")
}

fn snapshot_bytes(envelope: &Value) -> Vec<u8> {
    let hex = envelope["bytesHex"].as_str().expect("bytesHex string");
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex byte"))
        .collect()
}

fn run_bytes(bytes: &[u8]) -> Value {
    serde_json::from_str(&run_snapshot(bytes)).expect("valid snapshot run envelope JSON")
}

#[wasm_bindgen_test]
fn snapshot_envelope_roundtrips_through_the_vm() {
    let envelope = build_snapshot("let add = fn(a, b) { a + b }; add(1, 2)", false);
    assert_eq!(envelope["status"], "ok");

    let bytes = snapshot_bytes(&envelope);
    assert_eq!(envelope["layout"]["byteLength"], bytes.len());

    let run = run_bytes(&bytes);
    assert_eq!(run["status"], "ok");
    assert_eq!(run["result"], "3");
}

#[wasm_bindgen_test]
fn snapshot_layout_regions_tile_the_buffer_and_disassemble() {
    let envelope = build_snapshot("let add = fn(a, b) { a + b }; add(1, 2)", false);
    let layout = &envelope["layout"];
    assert_eq!(layout["formatVersion"], 1);
    assert_eq!(layout["hasDebugInfo"], true);
    assert!(layout["abiFingerprint"].as_str().unwrap().starts_with("0x"));

    let regions = layout["regions"].as_array().expect("regions array");
    let labels: Vec<&str> = regions
        .iter()
        .take(4)
        .filter_map(|region| region["label"].as_str())
        .collect();
    assert_eq!(labels, vec!["magic", "version", "abi fingerprint", "flags"]);

    let mut cursor = 0u64;
    for region in regions {
        assert_eq!(region["offset"], cursor);
        cursor += region["length"].as_u64().expect("region length");
    }
    assert_eq!(Value::from(cursor), layout["byteLength"]);
    assert!(regions
        .iter()
        .any(|region| region["label"] == "OpCall 2" && region["section"] == "main"));
}

#[wasm_bindgen_test]
fn stripped_snapshots_drop_debug_info_and_error_spans() {
    let source = "let not_callable = 5; not_callable()";
    let with_debug = build_snapshot(source, false);
    let stripped = build_snapshot(source, true);

    assert_eq!(stripped["layout"]["hasDebugInfo"], false);
    assert!(
        stripped["layout"]["byteLength"].as_u64().unwrap()
            < with_debug["layout"]["byteLength"].as_u64().unwrap()
    );
    assert!(stripped["layout"]["regions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|region| region["section"] != "debug"));

    let with_debug_run = run_bytes(&snapshot_bytes(&with_debug));
    assert_eq!(with_debug_run["status"], "error");
    assert_eq!(with_debug_run["stage"], "runtime");
    assert!(with_debug_run["span"]["start"].is_number());

    let stripped_run = run_bytes(&snapshot_bytes(&stripped));
    assert_eq!(stripped_run["status"], "error");
    assert_eq!(stripped_run["stage"], "runtime");
    assert!(stripped_run["span"].is_null());
    assert_eq!(stripped_run["message"], with_debug_run["message"]);
}

#[wasm_bindgen_test]
fn hostile_snapshot_bytes_are_rejected_before_the_vm() {
    let mut bytes = snapshot_bytes(&build_snapshot("1", false));
    bytes[0] = b'X';

    let run = run_bytes(&bytes);
    assert_eq!(run["status"], "error");
    assert_eq!(run["stage"], "snapshot");
    assert!(run["message"].as_str().unwrap().contains("BadMagic"));
}

#[wasm_bindgen_test]
fn snapshot_runs_share_the_playground_instruction_budget() {
    let source = "
let fibonacci = fn(n) {
  if (n < 2) { n } else { fibonacci(n - 1) + fibonacci(n - 2) }
};
fibonacci(30);
";
    let run = run_bytes(&snapshot_bytes(&build_snapshot(source, false)));
    assert_eq!(run["status"], "error");
    assert_eq!(run["stage"], "runtime");
    assert!(run["message"]
        .as_str()
        .unwrap()
        .contains("instruction limit exceeded"));
}

#[wasm_bindgen_test]
fn snapshot_parse_and_compile_failures_are_envelope_data() {
    let parse_error = build_snapshot("let =", false);
    assert_eq!(parse_error["status"], "error");
    assert_eq!(parse_error["stage"], "parse");

    let compile_error = build_snapshot("this;", false);
    assert_eq!(compile_error["status"], "error");
    assert_eq!(compile_error["stage"], "compile");
}

fn build_arm64(source: &str) -> Value {
    serde_json::from_str(&compile_to_arm64(source)).expect("valid arm64 envelope JSON")
}

#[wasm_bindgen_test]
fn arm64_envelope_lines_carry_kinds_and_spans() {
    let source = "let answer = 1 + 2; answer";
    let envelope = build_arm64(source);
    assert_eq!(envelope["status"], "ok");

    let lines = envelope["lines"].as_array().expect("lines array");
    let has = |kind: &str, needle: &str| {
        lines.iter().any(|line| {
            line["kind"] == kind && line["text"].as_str().unwrap_or("").contains(needle)
        })
    };
    assert!(has("label", "main:"));
    assert!(has("directive", ".globl main"));
    assert!(has("comment", "// let answer = 1 + 2;"));
    assert!(has("code", "bl rt_globals_init"));
    assert!(lines
        .iter()
        .any(|line| line["kind"] == "blank" && line["text"] == ""));

    // Every span stays inside the source and is well-formed; at least one code
    // line maps back to the `1 + 2` initializer for the godbolt linkage.
    for line in lines {
        if line["span"].is_null() {
            continue;
        }
        let start = line["span"]["start"].as_u64().expect("span start");
        let end = line["span"]["end"].as_u64().expect("span end");
        assert!(start <= end && end <= source.len() as u64);
    }
    let initializer = (
        source.find("1 + 2").unwrap() as u64,
        source.find("1 + 2").unwrap() as u64 + "1 + 2".len() as u64,
    );
    assert!(lines.iter().any(|line| {
        line["kind"] == "code"
            && line["span"]["start"] == initializer.0
            && line["span"]["end"] == initializer.1
    }));
}

#[wasm_bindgen_test]
fn arm64_parse_and_lowering_failures_are_envelope_data() {
    let parse_error = build_arm64("let =");
    assert_eq!(parse_error["status"], "error");
    assert_eq!(parse_error["stage"], "parse");
    assert!(parse_error["span"].is_null());

    let compile_error = build_arm64("missing;");
    assert_eq!(compile_error["status"], "error");
    assert_eq!(compile_error["stage"], "compile");
    assert_eq!(compile_error["message"], "undefined variable 'missing'");
    assert_eq!(compile_error["span"]["start"], 0);
    assert_eq!(compile_error["span"]["end"], 7);
}
