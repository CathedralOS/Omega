#!/usr/bin/env bash
# Exercise a real cache hit, then a source change hidden from mtime freshness.
# Usage: bash tools/xtask/check-mbx-checksum.sh /absolute/path/to/mbx NEW_OUTPUT_DIR
set -euo pipefail
if [ "$#" != 2 ]; then
  echo "usage: $0 /absolute/path/to/mbx NEW_OUTPUT_DIR" >&2
  exit 2
fi
mbx=$1
mkdir "$2"
output=$(cd "$2" && pwd -P)
project="$output/project"
mkdir -p "$project/src" "$project/.cargo"
export CARGO_TARGET_DIR="$project/target"
export MBX_CACHE_DIR="$output/store"
case "$(uname -s)" in
  MINGW*)
    export CARGO_TARGET_DIR="$(cygpath -m "$CARGO_TARGET_DIR")"
    export MBX_CACHE_DIR="$(cygpath -m "$MBX_CACHE_DIR")"
    ;;
esac
export MBX_GC_AUTO=false
export CARGO_INCREMENTAL=0
export CARGO_TERM_COLOR=never
export MBX_INCREMENTAL=0
cat > "$project/Cargo.toml" <<'EOF'
[package]
name = "checksum_probe"
version = "0.0.0"
edition = "2024"
[workspace]
EOF
cat > "$project/Cargo.lock" <<'EOF'
version = 4
[[package]]
name = "checksum_probe"
version = "0.0.0"
EOF
cat > "$project/.cargo/config.toml" <<'EOF'
[unstable]
checksum-freshness = true
[build]
fingerprint = "content"
EOF
cat > "$project/src/main.rs" <<'EOF'
include!("value.rs");
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--metadata") {
        let m = std::fs::metadata("src/value.rs").unwrap();
        let time = m.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap();
        println!("{} {}", m.len(), time.as_nanos());
    } else {
        println!("{VALUE}");
    }
}
EOF
printf 'const VALUE: u32 = 1;\n' > "$project/src/value.rs"
cd "$project"
"$mbx" --version > "$output/mbx-version.txt"
rustc -vV > "$output/rustc-version.txt"
"$mbx" build --locked < /dev/null > "$output/prime.log" 2>&1
binary="$CARGO_TARGET_DIR/debug/checksum_probe"
if [ -f "$binary.exe" ]; then binary="$binary.exe"; fi
test "$("$binary")" = 1
cp -p src/value.rs "$output/value-before.rs"
"$binary" --metadata > "$output/metadata-before.txt"
# Remove only this newly created probe's products. Cargo must invoke rustc,
# rather than answering the second build from its own existing products.
rm -rf "$CARGO_TARGET_DIR"
"$mbx" build --locked < /dev/null > "$output/hit.log" 2>&1
test "$("$binary")" = 1
if ! grep -Eq 'mbx\[cache\]: [1-9][0-9]* hits,' "$output/hit.log"; then
  echo "identical inputs did not produce an mbx cache hit; inspect $output/hit.log" >&2
  exit 1
fi
# Keep restored dep-info and products: Cargo must detect the byte change using
# the restored checksum comments, and mbx must not restore the old result.
printf 'const VALUE: u32 = 2;\n' > src/value.rs
touch -r "$output/value-before.rs" src/value.rs
"$binary" --metadata > "$output/metadata-after.txt"
cmp "$output/metadata-before.txt" "$output/metadata-after.txt"
"$mbx" build --locked < /dev/null > "$output/mutation.log" 2>&1
test "$("$binary")" = 2
grep -q 'Compiling checksum_probe' "$output/mutation.log"
printf '%s\n' 'PASS: identical-input cache hit; same-size/same-mtime include changed output 1 -> 2'
