#!/usr/bin/env bash
set -euo pipefail

curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
. "$HOME/.cargo/env"
rustup target add wasm32-unknown-unknown

curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

(
  cd wasm
  wasm-pack build --release --scope=gengjiawen
)

pnpm install --frozen-lockfile
