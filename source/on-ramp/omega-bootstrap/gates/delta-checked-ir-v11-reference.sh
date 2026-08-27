#!/usr/bin/env sh
# Independent CKIR11 canonical u32 Trapping Add decoding and mutation gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
IR="$GATE_DIR/checked_ir_v11_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v11-fixture.py"
for REQUIRED in "$IR" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || { echo "checked-IR-v11 reference: missing $REQUIRED" >&2; exit 1; }
done
command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR-v11 reference: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
python3 -B "$FIXTURE" emit "$T/cases"
python3 -B "$FIXTURE" emit "$T/repeat"
cmp "$T/cases/canonical.ckir11" "$T/repeat/canonical.ckir11" || {
  echo "checked-IR-v11 reference: nondeterministic fixture" >&2; exit 1;
}

POSITIVE=0
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  python3 -B "$FIXTURE" check "$T/cases/$NAME.ckir11"
  [ "$(python3 -B "$IR" run "$T/cases/$NAME.ckir11")" = "$EXPECTED" ] || {
    echo "checked-IR-v11 reference: $NAME result mismatch" >&2; exit 1;
  }
done < "$T/cases/positives.tsv"

python3 -B "$IR" validate "$T/cases/runtime-overflow.ckir11" >/dev/null
set +e
python3 -B "$IR" run "$T/cases/runtime-overflow.ckir11" \
  > "$T/runtime-overflow.out" 2> "$T/runtime-overflow.err"
ACTUAL=$?
set -e
[ "$ACTUAL" -eq 251 ] && [ ! -s "$T/runtime-overflow.out" ] || {
  echo "checked-IR-v11 reference: runtime overflow did not trap without publication" >&2
  exit 1
}

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  set +e
  python3 -B "$IR" validate "$T/cases/$NAME.ckir11" > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "checked-IR-v11 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    tail -n 2 "$T/$NAME.err" >&2 || true
    exit 1
  }
  [ ! -s "$T/$NAME.out" ] || {
    echo "checked-IR-v11 reference: rejected $NAME published stdout" >&2; exit 1;
  }
done < "$T/cases/manifest.tsv"

set +e
python3 -B "$IR" validate "$T/cases/old-schema-major-8.ckir11" \
  > "$T/old-schema-major-8.out" 2> "$T/old-schema-major-8.err"
ACTUAL=$?
set -e
[ "$ACTUAL" -eq 251 ] && [ ! -s "$T/old-schema-major-8.out" ] || {
  echo "checked-IR-v11 reference: CKIR8 cross-major control failed" >&2; exit 1;
}
COUNT=$((COUNT + 1))

echo "checked-IR-v11 reference: canonical u32 Trapping Add preserves 0+70/69+1/near-limit success, traps overflow without publication, and returns 70; $POSITIVE positives and $COUNT isolated schema/feature/arity/type/visibility/immediate/resource controls passed"
