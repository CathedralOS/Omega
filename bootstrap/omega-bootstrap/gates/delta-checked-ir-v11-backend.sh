#!/usr/bin/env sh
# CKIR11 canonical u32 Trapping Add backend gate. CKIR4-10 parity stays adjacent.
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
  *) echo "checked-IR-v11 backend: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v11 backend: skipped ($TOOL absent)"; exit 0;
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v11-backend-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v11_reference.py"
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
  CKIR="$T/cases/$NAME.ckir11"
  python3 -B "$FIXTURE" check-ir "$CKIR"
  for IMPLEMENTATION in native self; do
    "$T/backend.$IMPLEMENTATION" < "$CKIR" > "$T/$NAME.$IMPLEMENTATION.elf"
  done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf" || {
    echo "checked-IR-v11 backend: $NAME native/self artifact mismatch" >&2; exit 1;
  }
  python3 -B "$FIXTURE" check-artifact "$T/$NAME.native.elf" "$CKIR"
done < "$T/cases/positives.tsv"

for IMPLEMENTATION in native self; do
  "$T/backend.$IMPLEMENTATION" < "$T/cases/runtime-overflow.ckir11" \
    > "$T/runtime-overflow.$IMPLEMENTATION.elf"
done
cmp "$T/runtime-overflow.native.elf" "$T/runtime-overflow.self.elf" || {
  echo "checked-IR-v11 backend: runtime-overflow artifact mismatch" >&2; exit 1;
}
python3 -B "$FIXTURE" check-artifact "$T/runtime-overflow.native.elf" \
  "$T/cases/runtime-overflow.ckir11"

python3 - "$T/add-69-plus-1.native.elf" "$T/bad-add.elf" <<'PY'
from pathlib import Path
import sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
at = raw.index(b"\x03\x85")
raw[at] = 0x2b
Path(sys.argv[2]).write_bytes(raw)
PY
if python3 -B "$FIXTURE" check-artifact "$T/bad-add.elf" \
  "$T/cases/add-69-plus-1.ckir11" >/dev/null 2>&1; then
  echo "checked-IR-v11 backend: mutated Add accepted" >&2; exit 1
fi

# CKIR11 introduces a required profile relation rather than a new opcode. The
# same table body is valid CKIR8 and remains accepted by the shared historical
# backend under its own major; only the CKIR11 decoder rejects that cross-pair.
for IMPLEMENTATION in native self; do
  "$T/backend.$IMPLEMENTATION" < "$T/cases/old-schema-major-8.ckir11" \
    > "$T/old-schema-major-8.$IMPLEMENTATION.elf"
done
cmp "$T/old-schema-major-8.native.elf" "$T/old-schema-major-8.self.elf" || {
  echo "checked-IR-v11 backend: inherited CKIR8 artifact mismatch" >&2; exit 1;
}

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  for IMPLEMENTATION in native self; do
    set +e
    "$T/backend.$IMPLEMENTATION" < "$T/cases/$NAME.ckir11" > "$T/$NAME.$IMPLEMENTATION"
    ACTUAL=$?
    set -e
    [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.$IMPLEMENTATION" ] || {
      echo "checked-IR-v11 backend: $NAME/$IMPLEMENTATION failed" >&2; exit 1;
    }
  done
  set +e
  python3 -B "$REFERENCE" validate "$T/cases/$NAME.ckir11" \
    > "$T/$NAME.reference" 2> "$T/$NAME.reference.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.reference" ] || {
    echo "checked-IR-v11 backend: $NAME/reference failed" >&2; exit 1;
  }
done < "$T/cases/manifest.tsv"

echo "checked-IR-v11 backend: native/self canonical Add/carry/range/store artifacts preserve 0+70/69+1/near-limit success, retain overflow traps, and return 70; $POSITIVE positives and $COUNT controls passed"
