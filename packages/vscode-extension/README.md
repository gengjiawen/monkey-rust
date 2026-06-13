# Monkey Language VS Code Extension

First-class VS Code support for Monkey source files.

### Compiler playground
https://monkey-lang-playground-jw.vercel.app/

Repo: https://github.com/gengjiawen/monkey-rust

## Features

- **Syntax highlighting** for `.monkey` files.
- **Language-aware editing** with bracket matching, comment toggling, and auto-closing pairs.
- **Snippets** for common Monkey constructs such as `let`, `fn`, and `if`.
- **WASM-powered diagnostics** that report parser errors while editing.
- **AST preview** for inspecting the parsed JSON tree of the active file.
- **Bytecode compilation** for viewing compiler output from the active file.

## Commands

- **Monkey: Show AST (JSON)**: parse the active Monkey file and open the AST as JSON.
- **Monkey: Compile To Bytecode**: compile the active Monkey file and open the bytecode output.

## Configuration

- `monkey.enableWasmDiagnostics` (default: true)
