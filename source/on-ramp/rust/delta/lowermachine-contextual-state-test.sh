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

# Once `->` establishes a transition-target relation, only lexical trivia and
# then an identifier are admitted. String, punctuation, and EOF forms reject in
# phase 1, before the assembly header is published, in both compiler fixed points.
check_malformed_target() {
  case_name=$1
  case_source=$2
  printf '%s' "$case_source" > "$T/$case_name.alp"
  set +e
  "$T/lowermachine.native" < "$T/$case_name.alp" > "$T/$case_name.native.s" 2>/dev/null
  malformed_native=$?
  "$T/lowermachine.self" < "$T/$case_name.alp" > "$T/$case_name.self.s" 2>/dev/null
  malformed_self=$?
  set -e
  if [ "$malformed_native" -ne 1 ] || [ "$malformed_self" -ne 1 ] \
      || [ -s "$T/$case_name.native.s" ] || [ -s "$T/$case_name.self.s" ]; then
    echo "lowermachine malformed contextual target FAIL — $case_name native=$malformed_native self=$malformed_self, expected 1/1 with no output"
    exit 1
  fi
}

SOURCE_PREFIX='boundary trait Console { machine exit_process(return_code: i32); } data Main { console: Console; } machine Main::main(&mut self) { transition 0 { _ -> '
check_malformed_target target-string "${SOURCE_PREFIX}\"read_byte\" } }"
check_malformed_target target-punctuation "${SOURCE_PREFIX}) } }"
check_malformed_target target-eof "$SOURCE_PREFIX"

echo "LOWERMACHINE CONTEXTUAL STATES ✓ — 12 contextual names preserve exact native/self assembly and runtime control flow"
