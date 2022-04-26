FROM gitpod/workspace-full

ENV TRIGGER_REBUILD=6

RUN bash -cl "cargo install cargo-wasm cargo-generate \
    && curl -fsSL https://wasmtime.dev/install.sh  | bash; \
       rustup target add wasm32-wasi"

RUN mkdir /tmp/wasm-sdk \
    && cd /tmp/wasm-sdk \
    && wget "https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-14/wasi-sdk_14.0_amd64.deb" \
    && sudo dpkg -i wasi-sdk_14.0_amd64.deb \
    && rm -rf /tmp/wasi-sdk

RUN git clone --depth 1 "https://github.com/emscripten-core/emsdk.git" $HOME/.emsdk \
    && cd $HOME/.emsdk \
    && ./emsdk install latest \
    && ./emsdk activate latest \
    && printf "\nsource $HOME/.emsdk/emsdk_env.sh\nclear\n" >> ~/.bashrc

RUN brew install binaryen wabt wasm-pack && rustup target add wasm32-unknown-unknown

# fix Node.js path and use latest Node.js
RUN brew install n && sudo /home/linuxbrew/.linuxbrew/bin/n latest && sudo /usr/local/bin/npm i -g yarn sao
ENV PATH=/usr/local/bin/:$PATH
RUN printf "\nexport PATH="/usr/local/bin/:$PATH"\n" >> ~/.bashrc
