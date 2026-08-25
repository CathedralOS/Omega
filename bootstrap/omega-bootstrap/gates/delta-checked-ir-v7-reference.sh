#!/usr/bin/env sh
# Independent CKIR7 LogicalAnd/LogicalOr decoding, meaning, and mutation gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
IR="$GATE_DIR/checked_ir_v7_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v7-fixture.py"
for REQUIRED in "$IR" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || { echo "checked-IR-v7 reference: missing $REQUIRED" >&2; exit 1; }
done
command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR-v7 reference: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
python3 -B "$FIXTURE" emit "$T/cases"
python3 -B "$FIXTURE" emit "$T/repeat"
cmp "$T/cases/canonical.ckir7" "$T/repeat/canonical.ckir7" || {
  echo "checked-IR-v7 reference: nondeterministic fixture" >&2; exit 1;
}

POSITIVE=0
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  python3 -B "$FIXTURE" check "$T/cases/$NAME.ckir7"
  [ "$(python3 -B "$IR" run "$T/cases/$NAME.ckir7")" = "$EXPECTED" ] || {
    echo "checked-IR-v7 reference: $NAME result mismatch" >&2; exit 1;
  }
done < "$T/cases/positives.tsv"

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  set +e
  python3 -B "$IR" validate "$T/cases/$NAME.ckir7" > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "checked-IR-v7 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    tail -n 2 "$T/$NAME.err" >&2 || true
    exit 1
  }
  [ ! -s "$T/$NAME.out" ] || {
    echo "checked-IR-v7 reference: rejected $NAME published stdout" >&2; exit 1;
  }
done < "$T/cases/manifest.tsv"

echo "checked-IR-v7 reference: AND/OR all four input rows and result 70; $POSITIVE positive carriers, $COUNT isolated schema/feature/arity/type/visibility/immediate/resource controls passed"
