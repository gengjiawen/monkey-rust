# monkey-gc

QuickJS-style GC runtime for the Monkey programming language.

This crate is a GC-backed runtime that runs the same bytecode produced by
`monkey-compiler`, but stores Monkey runtime values in a `GcHeap` instead of
using `Rc<Object>` directly. It combines reference counting with a three-phase
cycle collector so cyclic object graphs can be reclaimed.

For the full design notes, see [`docs/gc.md`](../docs/gc.md).

## Usage

Parse, compile, and execute Monkey source:

```rust
let result = gc::eval_source("let add = fn(a, b) { a + b }; add(1, 2);").unwrap();
assert_eq!(result, object::Object::Integer(3));
```

Run an already parsed program:

```rust
let program = parser::parse("[1, 2, 3][1]").unwrap();
let result = gc::eval(&program).unwrap();
assert_eq!(result, object::Object::Integer(2));
```

Use the lower-level VM when you need direct access to the heap:

```rust
let program = parser::parse("let double = fn(x) { x * 2 }; double(21)").unwrap();
let bytecode = gc::compile(&program).unwrap();
let mut vm = gc::GcVM::new(bytecode);

vm.run();
let report = vm.collect_garbage();

let result = vm.export_last_result().expect("no result on stack");
assert_eq!(result, object::Object::Integer(42));
assert_eq!(report.phases.free_cycles.freed, 0);
```

Run untrusted/demo source with a fixed instruction budget and a structured
collection report:

```rust
let run = gc::run_source_with_report(source, 10_000)?;
println!("{}", run.result);
println!("collected instances: {}", run.report.collected_by_value_kind[&gc::ValueKind::Instance]);
```

## Public API

- `eval_source` parses, compiles, and runs Monkey source.
- `eval` runs a parsed AST node through the bytecode compiler and GC VM.
- `compile` reuses `monkey-compiler` to produce bytecode.
- `GcVM` executes Monkey bytecode with GC-managed values.
- `run_source_with_report` returns a staged parse/compile/runtime result plus
  before/after snapshots and per-phase collection diagnostics. Scan diagnostics
  includes sorted synthetic labels for restored and garbage-candidate objects.
- `GcVM::collect_garbage` atomically runs all three collector phases and returns
  a `GcCollectionReport`.
- `GcHeap` exposes allocation, `dup`/`free`, GC triggering, and heap inspection.
- `Value`, `GcClosure`, `import_object`, and `export_object` bridge between
  GC-managed values and `object::Object`.

## Runtime Model

`monkey-gc` is designed as a parallel runtime for the existing pipeline:

```text
lexer -> parser -> compiler -> gc::GcVM
```

The crate remains a parallel runtime rather than replacing `compiler::VM`.
The WASM package exposes it through `run_gc_with_report`, and the playground's
explicit GC tab uses that endpoint; the ordinary AST and bytecode views keep
their existing paths.

Class, instance, and bound-method values are native `GcRef` graphs. Builtins
dispatch by stable `BuiltinId` directly against GC values, so VM execution does
not depend on the legacy `Object` export/import bridge. The bridge remains for
acyclic compatibility APIs such as `eval_source`.

The playground can construct an unreachable cycle entirely in Monkey source:

```monkey
class Node {
  connect(other) { this.next = other; }
}

let makeCycle = fn() {
  let a = new Node();
  let b = new Node();
  a.connect(b);
  b.connect(a);
};
makeCycle();
```

Only a complete `gc_decref -> gc_scan -> gc_free_cycles` collection is public.
Pausing between phases would expose temporary reference counts and violate the
collector invariants.

Scan labels identify runtime objects without pretending to recover source
bindings: `Class(Node)#7`, `Instance(Node)#12`, and
`BoundMethod(Node.connect)#14`. Named closures retain their compile-time name,
for example `Closure(makeCycle)#10`; anonymous closures remain `Closure#18`.
IDs are scoped to one synchronous report.

Heap snapshots classify scalar and VM support values separately: integer,
boolean, string, null, error, compiled function, and builtin values no longer
collapse into `Other`. `Other` is reserved for GC runtime objects that are not
Monkey `Value`s. Snapshot totals still include constants and VM bookkeeping
values, not only objects explicitly created by source-level `new` expressions.

`run_gc_with_stats_bundle()` returns the object catalog plus teaching diagnostics:
object decisions with the RC formula
(`refCountBefore − heapIncomingEdges = trialRefCount`), typed visited heap
edges, and deterministic Scan restoration witnesses. `run_gc_with_stats()` is
the compatibility phase-stats view of the same atomic collection. Ordinary
`run_gc()` skips diagnostics collection.

## Modules

- `heap.rs` provides the high-level `GcHeap` and opaque `GcRef` handle.
- `gc_runtime.rs` implements reference counting and three-phase cycle collection.
- `gc_stats.rs` gathers `run_gc_with_stats` diagnostics: visited edges, object
  decisions, and restoration witnesses.
- `header.rs`, `list.rs`, and `malloc.rs` hold GC object metadata and allocator
  accounting.
- `value.rs` defines GC-managed Monkey values plus import/export helpers.
- `report.rs` defines heap snapshots and collection diagnostics.
- `frame.rs` and `vm.rs` implement the bytecode VM runtime.

## REPL

This crate also ships a `monkey-gc` binary with a stateful REPL (globals and
compiler constants persist across lines), matching `monkey-compiler`:

```sh
cargo run -p monkey-gc
```

## Development

Run the crate tests from the workspace root:

```sh
cargo test -p monkey-gc
```

Run the whole Rust workspace when changing shared compiler, object, or parser
behavior:

```sh
cargo test
```
