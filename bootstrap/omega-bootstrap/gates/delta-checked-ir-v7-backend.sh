#!/usr/bin/env sh
# CKIR7 LogicalAnd/LogicalOr backend gate. CKIR4/5/6 parity remains adjacent.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v7 backend: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v7 backend: skipped ($TOOL absent)"; exit 0;
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v7-backend-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v7_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend.native" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERMACHINE" "$T/lowermachine" >/dev/null
"$T/lowermachine" < "$BACKEND" > "$T/backend.s"
clang -arch arm64 -o "$T/backend.self" "$T/backend.s"
codesign -f -s - "$T/backend.self" >/dev/null 2>&1

python3 -B "$FIXTURE" emit "$T/cases"
TAB=$(printf '\t')
POSITIVE=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  CKIR="$T/cases/$NAME.ckir7"
  python3 -B "$FIXTURE" check-ir "$CKIR"
  for IMPLEMENTATION in native self; do
    "$T/backend.$IMPLEMENTATION" < "$CKIR" > "$T/$NAME.$IMPLEMENTATION.elf"
  done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf" || {
    echo "checked-IR-v7 backend: $NAME native/self artifact mismatch" >&2; exit 1;
  }
  case "$NAME" in
    and-*) python3 -B "$FIXTURE" check-artifact-and "$T/$NAME.native.elf" ;;
    or-*) python3 -B "$FIXTURE" check-artifact-or "$T/$NAME.native.elf" ;;
  esac
done < "$T/cases/positives.tsv"

python3 - "$T/and-11.native.elf" "$T/or-11.native.elf" "$T/bad-and.elf" "$T/bad-or.elf" <<'PY'
from pathlib import Path
import sys
for source, output, opcode in ((sys.argv[1], sys.argv[3], b"\x23\x85"),
                               (sys.argv[2], sys.argv[4], b"\x0b\x85")):
    raw = bytearray(Path(source).read_bytes())
    at = raw.index(opcode)
    raw[at] ^= 1
    Path(output).write_bytes(raw)
PY
if python3 -B "$FIXTURE" check-artifact-and "$T/bad-and.elf" >/dev/null 2>&1; then
  echo "checked-IR-v7 backend: mutated AND opcode accepted" >&2; exit 1
fi
if python3 -B "$FIXTURE" check-artifact-or "$T/bad-or.elf" >/dev/null 2>&1; then
  echo "checked-IR-v7 backend: mutated OR opcode accepted" >&2; exit 1
fi

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  for IMPLEMENTATION in native self; do
    set +e
    "$T/backend.$IMPLEMENTATION" < "$T/cases/$NAME.ckir7" > "$T/$NAME.$IMPLEMENTATION"
    ACTUAL=$?
    set -e
    [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.$IMPLEMENTATION" ] || {
      echo "checked-IR-v7 backend: $NAME/$IMPLEMENTATION failed" >&2; exit 1;
    }
  done
  set +e
  python3 -B "$REFERENCE" validate "$T/cases/$NAME.ckir7" \
    > "$T/$NAME.reference" 2> "$T/$NAME.reference.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.reference" ] || {
    echo "checked-IR-v7 backend: $NAME/reference failed" >&2; exit 1;
  }
done < "$T/cases/manifest.tsv"

echo "checked-IR-v7 backend: native/self exact load/AND-OR-memory/store artifacts, all four truth rows, result 70; $POSITIVE positives and $COUNT controls passed"
