#!/usr/bin/env bash
set -euo pipefail

# Simple version bump and tag script
# Usage: scripts/release.sh <level>
# level: patch | minor | major | <explicit-version>

fail() { echo "Error: $*" >&2; exit 1; }

level=${1:-}
[[ -n "$level" ]] || fail "release level required (patch|minor|major|<version>)"

# Read current version from Cargo.toml
cur=$(grep -E '^version\s*=\s*"[0-9]+\.[0-9]+\.[0-9]+"' Cargo.toml | sed -E 's/.*"([0-9]+\.[0-9]+\.[0-9]+)".*/\1/')
[[ -n "$cur" ]] || fail "could not find version in Cargo.toml"
IFS='.' read -r MAJ MIN PAT <<< "$cur"

case "$level" in
  patch)
    new="$MAJ.$MIN.$((PAT+1))" ;;
  minor)
    new="$MAJ.$((MIN+1)).0" ;;
  major)
    new="$((MAJ+1)).0.0" ;;
  *)
    new="$level" ;;
 esac

# Update Cargo.toml version
sed -i -E "s/^version\s*=\s*\"[0-9]+\.[0-9]+\.[0-9]+\"/version = \"$new\"/" Cargo.toml

echo "Bumped version: $cur -> $new"

git add Cargo.toml
git commit -m "chore(release): v$new"
git tag -a "v$new" -m "Release v$new"

echo "Created tag v$new. Push with:\n  git push origin dev --tags"
