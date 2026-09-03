#!/usr/bin/env bash
# Publish an npm package unless that exact name@version is already on the
# registry.
#
# `npm publish` fails with EPUBLISHCONFLICT when the version already exists, so
# a release that dies partway through the publish chain cannot simply be
# re-run: the retry fails on the first package instead of resuming at the one
# that failed, and the rest of the release has to be finished by hand.
# `cargo workspaces publish --from-git` already skips crates it finds on
# crates.io; this gives the npm half of the chain the same property.
#
# Usage: scripts/npm-publish-if-needed.sh <package-directory>
set -euo pipefail

directory="${1:?usage: npm-publish-if-needed.sh <package-directory>}"
cd "$directory"

if [ ! -f package.json ]; then
  echo "no package.json in $directory" >&2
  exit 1
fi

read_field() {
  node -p "JSON.parse(require('fs').readFileSync('package.json', 'utf8')).$1 ?? ''"
}

name=$(read_field name)
version=$(read_field version)

if [ -z "$name" ] || [ -z "$version" ]; then
  echo "$directory/package.json is missing a name or a version" >&2
  exit 1
fi

# The published version list, or `[]` when the package has never been
# published. A registry that cannot be reached is a third answer, and it must
# not be read as either of the first two: treating a timeout as "not published"
# walks into the EPUBLISHCONFLICT this script exists to avoid, and treating it
# as "published" skips a package that is genuinely missing. So only npm's own
# "no such package" answer means the package is new; anything else stops the
# release with the registry's error still visible.
errors=$(mktemp)
trap 'rm -f "$errors"' EXIT

if versions=$(npm view "$name" versions --json 2>"$errors"); then
  :
elif grep -q 'E404' "$errors"; then
  versions='[]'
else
  cat "$errors" >&2
  echo "could not ask the registry about $name; not publishing $version" >&2
  exit 1
fi

# `--json` returns a bare string, not a list, for a package with one version.
if node -e '
  const [versions, wanted] = process.argv.slice(1)
  const parsed = JSON.parse(versions)
  const published = Array.isArray(parsed) ? parsed : [parsed]
  process.exit(published.includes(wanted) ? 0 : 1)
' "$versions" "$version"; then
  echo "$name@$version is already on the registry, skipping"
  exit 0
fi

echo "publishing $name@$version"
npm publish --access public
