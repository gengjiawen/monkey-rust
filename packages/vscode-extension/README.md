# Monkey Language VS Code Extension

First-class VS Code support for Monkey source files, including syntax highlighting, snippets, WASM-powered diagnostics, AST preview, and bytecode compilation commands.

## Features

- **Editor support for `.monkey` files**: language registration, TextMate syntax highlighting, bracket/comment behavior, and snippets for common Monkey constructs.
- **WASM-powered diagnostics**: parses the current document through `@gengjiawen/monkey-wasm` and reports parser errors directly in the editor.
- **AST preview**: run "Monkey: Show AST (JSON)" to inspect the parser output for the active file.
- **Bytecode compilation**: run "Monkey: Compile To Bytecode" to compile the active file and inspect the compiler output.

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
