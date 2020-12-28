FROM gengjiawen/node-build

# Install custom tools, runtimes, etc.
# For example "bastet", a command-line tetris clone:
# RUN brew install bastet
#
# More information: https://www.gitpod.io/docs/config-docker/

RUN cargo install wasm-pack
RUN rustup target add wasm32-unknown-unknown