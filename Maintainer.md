## Version and Publish

Now automated with release-please

Version

```bash
cargo install cargo-workspaces
cargo workspaces version custom 0.7.0 --no-git-commit
```

Publish

```bash
cargo workspaces publish --from-git --token $CARGO_TOKEN
```

## Debug CI

docker run -v $PWD:/pwd -w /pwd gengjiawen/node-build bash -c "npx envinfo"
docker run -v $PWD:/pwd -w /pwd -it gengjiawen/node-build fish
docker run -v $PWD:/pwd -w /pwd gengjiawen/node-build bash -c "cd wasm && wasm-pack build --release --scope=gengjiawen"
