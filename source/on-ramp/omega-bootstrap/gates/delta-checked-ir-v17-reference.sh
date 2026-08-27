#!/usr/bin/env sh
# Independent platform-neutral CKIR17 checked-adapter event gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
REFERENCE="$GATE_DIR/checked_ir_v17_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v17-fixture.py"
OLD_REFERENCE="$GATE_DIR/checked_ir_v15_reference.py"
for REQUIRED in "$REFERENCE" "$FIXTURE" "$OLD_REFERENCE"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v17 reference: missing $REQUIRED" >&2
    exit 1
  }
done
command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR-v17 reference: skipped (python3 absent)"
  exit 0
}

CKIR17_TEMP=$(mktemp -d)
trap 'rm -rf "$CKIR17_TEMP"' EXIT
python3 -B "$FIXTURE" emit "$CKIR17_TEMP/cases"
python3 -B "$FIXTURE" emit "$CKIR17_TEMP/repeat"
cmp "$CKIR17_TEMP/cases/canonical.ckir17" \
    "$CKIR17_TEMP/repeat/canonical.ckir17" >/dev/null || {
  echo "checked-IR-v17 reference: nondeterministic fixture" >&2
  exit 1
}

python3 -B "$REFERENCE" validate "$CKIR17_TEMP/cases/canonical.ckir17" \
  > "$CKIR17_TEMP/valid.out"
python3 -B "$FIXTURE" check "$CKIR17_TEMP/cases/canonical.ckir17"

observe() {
  LABEL=$1
  ADAPTER=$2
  HEX=$3
  EXPECTED=$4
  ACTUAL=$(python3 -B "$REFERENCE" run \
    "$CKIR17_TEMP/cases/canonical.ckir17" \
    --adapter "$ADAPTER" --hex "$HEX")
  [ "$ACTUAL" = "$EXPECTED" ] || {
    echo "checked-IR-v17 reference: $LABEL produced $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
}

observe write-empty write '' '[]'
observe write-one write 46 '[70]'
observe write-line-two write_line 4647 '[70,71,10]'
observe write-line-empty write_line '' '[10]'

TAB=$(printf '\t')
COUNT=0
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  set +e
  python3 -B "$REFERENCE" validate "$CKIR17_TEMP/cases/$NAME.ckir17" \
    > "$CKIR17_TEMP/$NAME.out" 2> "$CKIR17_TEMP/$NAME.err"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] && [ ! -s "$CKIR17_TEMP/$NAME.out" ] || {
    echo "checked-IR-v17 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    tail -n 2 "$CKIR17_TEMP/$NAME.err" >&2 || true
    exit 1
  }
done < "$CKIR17_TEMP/cases/manifest.tsv"

set +e
python3 -B "$OLD_REFERENCE" validate \
  "$CKIR17_TEMP/cases/canonical.ckir17" \
  > "$CKIR17_TEMP/v15.out" 2> "$CKIR17_TEMP/v15.err"
OLD_STATUS=$?
set -e
[ "$OLD_STATUS" -eq 251 ] && [ ! -s "$CKIR17_TEMP/v15.out" ] || {
  echo "checked-IR-v17 reference: CKIR15 accepted the new carrier" >&2
  exit 1
}
COUNT=$((COUNT + 1))

set +e
python3 -B "$REFERENCE" run "$CKIR17_TEMP/cases/canonical.ckir17" \
  --adapter write --input-file "$CKIR17_TEMP/cases/overlong.bin" \
  > "$CKIR17_TEMP/trace.out" 2> "$CKIR17_TEMP/trace.err"
TRACE_STATUS=$?
set -e
[ "$TRACE_STATUS" -eq 252 ] && [ ! -s "$CKIR17_TEMP/trace.out" ] || {
  echo "checked-IR-v17 reference: dynamic trace exhaustion returned $TRACE_STATUS" >&2
  exit 1
}
COUNT=$((COUNT + 1))

IDENTITY=$(python3 -B "$FIXTURE" inspect \
  "$CKIR17_TEMP/cases/canonical.ckir17")
echo "checked-IR-v17 reference: free helper/static adapters, explicit u8-to-i32 casts, ranked/reach-closed abstract write_byte events and $COUNT negative/resource/version controls PASS; $IDENTITY"
