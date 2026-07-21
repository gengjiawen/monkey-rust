# `@gengjiawen/monkey-minifier`

A source-to-source minifier for the Monkey language in this repository. It uses
the Rust/Wasm parser, preserves the full signed 64-bit integer range, and emits
compact Monkey source.

```ts
import { minify } from '@gengjiawen/monkey-minifier'

const { code } = minify('let longName = 40 + 2; longName;')
// let a=42;a;
```

`mangle` and `fold` default to `true`. Disable either pass when only compact
printing is wanted:

```ts
minify(source, { mangle: false, fold: false })
```

Browser bundlers use the package's browser entry. The Node API and CLI require
Node 24 or newer and load the same Wasm parser through Node's WebAssembly
API.

The CLI reads files or stdin:

```sh
monkey-minify input.monkey
monkey-minify --no-mangle < input.monkey
```
