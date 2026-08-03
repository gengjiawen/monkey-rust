# @gengjiawen/monkey-ast-types

TypeScript definitions for the AST JSON that
[`@gengjiawen/monkey-wasm`](https://www.npmjs.com/package/@gengjiawen/monkey-wasm)
emits, shared by the Monkey linter, minifier, prettier plugin and type checker.

```ts
import type { Program, LetStatement } from '@gengjiawen/monkey-ast-types'
import { analyze_lossless } from '@gengjiawen/monkey-wasm'

const result = JSON.parse(analyze_lossless('let x: int = 5;'))
const program: Program = result.program
const statement = program.body[0] as LetStatement
statement.type_annotation // NamedType { name: 'int' }
```

## Two parser entries, one difference

`analyze_lossless` keeps integer literals as their source text; the plain
`parse` entry has already converted them to JS numbers. `IntegerLiteral.raw` is
typed `string | number` for that reason — call `String(...)` when you need text.

## Type annotations

Annotations (`let x: int = 5`, `fn(a: int): int`) are optional syntax that every
Monkey backend erases before execution, so a tool that only models runtime
behavior can skip those subtrees whole:

```ts
import {
  isTypeAnnotation,
  printTypeAnnotation,
} from '@gengjiawen/monkey-ast-types'
```

See `docs/type-system-design.md` in the repository for the full design.

## License

MIT
