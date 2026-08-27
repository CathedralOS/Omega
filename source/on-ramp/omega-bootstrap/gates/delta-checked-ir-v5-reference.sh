#!/usr/bin/env sh
# Independent handcrafted CKIR5 decoding, meaning, and mutation gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
IR="$GATE_DIR/checked_ir_v5_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v5-fixture.py"
V4="$GATE_DIR/checked_ir_v4_reference.py"
for REQUIRED in "$IR" "$FIXTURE" "$V4"; do
  [ -f "$REQUIRED" ] || { echo "checked-IR-v5 reference: missing $REQUIRED" >&2; exit 1; }
done
command -v python3 >/dev/null 2>&1 || {
  echo "checked-IR-v5 reference: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(python3 -c 'import time; print(time.time())')

python3 -m py_compile "$IR" "$FIXTURE"
python3 -B "$FIXTURE" emit "$T/cases"
python3 -B "$FIXTURE" emit "$T/cases-repeat"
cmp "$T/cases/canonical.ckir5" "$T/cases-repeat/canonical.ckir5" || {
  echo "checked-IR-v5 reference: fixture emission is nondeterministic" >&2
  exit 1
}
CANONICAL="$T/cases/canonical.ckir5"
python3 -B "$IR" validate "$CANONICAL" > "$T/validate.out"
[ "$(python3 -B "$IR" run "$CANONICAL")" = 70 ] || {
  echo "checked-IR-v5 reference: canonical result is not 70" >&2
  exit 1
}
python3 -B "$FIXTURE" check "$CANONICAL"
if python3 -B "$V4" validate "$CANONICAL" > "$T/v4.out" 2> "$T/v4.err"; then
  echo "checked-IR-v5 reference: CKIR4 accepted CKIR5" >&2
  exit 1
fi

COUNT=0
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  COUNT=$((COUNT + 1))
  set +e
  python3 -B "$IR" validate "$T/cases/$NAME.ckir5" > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  if [ "$ACTUAL" -ne "$EXPECTED" ]; then
    echo "checked-IR-v5 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    tail -n 2 "$T/$NAME.err" >&2 || true
    exit 1
  fi
  if [ "$EXPECTED" -ne 0 ]; then
    [ ! -s "$T/$NAME.out" ] || {
      echo "checked-IR-v5 reference: rejected $NAME published stdout" >&2
      exit 1
    }
  fi
done < "$T/cases/manifest.tsv"

FINISHED=$(python3 -c 'import time; print(time.time())')
python3 - "$STARTED" "$FINISHED" "$COUNT" <<'PY'
import sys
started, finished = map(float, sys.argv[1:3])
print(
    "checked-IR-v5 reference: ConstructCase -> structural Call, Copy, "
    "parameter/nonzero-field CaseDispatch, complete payload binding, result 70; "
    f"{sys.argv[3]} isolated schema/identity/span/type/resource controls passed "
    f"in {finished-started:.3f}s"
)
PY
