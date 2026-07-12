//! Test suite for the Web and headless browsers.

#![cfg(target_arch = "wasm32")]

extern crate wasm_bindgen_test;
use monkey_wasm::{parse, run_gc_with_report};
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
    assert!(global_roots
        .iter()
        .all(|root| root["objectId"].is_number()));
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
