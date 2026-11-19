#!/usr/bin/env bash
# Install the qualified checksum-capable mbx built from immutable upstream source.
set -euo pipefail
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
version=$(tr -d '[:space:]' < "$repo/tools/mbx-version")
patch="$repo/tools/xtask/mbx-checksum.patch"
# The release supplies only the verified bootstrap compiler wrapper. The
# installed tool comes from this exact commit with the repository's patch.
upstream_version=1.29.2
upstream_revision=7fa83b28d239bd39d1079127ffd2f8b0f92df00a
# Avoid upstream's expensive fat LTO for this host tool. Tests and delivery use
# this same optimized profile; its identity participates in the artifact key.
build_recipe='release;opt=3;debug=0;lto=false;codegen-units=16;panic=unwind;strip=symbols;windows-bundled-lld;content-fingerprints;build-script-execution=false'
if command -v sha256sum >/dev/null; then hash=(sha256sum); else hash=(shasum -a 256); fi
hash_file() { "${hash[@]}" "$1" | awk '{print $1}'; }
patch_digest=$(hash_file "$patch")
toolchain_digest=$(hash_file "$repo/rust-toolchain.toml")
installer_digest=$(hash_file "$repo/tools/xtask/bootstrap-mbx.sh")
recipe_unchanged() {
  if [ "$(hash_file "$patch")" != "$patch_digest" ] ||
     [ "$(hash_file "$repo/rust-toolchain.toml")" != "$toolchain_digest" ] ||
     [ "$(hash_file "$repo/tools/xtask/bootstrap-mbx.sh")" != "$installer_digest" ] ||
     [ "$(tr -d '[:space:]' < "$repo/tools/mbx-version")" != "$version" ]; then
    echo 'mbx setup inputs changed while installing; rerun setup' >&2
    return 1
  fi
}
static_key() {
  printf '%s\n' "$version" "$upstream_revision" "$build_recipe" \
    "$patch_digest" "$toolchain_digest" "$installer_digest" |
    "${hash[@]}" | awk '{print $1}'
}
install_dir="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/omega-mbx-$version"
case "$#:${1:-}" in
  0:) ;;
  1:--cache-key) static_key; exit 0 ;;
  2:--install-dir) install_dir=$2 ;;
  *) echo "usage: $0 [--install-dir DIRECTORY | --cache-key]" >&2; exit 2 ;;
esac
case "${RUNNER_OS:-$(uname -s)}-${RUNNER_ARCH:-$(uname -m)}" in
  Linux-X64|Linux-x86_64) target=x86_64-unknown-linux-gnu; asset=706569239 ;;
  Linux-ARM64|Linux-aarch64) target=aarch64-unknown-linux-gnu; asset=706569160 ;;
  macOS-ARM64|Darwin-arm64|Darwin-aarch64) target=aarch64-apple-darwin; asset=706569159 ;;
  Windows-X64|MINGW*-x86_64) target=x86_64-pc-windows-msvc; asset=706569216 ;;
  *) echo 'unsupported mbx executor' >&2; exit 1 ;;
esac
exe=
if [[ "$target" == *windows* ]]; then exe=.exe; fi
# Hosted jobs give Actions and this installer the same absolute cache path.
cache="${OMEGA_MBX_INSTALL_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/omega-mbx}"
if [ -n "$exe" ]; then
  cache=$(cygpath -u "$cache")
  install_dir=$(cygpath -u "$install_dir")
fi
# Build the host tool with the repository's pinned compiler, or the caller's
# explicit selection. A restored artifact needs no download when it is present.
mbx_toolchain=${RUSTUP_TOOLCHAIN:-$(sed -n 's/^channel = "\(.*\)"/\1/p' "$repo/rust-toolchain.toml")}
[ -n "$mbx_toolchain" ] || { echo 'cannot resolve the Rust toolchain for mbx' >&2; exit 1; }
if ! compiler=$(RUSTUP_TOOLCHAIN="$mbx_toolchain" rustc -vV 2>/dev/null); then
  rustup toolchain install "$mbx_toolchain" --profile minimal
  compiler=$(RUSTUP_TOOLCHAIN="$mbx_toolchain" rustc -vV)
fi
host=$(printf '%s\n' "$compiler" | sed -n 's/^host: //p')
if [ "$host" != "$target" ]; then
  echo "mbx executor $target does not match the active Rust host $host" >&2
  exit 1
fi
flags_key=$(printf '%s\n' "${RUSTFLAGS:-}" "${CARGO_ENCODED_RUSTFLAGS:-}" | "${hash[@]}" | awk '{print $1}')
work_key=$(printf '%s\n' "$upstream_revision" "$build_recipe" "$compiler" "$flags_key" | "${hash[@]}" | awk '{print $1}')
key=$(printf '%s\n' "$(static_key)" "$work_key" | "${hash[@]}" | awk '{print $1}')
artifact="$cache/artifacts/$key"
binary="mbx$exe"
valid_artifact() {
  [ -f "$artifact/$binary" ] && [ -f "$artifact/SHA256SUMS" ] &&
    (cd "$artifact" && "${hash[@]}" --check SHA256SUMS >/dev/null 2>&1) &&
    [ "$("$artifact/$binary" --version)" = "mbx $version" ]
}
lock=
scratch=
install_stage=
cleanup() {
  if [ -n "$install_stage" ]; then rm -rf "$install_stage"; fi
  if [ -n "$scratch" ]; then rm -rf "$scratch"; fi
  if [ -n "$lock" ]; then rm -rf "$lock"; fi
}
trap cleanup EXIT
if ! valid_artifact; then
  mkdir -p "$cache/artifacts" "$cache/work"
  lock="$cache/work/$work_key.lock"
  if ! mkdir "$lock" 2>/dev/null; then
    echo "mbx tool build already reserved at $lock; let its installer finish before retrying" >&2
    lock=
    exit 1
  fi
  # Another completed installer may have published between the first check
  # and acquiring the shared build workspace.
  if ! valid_artifact; then
    scratch=$(mktemp -d "$cache/.download.XXXXXX")
    cp "$patch" "$scratch/mbx-checksum.patch"
    [ "$(hash_file "$scratch/mbx-checksum.patch")" = "$patch_digest" ]
    recipe_unchanged
    release="https://github.com/jdx/mr-boxington/releases/download/v$upstream_version"
    if [ -n "$exe" ]; then archive="mbx-$target.zip"; else archive="mbx-$target.tar.gz"; fi
    download() {
      local filename=$1 asset_id=$2
      if ! curl --retry 3 --retry-delay 2 --retry-max-time 120 -fsSL -o "$scratch/$filename" "$release/$filename"; then
        echo 'release download failed; retrying the same pinned asset through the GitHub API' >&2
        curl --retry 3 --retry-delay 2 --retry-max-time 120 -fsSL \
          -H 'Accept: application/octet-stream' -o "$scratch/$filename" \
          "https://api.github.com/repos/jdx/mr-boxington/releases/assets/$asset_id"
      fi
    }
    download "$archive" "$asset"
    download SHA256SUMS 706569161
    (cd "$scratch" && grep " $archive\$" SHA256SUMS | "${hash[@]}" --check -)
    if [ -n "$exe" ]; then (cd "$scratch" && unzip -oq "$archive"); else tar -xzf "$scratch/$archive" -C "$scratch"; fi
    bootstrap="$scratch/$binary"
    [ "$("$bootstrap" --version)" = "mbx $upstream_version" ]
    work="$cache/work/$work_key"
    source="$work/source"
    build_target="$work/target"
    mkdir -p "$work"
    if [ ! -d "$source/.git" ]; then
      git clone --config core.autocrlf=false --quiet --depth 1 --branch "v$upstream_version" https://github.com/jdx/mr-boxington.git "$source"
    fi
    [ "$(git -C "$source" rev-parse HEAD)" = "$upstream_revision" ] || {
      echo 'cached mbx source does not match its immutable upstream revision' >&2; exit 1;
    }
    # Preserve the pinned source bytes independently of the caller's checkout
    # newline policy, including Git for Windows' usual CRLF conversion.
    git -C "$source" config core.autocrlf false
    # This is the installer's generated source workspace, protected by its
    # build lock. Targets live beside it, so patch updates reuse dependencies.
    git -C "$source" reset --hard "$upstream_revision" >/dev/null
    git -C "$source" clean -fdx >/dev/null
    git -C "$source" apply --check "$scratch/mbx-checksum.patch"
    git -C "$source" apply "$scratch/mbx-checksum.patch"
    printf '%s\n' "$compiler" > "$work/compiler.txt"
    cargo_target="$build_target"
    if [ -n "$exe" ]; then cargo_target=$(cygpath -m "$cargo_target"); fi
    build_config=(--config 'profile.release.opt-level=3' --config 'profile.release.debug=0'
      --config 'profile.release.lto=false' --config 'profile.release.codegen-units=16'
      --config 'profile.release.panic="unwind"' --config 'profile.release.strip="symbols"')
    if [ -n "$exe" ]; then
      build_config+=(--config 'target.x86_64-pc-windows-msvc.rustflags=["-C","linker-flavor=lld-link","-C","linker=rust-lld"]')
    fi
    echo "Building mbx $version once for $target ($key)" >&2
    printf 'mbx build workspace: %s\n' "$work" >&2
    (cd "$source" && RUSTUP_TOOLCHAIN="$mbx_toolchain" CARGO_TARGET_DIR="$cargo_target" \
      CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true CARGO_BUILD_FINGERPRINT=content MBX_GC_AUTO=false MBX_BUILD_SCRIPT_EXECUTION=false \
      "$bootstrap" build --locked --release --package mbx --target "$target" \
        "${build_config[@]}" < /dev/null)
    staged="$scratch/artifact"
    mkdir "$staged"
    cp "$build_target/$target/release/$binary" "$staged/$binary"
    [ "$("$staged/$binary" --version)" = "mbx $version" ]
    (cd "$staged" && "${hash[@]}" "$binary" > SHA256SUMS)
    printf '%s\n' "$upstream_revision" "$patch_digest" "$toolchain_digest" "$installer_digest" "$build_recipe" "$compiler" "$flags_key" > "$staged/inputs.txt"
    recipe_unchanged
    # Publish a complete artifact; never let a partial build look installed.
    rm -rf "$artifact"
    mv "$staged" "$artifact"
  fi
else
  echo "Reusing verified mbx $version ($key)" >&2
fi
recipe_unchanged
mkdir -p "$install_dir"
install_stage=$(mktemp -d "$install_dir/.mbx-install.XXXXXX")
cp "$artifact/$binary" "$install_stage/$binary"
chmod +x "$install_stage/$binary"
(cd "$install_stage" && "${hash[@]}" --check "$artifact/SHA256SUMS" >/dev/null)
[ "$("$install_stage/$binary" --version)" = "mbx $version" ]
mv -f "$install_stage/$binary" "$install_dir/$binary"
if [ -n "${GITHUB_PATH:-}" ]; then printf '%s\n' "$install_dir" >> "$GITHUB_PATH"; fi
"$install_dir/$binary" --version
