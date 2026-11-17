#!/usr/bin/env bash
# Build the checksum-aware adapter from exact sources before replacing a tool.
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
version=$(tr -d '[:space:]' < "$repo/tools/mbx-version")
upstream=1.29.2
commit=7fa83b28d239bd39d1079127ffd2f8b0f92df00a
source_sha=7154a86a1e38900962878ea43051665b5a4f276fc69361b74f73a102d9009189
if [ "$#" != 1 ]; then echo 'usage: tools/mbx/install.sh <destination-directory>' >&2; exit 2; fi
destination=$1
mkdir -p "$destination"
destination=$(cd "$destination" && pwd)
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-gnu; asset=706569239; suffix= ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-gnu; asset=706569160; suffix= ;;
  Darwin-arm64|Darwin-aarch64) target=aarch64-apple-darwin; asset=706569159; suffix= ;;
  MINGW*-x86_64|MSYS*-x86_64) target=x86_64-pc-windows-msvc; asset=706569216; suffix=.exe ;;
  *) echo 'unsupported mbx executor' >&2; exit 1 ;;
esac
digest() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d ' ' -f 1;
  else shasum -a 256 "$1" | cut -d ' ' -f 1; fi
}
verify() {
  if [[ ! "$1" =~ ^[0-9a-f]{64}$ ]] || [ "$(digest "$2")" != "$1" ]; then
    echo "checksum mismatch: $2" >&2
    exit 1
  fi
}
patch_sha=$(digest "$repo/tools/mbx/checksum-cache.patch")
toolchain=$(rustup show active-toolchain | cut -d ' ' -f 1)
identity="$version $commit $source_sha $patch_sha $target $toolchain"
binary="$destination/mbx$suffix"
receipt="$destination/mbx.source"
if [ -f "$binary" ] && [ -f "$receipt" ] &&
   [ "$(sed -n '1p' "$receipt")" = "$identity" ] &&
   [ "$(sed -n '2p' "$receipt")" = "$(digest "$binary")" ] &&
   [ "$("$binary" --version)" = "mbx $version" ]; then
  "$binary" --version
  exit 0
fi
if [ -n "${MBX_INSTALL_WORK:-}" ]; then work=$MBX_INSTALL_WORK; mkdir -p "$work";
else work=$(mktemp -d); trap 'rm -rf "$work"' EXIT; fi
work=$(cd "$work" && pwd)
mkdir -p "$work/bootstrap"
if [ -n "$suffix" ]; then archive="mbx-$target.zip"; else archive="mbx-$target.tar.gz"; fi
download() {
  local filename=$1 asset_id=$2
  if ! curl --retry 3 --retry-delay 2 --retry-max-time 120 -fsSL -o "$work/bootstrap/$filename" \
      "https://github.com/jdx/mr-boxington/releases/download/v$upstream/$filename"; then
    curl --retry 3 --retry-delay 2 --retry-max-time 120 -fsSL -H 'Accept: application/octet-stream' \
      -o "$work/bootstrap/$filename" "https://api.github.com/repos/jdx/mr-boxington/releases/assets/$asset_id"
  fi
}
download "$archive" "$asset"
download SHA256SUMS 706569161
expected=$(awk -v name="$archive" '$2 == name {print $1}' "$work/bootstrap/SHA256SUMS")
verify "$expected" "$work/bootstrap/$archive"
if [ -n "$suffix" ]; then unzip -oq "$work/bootstrap/$archive" -d "$work/bootstrap";
else tar -xzf "$work/bootstrap/$archive" -C "$work/bootstrap"; fi
curl --retry 3 --retry-delay 2 --retry-max-time 120 -fsSL \
  "https://codeload.github.com/jdx/mr-boxington/tar.gz/$commit" -o "$work/source.tar.gz"
verify "$source_sha" "$work/source.tar.gz"
# A tar overlay can retain files from an earlier attempt. Extract into a new
# directory we alone created; shared build outputs stay outside this source.
source=$(mktemp -d "$work/source.XXXXXX")
printf '%s\n' "$source" > "$work/source-path"
tar -xzf "$work/source.tar.gz" --strip-components=1 -C "$source"
patch --batch --forward -F0 -p1 -d "$source" < "$repo/tools/mbx/checksum-cache.patch"
(
  cd "$source"
  # Never change Omega's active toolchain or another build's cache/settings.
  export RUSTUP_TOOLCHAIN="$toolchain" MBX_CACHE_DIR="$work/cache" MBX_TARGET_ROOT="$work/targets"
  export MBX_SHIMS_DIR="$work/cache/shims" MBX_STATS_REPORT="$work/build-stats.json"
  export MBX_BYPASS_LOG="$work/build-bypasses.jsonl"
  export MBX_GC_AUTO=false MBX_REMOTE_MODE=read-only XDG_CONFIG_HOME="$work/config"
  export CARGO_HOME="$work/cargo" CARGO_TARGET_DIR="$work/target"
  export CARGO_INCREMENTAL=0
  export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
  "$work/bootstrap/mbx$suffix" build --locked --release -p mbx < /dev/null
)
built="$work/target/release/mbx$suffix"
built_version=$("$built" --version)
if [ "$built_version" != "mbx $version" ]; then
  echo "built mbx version mismatch: expected $version, received $built_version" >&2
  exit 1
fi
mkdir -p "$destination"
cp "$built" "$binary.new"
chmod +x "$binary.new"
printf '%s\n%s\n' "$identity" "$(digest "$binary.new")" > "$receipt.new"
mv "$binary.new" "$binary"
mv "$receipt.new" "$receipt"
"$binary" --version
