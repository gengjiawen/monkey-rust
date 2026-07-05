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
vm.heap_mut().run_gc();

let result = vm.export_last_result().expect("no result on stack");
assert_eq!(result, object::Object::Integer(42));
```

## Public API

- `eval_source` parses, compiles, and runs Monkey source.
- `eval` runs a parsed AST node through the bytecode compiler and GC VM.
- `compile` reuses `monkey-compiler` to produce bytecode.
- `GcVM` executes Monkey bytecode with GC-managed values.
- `GcHeap` exposes allocation, `dup`/`free`, GC triggering, and heap inspection.
- `Value`, `GcClosure`, `import_object`, and `export_object` bridge between
  GC-managed values and `object::Object`.

## Runtime Model

`monkey-gc` is designed as a parallel runtime for the existing pipeline:

```text
lexer -> parser -> compiler -> gc::GcVM
```

The crate does not replace the default `compiler::VM`, `wasm`, or playground
runtime. Instead, it reuses the same `Bytecode` and opcode definitions while
testing an alternate heap implementation.

## Modules

- `heap.rs` provides the high-level `GcHeap` and opaque `GcRef` handle.
- `runtime.rs` implements reference counting and three-phase cycle collection.
- `header.rs`, `list.rs`, and `malloc.rs` hold GC object metadata and allocator
  accounting.
- `value.rs` defines GC-managed Monkey values plus import/export helpers.
- `frame.rs` and `vm.rs` implement the bytecode VM runtime.

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
