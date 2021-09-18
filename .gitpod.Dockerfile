FROM gitpod/workspace-wasm:latest

# Install custom tools, runtimes, etc.
# For example "bastet", a command-line tetris clone:
# RUN brew install bastet
#
# More information: https://www.gitpod.io/docs/config-docker/

# fix node.js path and use latest node.js
RUN brew install n && sudo /home/linuxbrew/.linuxbrew/bin/n latest
ENV PATH=/usr/local/bin/:$PATH
