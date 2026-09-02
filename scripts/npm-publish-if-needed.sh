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

# `npm view` exits non-zero when the package has never been published, and
# exits zero printing nothing when the package exists but the version does not,
# so the output is the only signal that covers both cases.
published=$(npm view "$name@$version" version 2>/dev/null || true)

if [ "$published" = "$version" ]; then
  echo "$name@$version is already on the registry, skipping"
  exit 0
fi

echo "publishing $name@$version"
npm publish --access public
