#!/usr/bin/env sh
# CKIR12 shared static-byte-view conservative backend gate.
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
  *) echo "checked-IR-v12 backend: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v12 backend: skipped ($TOOL absent)"; exit 0;
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v12-backend-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v12_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
V11_GATE="$GATE_DIR/delta-checked-ir-v11-backend.sh"
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
python3 -B "$FIXTURE" emit "$T/repeat"
cmp "$T/cases/canonical.ckir12" "$T/repeat/canonical.ckir12" || {
  echo "checked-IR-v12 backend: nondeterministic fixture" >&2; exit 1;
}

TAB=$(printf '\t')
POSITIVE=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  CKIR="$T/cases/$NAME.ckir12"
  python3 -B "$FIXTURE" check-ir "$CKIR"
  [ "$(python3 -B "$REFERENCE" run "$CKIR")" = "$EXPECTED" ] || {
    echo "checked-IR-v12 backend: $NAME meaning mismatch" >&2; exit 1;
  }
  for IMPLEMENTATION in native self; do
    "$T/backend.$IMPLEMENTATION" < "$CKIR" > "$T/$NAME.$IMPLEMENTATION.elf"
    python3 -B "$FIXTURE" check-artifact "$T/$NAME.$IMPLEMENTATION.elf" "$CKIR"
  done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf" || {
    echo "checked-IR-v12 backend: $NAME native/self artifact mismatch" >&2; exit 1;
  }
done < "$T/cases/positives.tsv"

# The artifact recognizer must reject corruption in the partial-head template.
python3 - "$T/one-byte.native.elf" "$T/bad-head.elf" <<'PY'
from pathlib import Path
import sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
needle = b"\x49\x8b\x03\x0f\xb6\x00"
at = raw.index(needle)
raw[at + 4] = 0xb7
Path(sys.argv[2]).write_bytes(raw)
PY
if python3 -B "$FIXTURE" check-artifact "$T/bad-head.elf" \
  "$T/cases/one-byte.ckir12" >/dev/null 2>&1; then
  echo "checked-IR-v12 backend: mutated SliceHead template accepted" >&2; exit 1
fi

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  for IMPLEMENTATION in native self; do
    set +e
    "$T/backend.$IMPLEMENTATION" < "$T/cases/$NAME.ckir12" \
      > "$T/$NAME.$IMPLEMENTATION.out"
    ACTUAL=$?
    set -e
    [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.$IMPLEMENTATION.out" ] || {
      echo "checked-IR-v12 backend: $NAME/$IMPLEMENTATION returned $ACTUAL, expected $EXPECTED" >&2
      exit 1
    }
  done
  set +e
  python3 -B "$REFERENCE" validate "$T/cases/$NAME.ckir12" \
    > "$T/$NAME.reference.out" 2> "$T/$NAME.reference.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.reference.out" ] || {
    echo "checked-IR-v12 backend: $NAME/reference returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
done < "$T/cases/manifest.tsv"

# Keep the inherited CKIR11 artifact contract adjacent and executable.
sh "$V11_GATE"

echo "checked-IR-v12 backend: one-byte true edge and empty false bypass both return 70; native/self descriptor/head/tail artifacts match; $POSITIVE positives and $COUNT mutation/resource controls passed; CKIR11 regression passed"
