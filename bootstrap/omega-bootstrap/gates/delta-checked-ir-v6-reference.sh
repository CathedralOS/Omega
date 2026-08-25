#!/usr/bin/env sh
# Independent CKIR6 LogicalNot decoding, meaning, and mutation gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
IR="$GATE_DIR/checked_ir_v6_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v6-fixture.py"
for REQUIRED in "$IR" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || { echo "checked-IR-v6 reference: missing $REQUIRED" >&2; exit 1; }
done
command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR-v6 reference: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
python3 -B "$FIXTURE" emit "$T/cases"
python3 -B "$FIXTURE" emit "$T/repeat"
cmp "$T/cases/canonical.ckir6" "$T/repeat/canonical.ckir6" || {
  echo "checked-IR-v6 reference: nondeterministic fixture" >&2; exit 1;
}
CANONICAL="$T/cases/canonical.ckir6"
python3 -B "$FIXTURE" check "$CANONICAL"
[ "$(python3 -B "$IR" run "$CANONICAL")" = 70 ] || {
  echo "checked-IR-v6 reference: canonical result is not 70" >&2; exit 1;
}

COUNT=0
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  set +e
  python3 -B "$IR" validate "$T/cases/$NAME.ckir6" > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "checked-IR-v6 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    tail -n 2 "$T/$NAME.err" >&2 || true
    exit 1
  }
  [ ! -s "$T/$NAME.out" ] || {
    echo "checked-IR-v6 reference: rejected $NAME published stdout" >&2; exit 1;
  }
done < "$T/cases/manifest.tsv"

echo "checked-IR-v6 reference: LogicalNot truth function and inherited result 70; $COUNT isolated schema/arity/type/visibility/resource controls passed"
