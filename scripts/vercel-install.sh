#!/usr/bin/env bash
set -euo pipefail

if [ -f /rust/env ]; then
  . /rust/env
fi

if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
  export RUSTUP_INIT_SKIP_PATH_CHECK=yes
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown
fi

corepack enable
corepack prepare pnpm@11.5.1 --activate
npx --yes pnpm@11.5.1 --version

curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

(
  cd wasm
  wasm-pack build --release --scope=gengjiawen
)

npx --yes pnpm@11.5.1 install --frozen-lockfile
