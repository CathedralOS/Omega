#!/usr/bin/env sh
# Independent CKIR12 shared static-byte-view decoding and mutation gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
IR="$GATE_DIR/checked_ir_v12_reference.py"
IR11="$GATE_DIR/checked_ir_v11_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v12-fixture.py"
for REQUIRED in "$IR" "$IR11" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v12 reference: missing $REQUIRED" >&2
    exit 1
  }
done
command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR-v12 reference: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
python3 -B "$FIXTURE" emit "$T/cases"
python3 -B "$FIXTURE" emit "$T/repeat"
cmp "$T/cases/canonical.ckir12" "$T/repeat/canonical.ckir12" || {
  echo "checked-IR-v12 reference: nondeterministic fixture" >&2
  exit 1
}

POSITIVE=0
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  python3 -B "$FIXTURE" check "$T/cases/$NAME.ckir12"
  [ "$(python3 -B "$IR" run "$T/cases/$NAME.ckir12")" = "$EXPECTED" ] || {
    echo "checked-IR-v12 reference: $NAME result mismatch" >&2
    exit 1
  }
done < "$T/cases/positives.tsv"

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  set +e
  python3 -B "$IR" validate "$T/cases/$NAME.ckir12" \
    > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "checked-IR-v12 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    tail -n 2 "$T/$NAME.err" >&2 || true
    exit 1
  }
  [ ! -s "$T/$NAME.out" ] || {
    echo "checked-IR-v12 reference: rejected $NAME published stdout" >&2
    exit 1
  }
done < "$T/cases/manifest.tsv"

set +e
python3 -B "$IR11" validate "$T/cases/canonical.ckir12" \
  > "$T/new-major-v11.out" 2> "$T/new-major-v11.err"
ACTUAL=$?
set -e
[ "$ACTUAL" -eq 251 ] && [ ! -s "$T/new-major-v11.out" ] || {
  echo "checked-IR-v12 reference: CKIR11 cross-major control failed" >&2
  exit 1
}
COUNT=$((COUNT + 1))

echo "checked-IR-v12 reference: empty bypass avoids head/tail, one-byte tail empties, and Fp returns 70; $POSITIVE positives and $COUNT schema/type/constant/op/synthetic/value/resource controls passed"
