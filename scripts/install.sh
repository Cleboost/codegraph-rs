#!/bin/sh
# codegraph install script
# Usage: curl -fsSL https://raw.githubusercontent.com/cleboost/codegraph/main/scripts/install.sh | sh

set -eu

REPO="cleboost/codegraph"
BIN_NAME="codegraph"

uname_s="$(uname -s | tr '[:upper:]' '[:lower:]')"
uname_m="$(uname -m)"

case "$uname_s/$uname_m" in
  linux/x86_64)   target="x86_64-unknown-linux-musl" ;;
  linux/aarch64)  target="aarch64-unknown-linux-gnu" ;;
  darwin/x86_64)  target="x86_64-apple-darwin" ;;
  darwin/arm64)   target="aarch64-apple-darwin" ;;
  *) echo "unsupported platform: $uname_s/$uname_m" >&2; exit 1 ;;
esac

tag="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep -m1 tag_name | sed -E 's/.*"([^"]+)".*/\1/')"
[ -n "$tag" ] || { echo "could not detect latest tag" >&2; exit 1; }

url="https://github.com/$REPO/releases/download/$tag/codegraph-$target.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $url"
curl -fsSL "$url" -o "$tmp/cg.tar.gz"
tar -xzf "$tmp/cg.tar.gz" -C "$tmp"

echo "Running self-installer..."
"$tmp/$BIN_NAME" install
