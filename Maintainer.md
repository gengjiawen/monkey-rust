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

Run the `release-please` workflow manually with `resume_publish` checked. That
is the only way back into the publish chain besides the run that cut the
release — re-running the failed job from the Actions UI does not do it, and
neither does a dispatch with the box unchecked, because `release_created` is
only true on the run whose push merged the release PR. The ask is deliberate:
any rerun entering the chain on its own would publish the versions in the tree
without a release behind them.

The chain then publishes what is missing and skips what is already out there:
`cargo workspaces publish --from-git` reports `already published` for crates it
finds on crates.io, `ovsx publish` runs with `--skip-duplicate`, and every npm
package goes through `scripts/npm-publish-if-needed.sh`, which skips a package
whose exact `name@version` the registry already has. A resume with nothing left
to do exits 0.

If the registry cannot be reached, `npm-publish-if-needed.sh` fails the release
rather than guessing — "not published" would walk into the
`EPUBLISHCONFLICT` that stops the chain in the first place. Re-run once the
registry answers again.

## Debug CI

docker run -v $PWD:/pwd -w /pwd gengjiawen/node-build bash -c "npx envinfo"
docker run -v $PWD:/pwd -w /pwd -it gengjiawen/node-build fish
docker run -v $PWD:/pwd -w /pwd gengjiawen/node-build bash -c "cd wasm && wasm-pack build --release --scope=gengjiawen"
