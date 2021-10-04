FROM gitpod/workspace-wasm:latest

# Install custom tools, runtimes, etc.
# For example "bastet", a command-line tetris clone:
# RUN brew install bastet
#
# More information: https://www.gitpod.io/docs/config-docker/

ENV TRIGGER_REBUILD=1

# fix node.js path and use latest node.js
RUN brew install n && sudo /home/linuxbrew/.linuxbrew/bin/n latest && sudo /usr/local/bin/npm i -g yarn sao
RUN cargo install wasm-pack && rustup target add wasm32-unknown-unknown
ENV PATH=/usr/local/bin/:$PATH
RUN printf "\nexport PATH="/usr/local/bin/:$PATH"\n" >> ~/.bashrc
