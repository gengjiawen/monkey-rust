FROM gengjiawen/node-build

# Install custom tools, runtimes, etc.
# For example "bastet", a command-line tetris clone:
# RUN brew install bastet
#
# More information: https://www.gitpod.io/docs/config-docker/

USER gitpod

RUN rustup target add wasm32-unknown-unknown
RUN cargo install wasm-pack
