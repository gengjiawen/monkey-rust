# Monkey Language VS Code Extension

Provides syntax highlighting, lightweight diagnostics using the Monkey WASM parser, and a couple of convenience commands.

## Features

- Syntax highlighting for `.monkey` files
- Diagnostics via `@gengjiawen/monkey-wasm`
- Commands:
  - "Monkey: Compile To Bytecode" - compile current file and show bytecode
  - "Monkey: Show AST (JSON)" - parse and view AST JSON

## Development

Install dependencies and build the extension:

```
pnpm i
pnpm --filter monkey-extension run build
```

Package a VSIX:

```
pnpm --filter monkey-extension run package
```

The extension depends on the published `@gengjiawen/monkey-wasm` package so VSIX packaging can include runtime dependencies without relying on the ignored `wasm/pkg` workspace output.

## Settings

- `monkey.enableWasmDiagnostics` (default: true)

## Notes

- The diagnostics currently use error messages thrown by the wasm parser and mark the first line. We can extend the wasm API to return structured spans for precise ranges later.
