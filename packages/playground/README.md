# monkey-playground

Interactive online playground for the Monkey programming language.

### Compiler playground

https://monkey-lang-playground-jw.vercel.app/

Repo: https://github.com/gengjiawen/monkey-rust

## Features

- Editor with syntax highlighting and Vim mode
- AST, bytecode, and explicit GC output views
- Code snippet library with localStorage persistence
- WASM-powered parser (via [@gengjiawen/monkey-wasm](https://www.npmjs.com/package/@gengjiawen/monkey-wasm))
- Formatting powered by [prettier-plugin-monkey](https://www.npmjs.com/package/prettier-plugin-monkey)
- JS-style Monkey classes with `constructor`, `this`, `new`, methods, and mutable instance fields
- A `Class cycle (GC)` example that creates an unreachable two-instance cycle in Monkey source
- Before/after heap snapshots, per-value-kind counts, and diagnostics for the three collector phases
- Object decision walkthrough (RC before / heap in-edges / trial RC / Candidate|Survivor / Scan / Final)
- Visited heap edges with typed relations (`fields["next"]`, `items[0]`, …) and Scan reachability witnesses
- Heap topology graph plus a step-by-step phase replay that animates trial deletion, Scan restores, and freeing over the same drawing
- Tagged parse/compile/runtime errors and stale-request protection for GC runs

The GC view executes only when **Run GC** is pressed. Editing never runs the
program automatically, and the collector always runs
`gc_decref -> gc_scan -> gc_free_cycles` atomically. Object IDs are scoped to a
single report; `Edges visited` counts heap-to-heap references only; witnesses
are deterministic reachability proofs, not the collector's real event order.
`Tracked bytes` is Monkey collector accounting, not browser resident memory;
WebAssembly linear memory may remain allocated after objects are reclaimed.

## Development

After Rust parser/compiler/GC changes, rebuild `wasm/pkg` before running the
playground because it consumes the generated package rather than Rust sources:

```sh
cd wasm
wasm-pack build --release --scope=gengjiawen
```

Then run the package checks from the repository root:

```sh
pnpm -C packages/playground test
pnpm -C packages/playground lint
pnpm -C packages/playground build
```
