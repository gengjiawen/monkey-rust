FROM gengjiawen/node-build
RUN brew install rustup && rustup update
RUN cargo install --git https://github.com/rustwasm/wasm-pack && rustup target add wasm32-unknown-unknown && cargo install cargo-workspaces
RUN envinfo