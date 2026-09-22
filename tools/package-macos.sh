#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
target="${1:?Rust target required}"
arch="${2:?Architecture required}"
case "$target:$arch" in
  aarch64-apple-darwin:arm64|x86_64-apple-darwin:x64) ;;
  *) echo 'Unsupported target/architecture pair' >&2; exit 1 ;;
esac
version="$(node -p 'require("./package.json").version')"
name="AirCue-$version-macos-$arch"
bundle="target/$target/release/bundle"
mkdir -p dist
ditto -c -k --sequesterRsrc --keepParent "$bundle/macos/AirCue.app" "dist/$name.zip"
dmgs=("$bundle"/dmg/*.dmg)
[ "${#dmgs[@]}" -eq 1 ] && [ -f "${dmgs[0]}" ]
cp "${dmgs[0]}" "dist/$name.dmg"

# The archive contains exactly the committed source built by this workflow.
source_dir="dist/$name-source"
mkdir -p "$source_dir/.cargo"
git archive HEAD | tar -x -C "$source_dir"
(
  cd "$source_dir"
  cargo vendor --locked vendor > .cargo/config.toml
)
tar -czf "dist/$name-source.tar.gz" -C dist "$name-source"
(
  cd dist
  shasum -a 256 "$name.zip" "$name.dmg" "$name-source.tar.gz" > "SHA256SUMS-macos-$arch.txt"
)
