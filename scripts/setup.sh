#!/usr/bin/env bash
# Install the pinned Typst version into .tools/ (gitignored).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=0.13.1
SHA256=7d214bfeffc2e585dc422d1a09d2b144969421281e8c7f5d784b65fc69b5673f
TARGET=typst-x86_64-unknown-linux-musl

if [ -x .tools/typst ] && .tools/typst --version | grep -q "typst $VERSION "; then
  echo "typst $VERSION already installed"
  exit 0
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -sSfL -o "$tmp/typst.tar.xz" \
  "https://github.com/typst/typst/releases/download/v$VERSION/$TARGET.tar.xz"
echo "$SHA256  $tmp/typst.tar.xz" | sha256sum -c --quiet
tar -xf "$tmp/typst.tar.xz" -C "$tmp"
mkdir -p .tools
install -m 755 "$tmp/$TARGET/typst" .tools/typst
.tools/typst --version
