#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
python3 -B "$GATE_DIR/delta-checked-ir-v16-fixture.py" emit "$T"
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME OUTCOME; do
  python3 -B "$GATE_DIR/delta-checked-ir-v16-fixture.py" check \
    "$T/$NAME.ckir16" "$OUTCOME"
  python3 -B "$GATE_DIR/checked_ir_v16_reference.py" validate \
    "$T/$NAME.ckir16" >/dev/null
done < "$T/positives.tsv"
while IFS="$TAB" read -r NAME STATUS; do
  set +e
  python3 -B "$GATE_DIR/checked_ir_v16_reference.py" validate \
    "$T/$NAME.ckir16" >/dev/null 2>&1
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$STATUS" ] || {
    echo "checked-IR-v16 reference: $NAME status $ACTUAL, expected $STATUS" >&2
    exit 1
  }
done < "$T/manifest.tsv"
echo "checked-IR-v16 reference: PASS"
