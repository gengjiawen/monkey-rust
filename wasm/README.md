![Rust](https://github.com/gengjiawen/monkey-rust/workflows/Rust/badge.svg)

This lib designed for compiling monkey-parser into WebAssembly and
publishing the resulting package to NPM.

![The Monkey Programming Language](https://cloud.githubusercontent.com/assets/1013641/22617482/9c60c27c-eb09-11e6-9dfa-b04c7fe498ea.png)

## What’s Monkey?

### Compiler playground
https://monkey-lang-playground-jw.vercel.app/

Repo: https://github.com/gengjiawen/monkey-rust

Monkey has a C-like syntax, supports **variable bindings**, **prefix** and **infix operators**, has **first-class** and **higher-order functions**, can handle **closures** with ease and has **integers**, **booleans**, **arrays** and **hashes** built-in.

## Features

- Split packages to make everything minimum
- **REPL**: A Read-Eval-Print-Loop (REPL) for Monkey tokenizer, parser, evaluator, compiler
- location info for ast
- test for every module
- **Wasm**: A WebAssembly target, thus run monkey on browser is directly supported.
- **Prettier Plugin**: Format Monkey source code with [Prettier](https://prettier.io/) via `prettier-plugin-monkey`.
- **VS Code Extension**: First-class editor support with syntax highlighting, snippets, WASM-powered diagnostics, AST preview, and bytecode compilation commands.
- bytecode viewer from source
