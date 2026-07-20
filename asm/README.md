# monkey-asm

AOT arm64 (AArch64) assembly backend for Monkey, implementing
[docs/arm64-asm-backend-design.md](../docs/arm64-asm-backend-design.md):
a single-pass lowering from the AST to AArch64 assembly text, assembled and
linked against a Rust runtime static library. No IR, no JIT, no register
allocation — the accumulator lives in `x0` and temporaries on the machine
stack.

## One crate, built twice

| Build | Command | Product |
| --- | --- | --- |
| host | `cargo build -p monkey-asm` | `monkey-asm` CLI (parse + lower to `.s`) |
| aarch64 cross | `cargo build -p monkey-asm --lib --release --target aarch64-unknown-linux-gnu` | `libmonkey_asm.a`, the runtime the generated `.s` links against |

The generated `.s` file is the only interface between the two: the CLI never
executes Monkey code, the runtime never parses it.

## Usage

```sh
# One-time setup (Debian/Ubuntu example)
rustup target add aarch64-unknown-linux-gnu
sudo apt-get install gcc-aarch64-linux-gnu qemu-user

# Build the runtime library once (--lib: the staticlib needs no cross linker)
cargo build -p monkey-asm --lib --release --target aarch64-unknown-linux-gnu

# Compile and run a program (uses qemu-aarch64 automatically on non-arm64 hosts)
cargo run -p monkey-asm -- run examples/fib.monkey

# Just look at the assembly (works on any host, no toolchain needed)
cargo run -p monkey-asm -- emit examples/fib.monkey

# Produce a static arm64 executable + its .s next to it
cargo run -p monkey-asm -- build examples/fib.monkey -o fib
```

Environment overrides: `MONKEY_ASM_CC` (default `aarch64-linux-gnu-gcc`),
`MONKEY_ASM_QEMU` (default `qemu-aarch64`), `MONKEY_ASM_RUNTIME` (path to
`libmonkey_asm.a`).

`--observe` builds the differential-testing variant: the program writes one
canonical result record (u64 big-endian length + JSON) to fd 3 at exit, while
stdout stays the untouched `puts` byte stream; `run --observe` decodes the
record to stderr.

## Layout

- `runtime_core.rs` — storage-agnostic semantics: tagged values (SMI +
  boxed integers, heap refs, builtin immediates), checked arithmetic, the
  equality/truthiness/display matrices, builtins, call/construct dispatch,
  canonical observer JSON.
- `runtime_backend.rs` — `ValueStore` trait with the two backends:
  `PointerStore` (native tagged pointers, leaked cells) and `HandleStore`
  (arena indices, used by tests and a future wasm simulator).
- `runtime.rs` — the `extern "C"` `rt_*` shells the generated assembly calls;
  fatal-error and observer plumbing.
- `emitter.rs` — assembly text buffers, labels, `.rodata` interning, source
  span map, and the AArch64 encoding-limit helpers (`load_imm64`, frame/sp
  addressing) that lowering must never bypass.
- `lower.rs` — AST → assembly for the full language (functions, closures,
  classes), reusing the bytecode compiler's `SymbolTable` for scope analysis.
- `main.rs` — `emit`/`build`/`run` CLI.
- `testdata/*.s` — handwritten ABI probes freezing the `.s` ↔ runtime
  contract.

## Tests

```sh
cargo test -p monkey-asm            # unit + snapshot tests (host only)
cargo test -p monkey-asm -- --ignored   # e2e: cross-assemble + run under qemu
```

The e2e tests skip themselves with a message when the cross toolchain, qemu,
or the Rust aarch64 target is missing.
