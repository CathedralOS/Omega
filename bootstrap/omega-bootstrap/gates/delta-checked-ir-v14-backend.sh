#!/usr/bin/env sh
# CKIR14 recursive full-width arithmetic conservative backend gate.
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
  *) echo "checked-IR-v14 backend: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign cmp; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v14 backend: skipped ($TOOL absent)"; exit 0;
  }
done

BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp"
FIXTURE="$GATE_DIR/delta-checked-ir-v14-backend-fixture.py"
REFERENCE="$GATE_DIR/checked_ir_v14_reference.py"
LOWERMACHINE="$OMEGA_PATH_DELTA/samples/lowermachine.alp"
V12_GATE="$GATE_DIR/delta-checked-ir-v12-backend.sh"
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
cmp "$T/cases/positives.tsv" "$T/repeat/positives.tsv"
cmp "$T/cases/invalid.tsv" "$T/repeat/invalid.tsv"

TAB=$(printf '\t')
POSITIVE=0
while IFS="$TAB" read -r NAME OUTCOME; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  CKIR="$T/cases/$NAME.ckir14"
  cmp "$CKIR" "$T/repeat/$NAME.ckir14" || {
    echo "checked-IR-v14 backend: nondeterministic $NAME carrier" >&2; exit 1;
  }
  python3 -B "$FIXTURE" check-ir "$CKIR" "$OUTCOME"
  for IMPLEMENTATION in native self; do
    "$T/backend.$IMPLEMENTATION" < "$CKIR" > "$T/$NAME.$IMPLEMENTATION.elf"
    python3 -B "$FIXTURE" check-artifact \
      "$T/$NAME.$IMPLEMENTATION.elf" "$CKIR"
  done
  cmp "$T/$NAME.native.elf" "$T/$NAME.self.elf" || {
    echo "checked-IR-v14 backend: $NAME native/self artifact mismatch" >&2; exit 1;
  }
done < "$T/cases/positives.tsv"

# The artifact recognizer must reject a selected-node opcode substitution.
python3 - "$T/recursive-mixed.native.elf" "$T/bad-add.elf" <<'PY'
from pathlib import Path
import sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
at = raw.index(b"\x03\x85")
raw[at] = 0x2b
Path(sys.argv[2]).write_bytes(raw)
PY
if python3 -B "$FIXTURE" check-artifact "$T/bad-add.elf" \
  "$T/cases/recursive-mixed.ckir14" >/dev/null 2>&1; then
  echo "checked-IR-v14 backend: mutated Add artifact accepted" >&2; exit 1
fi

INVALID=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  INVALID=$((INVALID + 1))
  CKIR="$T/cases/$NAME.ckir14"
  cmp "$CKIR" "$T/repeat/$NAME.ckir14" || {
    echo "checked-IR-v14 backend: nondeterministic $NAME control" >&2; exit 1;
  }
  for IMPLEMENTATION in native self; do
    set +e
    "$T/backend.$IMPLEMENTATION" < "$CKIR" > "$T/$NAME.$IMPLEMENTATION.out"
    ACTUAL=$?
    set -e
    [ "$ACTUAL" -eq "$EXPECTED" ] && \
      [ ! -s "$T/$NAME.$IMPLEMENTATION.out" ] || {
      echo "checked-IR-v14 backend: $NAME/$IMPLEMENTATION returned $ACTUAL, expected $EXPECTED" >&2
      exit 1
    }
  done
  set +e
  python3 -B "$REFERENCE" validate "$CKIR" \
    > "$T/$NAME.reference.out" 2> "$T/$NAME.reference.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.reference.out" ] || {
    echo "checked-IR-v14 backend: $NAME/reference returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
done < "$T/cases/invalid.tsv"

# Keep the inherited CKIR12 artifact contract adjacent and executable.
sh "$V12_GATE"

echo "checked-IR-v14 backend: $POSITIVE deterministic no-view/composed-view success and runtime-trap carriers preserve full-u32 edges, widening custody, exact add/subtract/multiply templates, shared ud2 branches, and native/self identity; $INVALID identity/profile/custody controls passed; CKIR12 regression passed"
