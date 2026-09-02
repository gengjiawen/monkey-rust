# `@gengjiawen/monkey-typechecker`

A gradual type checker for the Monkey language in this repository. It reuses the
Rust/Wasm parser and its validation pass, then walks the AST reading the optional
type annotations the language gained in `let` bindings, parameters and return
positions.

Annotations are optional and every backend erases them, so an unannotated
program still checks — it just infers `any` in more places. The checker follows
the measured behaviour of the runtimes and is deliberately unsound in a few
documented spots rather than noisy: it would rather miss an error than invent
one.

```ts
import { check } from '@gengjiawen/monkey-typechecker'

const { diagnostics } = check('let name: string = 42;')
// [{ code: 'type-mismatch', severity: 'error',
//    message: "type 'int' is not assignable to type 'string'",
//    span: { start: 19, end: 21 } }]
```

Each diagnostic carries a kebab-case `code`, a `severity` (`error` or
`warning`), a `message`, and a UTF-8 byte `span` (absent only for parser errors
without a location). A parse or validation failure is reported as a single
`parse-error` / `validation-error` diagnostic and no type rules run.

## Diagnostics

| Code                 | Reported when                                                            |
| -------------------- | ------------------------------------------------------------------------ |
| `type-mismatch`      | an assignment, argument, field write or return value is incompatible     |
| `operator-type`      | an operator's operands do not satisfy its signature                      |
| `mixed-equality`     | `==` / `!=` compare different categories — GC VM raises, others do not   |
| `invalid-comparison` | `==` / `!=` on an array, hash or function                                |
| `arity-mismatch`     | a call or `new` passes the wrong number of arguments                     |
| `not-callable`       | the callee's static type is not a function                               |
| `not-constructable`  | `new` is applied to something other than a class                         |
| `unknown-property`   | a property read or write misses the class's property map                 |
| `assign-to-method`   | a write targets a method name (it would shadow it on that instance only) |
| `unknown-type-name`  | an annotation names a type that is not in scope                          |
| `reserved-type-name` | a class shadows a builtin type name (warning)                            |
| `invalid-hash-key`   | a hash key type is not hashable                                          |
| `invalid-index`      | the index target or subscript type is wrong                              |

## Types

```text
int | bool | string | null | any
[T]            array
{K: V}         hash
fn(A, B): R    function
T?             nullable, i.e. the union of T and null
```

Instances are nominal by declaration identity: two classes with the same name
are two types, and an alias saved before a redeclaration keeps pointing at the
first one. Unions have no source syntax; they arise from `if` branches, array
and hash literals, and `T?`.

Three behaviours are worth knowing up front, all of them chosen to keep idiomatic
Monkey quiet:

- **`any` is total.** Any operation on an `any` value is allowed, and the result
  is the operator's own result type where that is unambiguous (`any - 1` is
  `int`) or `any` otherwise.
- **Null is optimistic.** `null` is stripped from a union before a check, so
  `first(xs) + 1` passes even though it fails at runtime on an empty array. The
  nullability is still carried in the type and shown in diagnostics.
- **Return types are not inferred across functions.** An unannotated recursive
  function, or an unannotated method called from a sibling method, is seen as
  returning `any`. Annotating the return type restores the precise check.

See `docs/type-system-design.md` for the full semantics, including the runtime
evidence behind each rule.

## Node and bundlers

The `check` import resolves to a browser build under a bundler and to a Node
build (`main`) under Node. The Node build requires Node 24 or newer and loads the
same Wasm module through Node's WebAssembly API.

`checkProgram` is also exported for callers that already have a parsed program
from `analyze_lossless` and do not want to pay for a second parse.
