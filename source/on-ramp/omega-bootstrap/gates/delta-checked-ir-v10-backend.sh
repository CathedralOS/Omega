#!/usr/bin/env sh
# CKIR10 IntegerWiden backend gate. CKIR4-9 parity remains adjacent.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT")
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR-v10 backend: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v10 backend: skipped ($TOOL absent)"; exit 0;
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v10-backend-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v10_reference.py"
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
  CKIR="$T/cases/$NAME.ckir10"
  python3 -B "$FIXTURE" check-ir "$CKIR"
  for IMPLEMENTATION in native self; do
    "$T/backend.$IMPLEMENTATION" < "$CKIR" > "$T/$NAME.$IMPLEMENTATION.elf"
  done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf" || {
    echo "checked-IR-v10 backend: $NAME native/self artifact mismatch" >&2; exit 1;
  }
  python3 -B "$FIXTURE" check-artifact "$T/$NAME.native.elf" "$CKIR"
done < "$T/cases/positives.tsv"

python3 - "$T/widen-70.native.elf" "$T/bad-movzx.elf" <<'PY'
from pathlib import Path
import sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
at = raw.index(b"\x0f\xb6\xc0")
raw[at + 1] = 0xb7
Path(sys.argv[2]).write_bytes(raw)
PY
if python3 -B "$FIXTURE" check-artifact "$T/bad-movzx.elf" \
  "$T/cases/widen-70.ckir10" >/dev/null 2>&1; then
  echo "checked-IR-v10 backend: mutated MOVZX accepted" >&2; exit 1
fi

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  for IMPLEMENTATION in native self; do
    set +e
    "$T/backend.$IMPLEMENTATION" < "$T/cases/$NAME.ckir10" > "$T/$NAME.$IMPLEMENTATION"
    ACTUAL=$?
    set -e
    [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.$IMPLEMENTATION" ] || {
      echo "checked-IR-v10 backend: $NAME/$IMPLEMENTATION failed" >&2; exit 1;
    }
  done
  set +e
  python3 -B "$REFERENCE" validate "$T/cases/$NAME.ckir10" \
    > "$T/$NAME.reference" 2> "$T/$NAME.reference.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.reference" ] || {
    echo "checked-IR-v10 backend: $NAME/reference failed" >&2; exit 1;
  }
done < "$T/cases/manifest.tsv"

echo "checked-IR-v10 backend: native/self exact load/MOVZX/store artifacts preserve 0/70/255 and result 70; $POSITIVE positives and $COUNT controls passed"
