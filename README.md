# monkey-rust
![Rust](https://github.com/gengjiawen/monkey-rust/workflows/Rust/badge.svg)
[![Gitpod ready-to-code](https://img.shields.io/badge/Gitpod-ready--to--code-blue?logo=gitpod)](https://gitpod.io/#https://github.com/gengjiawen/monkey_rust)

An interpreter for the Monkey programming language written in Rust

![The Monkey Programming Language](https://cloud.githubusercontent.com/assets/1013641/22617482/9c60c27c-eb09-11e6-9dfa-b04c7fe498ea.png)

## What’s Monkey?

Monkey has a C-like syntax, supports **variable bindings**, **prefix** and **infix operators**, has **first-class** and **higher-order functions**, can handle **closures** with ease and has **integers**, **booleans**, **arrays** and **hashes** built-in.

Official site is: https://monkeylang.org/. It's has various implementation languages :). 

There is a book about learning how to make an interpreter: [Writing An Interpreter In Go](https://interpreterbook.com/#the-monkey-programming-language). This is where the Monkey programming language come from.

## Instruction

### Build and test

```bash
$ cargo build
$ cargo test
```

### Version and Publish
Version
```bash
cargo install cargo-workspaces
cargo workspaces version custom --exact 0.4.0 --no-git-commit
```

Publish
```bash
cargo workspaces publish --from-git --token $CARGO_TOKEN
```

## References on compiler
* https://github.com/dinfuehr/dora
* https://github.com/wren-lang/wren
* https://github.com/apollographql/federation