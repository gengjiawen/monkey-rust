# `@gengjiawen/monkey-lint`

A static linter for the Monkey language in this repository. It reuses the
Rust/Wasm parser and its validation pass, then runs a set of AST rules whose
behaviour is anchored to both Monkey backends (the tree-walking interpreter and
the GC bytecode VM) — a construct is only flagged when both backends agree it is
wrong or useless.

```ts
import { lint } from '@gengjiawen/monkey-lint'

const { diagnostics } = lint('let x = 1; puts("hi");')
// [{ rule: 'no-unused-let', severity: 'warn',
//    message: "'x' is declared but never used", span: { start: 4, end: 5 } }]
```

Each diagnostic carries a `rule` id, a `severity` (`error` or `warn`), a
`message`, and a UTF-8 byte `span` (absent only for parser errors without a
location). A parse or validation failure is reported as a single `error`
diagnostic and no rules run.

## Rules

All rules default to `warn`.

| Rule | Flags |
| --- | --- |
| `no-unused-let` | a `let`/`class` binding that is never referenced |
| `no-unused-param` | a parameter that is never referenced (opt out with a leading `_`) |
| `no-unused-expression` | an expression statement whose value is discarded and has no side effect |
| `no-unreachable-code` | a statement following a `return` in the same block |
| `no-duplicate-hash-key` | a scalar literal key written more than once in one hash |
| `builtin-arity` | a call to `len` with anything other than one argument |
| `no-shadowed-builtin` | a binding whose name shadows a predefined builtin |
| `no-constant-condition` | an `if` whose condition is a literal |
| `no-literal-type-mismatch` | an operator applied to two literals of incompatible types |

Override a rule's level with the `rules` option (`off`, `warn`, or `error`):

```ts
lint(source, { rules: { 'no-unused-let': 'error', 'no-shadowed-builtin': 'off' } })
```

## Node and CLI

The same `lint` import resolves to a browser build under a bundler and to a Node
build (`main`) under Node. The Node build and the CLI require Node 24 or newer
and load the same Wasm module through Node's WebAssembly API.

The CLI reads files or stdin:

```sh
monkey-lint input.monkey
monkey-lint --format json input.monkey
monkey-lint --rule no-unused-let:error --deny-warnings < input.monkey
```

It exits `1` when any `error` is reported, or when `--deny-warnings` is set and
any warning is reported; otherwise `0`.
