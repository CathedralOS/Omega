#!/usr/bin/env sh
# Independent CKIR15 recurrent shared-view edge decoding and mutation gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
IR="$GATE_DIR/checked_ir_v15_reference.py"
IR14="$GATE_DIR/checked_ir_v14_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v15-fixture.py"
for REQUIRED in "$IR" "$IR14" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v15 reference: missing $REQUIRED" >&2
    exit 1
  }
done
command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR-v15 reference: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
python3 -B "$FIXTURE" emit "$T/cases"
python3 -B "$FIXTURE" emit "$T/repeat"
cmp "$T/cases/canonical.ckir15" "$T/repeat/canonical.ckir15" >/dev/null || {
  echo "checked-IR-v15 reference: nondeterministic fixture" >&2
  exit 1
}

POSITIVE=0
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  POSITIVE=$((POSITIVE + 1))
  python3 -B "$FIXTURE" check "$T/cases/$NAME.ckir15"
  [ "$(python3 -B "$IR" run "$T/cases/$NAME.ckir15")" = "$EXPECTED" ] || {
    echo "checked-IR-v15 reference: $NAME result mismatch" >&2
    exit 1
  }
done < "$T/cases/positives.tsv"

COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  set +e
  python3 -B "$IR" validate "$T/cases/$NAME.ckir15" \
    > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$T/$NAME.out" ] || {
    echo "checked-IR-v15 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    tail -n 2 "$T/$NAME.err" >&2 || true
    exit 1
  }
done < "$T/cases/manifest.tsv"

set +e
python3 -B "$IR14" validate "$T/cases/canonical.ckir15" \
  > "$T/new-major-v14.out" 2> "$T/new-major-v14.err"
ACTUAL=$?
set -e
[ "$ACTUAL" -eq 251 ] && [ ! -s "$T/new-major-v14.out" ] || {
  echo "checked-IR-v15 reference: CKIR14 cross-major control failed" >&2
  exit 1
}
COUNT=$((COUNT + 1))

echo "checked-IR-v15 reference: recurrent two-byte and one-byte true edges plus empty false bypass return 70; ordered pass-through vectors and $COUNT schema/type/constant/op/synthetic/value/resource controls passed across $POSITIVE positives"
