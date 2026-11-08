#!/usr/bin/env bash
# Install the pinned mbx release used by the current Rust validation workflows.
set -euo pipefail
version=1.29.2
case "${RUNNER_OS:-$(uname -s)}-${RUNNER_ARCH:-$(uname -m)}" in
  Linux-X64|Linux-x86_64) target=x86_64-unknown-linux-gnu; asset=706569239 ;;
  Linux-ARM64|Linux-aarch64) target=aarch64-unknown-linux-gnu; asset=706569160 ;;
  macOS-ARM64|Darwin-arm64) target=aarch64-apple-darwin; asset=706569159 ;;
  Windows-X64|MINGW*-x86_64) target=x86_64-pc-windows-msvc; asset=706569216 ;;
  *) echo 'unsupported mbx executor' >&2; exit 1 ;;
esac
install_dir="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/omega-mbx-$version"
mkdir -p "$install_dir"
cd "$install_dir"
if [[ "$target" == *windows* ]]; then archive="mbx-$target.zip"; else archive="mbx-$target.tar.gz"; fi
release="https://github.com/jdx/mr-boxington/releases/download/v$version"
# These asset IDs belong to the same pinned release. The API route avoids
# a cached release-redirect failure; both routes still owe checksum validation.
download() {
  local filename=$1 asset_id=$2
  if ! curl --retry 3 --retry-delay 2 --retry-max-time 120 -fsSL -o "$filename" "$release/$filename"; then
    echo "release download failed; retrying the same pinned asset through the GitHub API" >&2
    curl --retry 3 --retry-delay 2 --retry-max-time 120 -fsSL \
      -H 'Accept: application/octet-stream' -o "$filename" \
      "https://api.github.com/repos/jdx/mr-boxington/releases/assets/$asset_id"
  fi
}
download "$archive" "$asset"
download SHA256SUMS 706569161
if command -v sha256sum >/dev/null; then
  grep " $archive\$" SHA256SUMS | sha256sum --check --strict -
else
  grep " $archive\$" SHA256SUMS | shasum -a 256 --check -
fi
if [[ "$target" == *windows* ]]; then unzip -oq "$archive"; else tar -xzf "$archive"; fi
if [ -n "${GITHUB_PATH:-}" ]; then printf '%s\n' "$install_dir" >> "$GITHUB_PATH"; fi
"$install_dir/mbx" --version
