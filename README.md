# monkey-rust

![Rust](https://github.com/gengjiawen/monkey-rust/workflows/Rust/badge.svg)
[![Gitpod ready-to-code](https://img.shields.io/badge/Gitpod-ready--to--code-blue?logo=gitpod)](https://gitpod.io/#https://github.com/gengjiawen/monkey_rust)
[![monkey-interpreter](https://img.shields.io/crates/v/monkey-interpreter)](https://crates.io/crates/monkey-interpreter)
[![npm version](https://img.shields.io/npm/v/@gengjiawen/monkey-wasm)](https://www.npmjs.com/package/@gengjiawen/monkey-wasm)

An interpreter for the Monkey programming language written in Rust

![The Monkey Programming Language](https://cloud.githubusercontent.com/assets/1013641/22617482/9c60c27c-eb09-11e6-9dfa-b04c7fe498ea.png)

## What’s Monkey?

Monkey has a C-like syntax, supports **variable bindings**, **prefix** and **infix operators**, has **first-class** and **higher-order functions**, can handle **closures** with ease and has **integers**, **booleans**, **arrays** and **hashes** built-in.

Official site is: https://monkeylang.org/. It's has various implementation languages :).

There is a book about learning how to make an interpreter: [Writing An Interpreter In Go](https://interpreterbook.com/#the-monkey-programming-language). This is where the Monkey programming language come from.

## Features

- Split packages to make everything minimum
- location info for ast
- test for every module
- **Wasm**: A WebAssembly target.

### AST Online playground
https://astexplorer.net/#/gist/e23a81ce309e8fcffe95ddd1b5661061/01d0b4b078304ddd9639eae9f4e6d342e2b9d075

### Compiler playground
https://gengjiawen.github.io/monkey-rust/

## Instruction

### Build and test

```bash
$ cargo build
$ cargo test
```
