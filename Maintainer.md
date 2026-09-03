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

Either re-run the failed `release-please` job from the Actions UI, or run the
workflow manually with `resume_publish` checked. Both re-enter the publish
chain; a plain push, or a dispatch without `resume_publish`, does not, because
`release_created` is only true on the run that cut the release.

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
