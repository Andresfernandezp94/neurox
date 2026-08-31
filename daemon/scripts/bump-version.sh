#!/usr/bin/env bash
# Bump version across all crates + commit + tag.
# Usage: ./scripts/bump-version.sh 0.2.0
set -euo pipefail

NEW_VERSION="${1:-}"
if [[ -z "$NEW_VERSION" ]]; then
  err "usage: $0 <new-version> (e.g. 0.2.0)"
fi

if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  err "version must be semver: MAJOR.MINOR.PATCH (got: $NEW_VERSION)"
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
log() { printf "${GREEN}[+]${NC} %s\n" "$*"; }
err() { printf "${RED}[x]${NC} %s\n" "$*"; exit 1; }

# Update workspace.package.version in root Cargo.toml
log "updating root Cargo.toml -> $NEW_VERSION"
sed -i.bak -E "s|^version = \".*\"|version = \"$NEW_VERSION\"|" Cargo.toml
rm -f Cargo.toml.bak

# Update version in each workspace crate's Cargo.toml.
# The list mirrors `[workspace] members` in the root Cargo.toml — keep in sync.
  if [[ -f "$crate/Cargo.toml" ]]; then
    sed -i.bak -E "s|^version = \".*\"|version = \"$NEW_VERSION\"|" "$crate/Cargo.toml"
    rm -f "$crate/Cargo.toml.bak"
  fi
done

# Verify
grep "^version" Cargo.toml

# Git commit + tag
log "creating commit + tag v$NEW_VERSION"
git add -A
git commit -m "chore(release): v$NEW_VERSION"
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"

log "done ✓"
log "next: git push origin main --follow-tags"
log "(the release workflow will build binaries and create a GitHub Release)"
