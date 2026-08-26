#!/usr/bin/env sh
# Focused regression for contextual state identifiers in the Delta lowermachine.
# Rust, the native lowermachine, and the self-built lowermachine must publish
# byte-identical assembly and the three artifacts must traverse every state.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
SAMPLES="$HERE/../samples"
BIN="$HERE/target/debug/delta"
SOURCE="$SAMPLES/contextual-state-identifiers.alp"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "lowermachine contextual states: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for tool in cargo clang codesign cmp; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "lowermachine contextual states: skipped ($tool absent)"
    exit 0
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT HUP INT TERM

cd "$HERE"
cargo build --quiet
DELTA_ARCH=aarch64 "$BIN" "$SAMPLES/lowermachine.alp" "$T/lowermachine.native" >/dev/null
"$T/lowermachine.native" < "$SAMPLES/lowermachine.alp" > "$T/lowermachine.self.s"
clang -arch arm64 -o "$T/lowermachine.self" "$T/lowermachine.self.s"
codesign -f -s - "$T/lowermachine.self" >/dev/null 2>&1

DELTA_ARCH=aarch64 "$BIN" "$SOURCE" "$T/reference" >/dev/null
"$T/lowermachine.native" < "$SOURCE" > "$T/native.s"
"$T/lowermachine.self" < "$SOURCE" > "$T/self.s"
cmp "$T/reference.s" "$T/native.s" >/dev/null
cmp "$T/native.s" "$T/self.s" >/dev/null

clang -arch arm64 -o "$T/native" "$T/native.s"
clang -arch arm64 -o "$T/self" "$T/self.s"
codesign -f -s - "$T/native" >/dev/null 2>&1
codesign -f -s - "$T/self" >/dev/null 2>&1

set +e
"$T/reference" >/dev/null 2>&1
reference_status=$?
"$T/native" >/dev/null 2>&1
native_status=$?
"$T/self" >/dev/null 2>&1
self_status=$?
set -e
if [ "$reference_status" -ne 37 ] || [ "$native_status" -ne 37 ] || [ "$self_status" -ne 37 ]; then
  echo "lowermachine contextual states FAIL — reference=$reference_status native=$native_status self=$self_status, expected 37"
  exit 1
fi

echo "LOWERMACHINE CONTEXTUAL STATES ✓ — 12 contextual names preserve exact native/self assembly and runtime control flow"
