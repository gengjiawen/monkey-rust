# Repository Guidelines

## Project Structure & Module Organization
- Rust workspace (`Cargo.toml`) with crates: `lexer/`, `parser/`, `object/`, `interpreter/`, `compiler/`, `wasm/`.
- JS/TS packages under `packages/`: `prettier-plugin-monkey/` (Prettier plugin) and `playground/` (web demo). Workspace managed by `pnpm` (`pnpm-workspace.yaml`).
- Examples in `examples/`; CI in `.github/workflows/`.
- Crates use flat files (no `src/`); primary files live at crate root (for example `parser/lib.rs`, `compiler/vm.rs`). Tests are colocated as `*_test.rs` plus `insta` snapshots in `snapshots/`.

## Build, Test, and Development Commands
- Rust (workspace): `cargo build` and `cargo test` — builds/tests all crates.
- Single crate: `cargo build -p monkey-parser` or `cargo test -p monkey-compiler`.
- Wasm package: `cd wasm && wasm-pack build --release --scope=gengjiawen`.
- JS workspace: `pnpm i` then
  - Playground: `pnpm -C packages/playground dev` (local server) or `pnpm build`.
  - Prettier plugin: `pnpm -C packages/prettier-plugin-monkey test` and `pnpm -C packages/prettier-plugin-monkey build`.

## Coding Style & Naming Conventions
- Rust formatting via `cargo fmt --all` (see `rustfmt.toml`: keep wide struct and fn-call widths). Use 4‑space indent, `snake_case` for functions/vars, `CamelCase` for types.
- JS/TS formatting via Prettier (`.prettierrc`: `singleQuote: true`, `semi: false`). Run `pnpm format` for repo YAML/markdown when applicable.
- Keep tests in `*_test.rs`; snapshot names are short and descriptive.

## Testing Guidelines
- Run all Rust tests with `cargo test` at repo root. Snapshot tests use `insta`.
- To refresh snapshots locally: `INSTA_UPDATE=always cargo test` (review diffs before committing).
- Wasm tests (if used): `cd wasm && wasm-pack test --node`.

## Commit & Pull Request Guidelines
- Follow conventional commit style seen in history: `feat:`, `fix:`, `chore(deps):`, `docs:`, `refactor:`, `test:`. Keep scope clear (e.g., `fix(parser): ...`).
- PRs should include: concise description, affected crates/packages, rationale, and, if UI/behavior changes, examples or screenshots. Ensure CI passes (`cargo build/test`) and update snapshots/docs as needed.
- Do not include version bumps or publish steps in regular PRs (maintainers handle releases).

## Architecture Overview
- Pipeline: `lexer` → `parser` → `object` shared types → `interpreter` (AST eval) and `compiler`/`vm` (bytecode). `wasm` exposes parser/compiler to JS; `packages/` consume the wasm output for formatting and the playground.

