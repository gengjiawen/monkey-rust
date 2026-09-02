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

### Re-running a failed release

The publish chain is idempotent, so re-running the `release-please` workflow
after a failure resumes instead of starting over. `cargo workspaces publish
--from-git` skips crates it already finds on crates.io, `ovsx publish` is called
with `--skip-duplicate`, and every npm package goes through
`scripts/npm-publish-if-needed.sh`, which skips a package whose exact
`name@version` is already on the registry. A re-run therefore publishes only
what is missing and exits 0 when there is nothing left to do.

## Debug CI

docker run -v $PWD:/pwd -w /pwd gengjiawen/node-build bash -c "npx envinfo"
docker run -v $PWD:/pwd -w /pwd -it gengjiawen/node-build fish
docker run -v $PWD:/pwd -w /pwd gengjiawen/node-build bash -c "cd wasm && wasm-pack build --release --scope=gengjiawen"
