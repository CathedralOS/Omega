#!/usr/bin/env sh
# Focused independent CKIR4 opcode-13 meaning and exact-artifact gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
for TOOL in python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR-v4 reference: skipped ($TOOL absent)"
    exit 0
  }
done

IR="$GATE_DIR/checked_ir_v4_reference.py"
ELF="$GATE_DIR/checked_elf_v4_reference.py"
FIXTURE="$GATE_DIR/delta-checked-ir-v4-fixture.py"
V3="$GATE_DIR/checked_ir_v3_reference.py"
for REQUIRED in "$IR" "$ELF" "$FIXTURE" "$V3"; do
  [ -f "$REQUIRED" ] || {
    echo "checked-IR-v4 reference: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(python3 -c 'import time; print(time.time())')

python3 -m py_compile "$IR" "$ELF" "$FIXTURE"
python3 -B "$FIXTURE" emit "$T/cases"
CANONICAL="$T/cases/canonical.ckir4"

python3 -B "$IR" validate "$CANONICAL" > "$T/validate.out"
[ "$(python3 -B "$IR" run "$CANONICAL")" = 70 ] || {
  echo "checked-IR-v4 reference: canonical result is not 70" >&2
  exit 1
}
if python3 -B "$V3" validate "$CANONICAL" > "$T/v3.out" 2> "$T/v3.err"; then
  echo "checked-IR-v4 reference: CKIR3 accepted CKIR4" >&2
  exit 1
fi

CASE_COUNT=0
TAB=$(printf '\t')
while IFS="$TAB" read -r NAME EXPECTED; do
  [ -n "$NAME" ] || continue
  CASE_COUNT=$((CASE_COUNT + 1))
  set +e
  python3 -B "$IR" validate "$T/cases/$NAME.ckir4" \
    > "$T/$NAME.out" 2> "$T/$NAME.err"
  ACTUAL=$?
  set -e
  if [ "$ACTUAL" -ne "$EXPECTED" ]; then
    echo "checked-IR-v4 reference: $NAME returned $ACTUAL, expected $EXPECTED" >&2
    exit 1
  fi
  [ ! -s "$T/$NAME.out" ] || {
    echo "checked-IR-v4 reference: malformed $NAME published stdout" >&2
    exit 1
  }
done < "$T/cases/manifest.tsv"
[ "$CASE_COUNT" -eq 15 ] || {
  echo "checked-IR-v4 reference: mutation census $CASE_COUNT, expected 15" >&2
  exit 1
}

python3 -B "$ELF" emit "$CANONICAL" "$T/canonical.elf" > "$T/emit.out"
[ "$(wc -c < "$T/canonical.elf" | tr -d ' ')" -eq 8192 ] || {
  echo "checked-IR-v4 reference: canonical ELF extent drifted" >&2
  exit 1
}
python3 -B "$ELF" check "$CANONICAL" "$T/canonical.elf" > "$T/check.out"
python3 -B "$ELF" mutation-sweep "$CANONICAL" "$T/canonical.elf" > "$T/sweep.out"
python3 -B "$FIXTURE" check-artifact "$CANONICAL" "$T/canonical.elf" \
  > "$T/templates.out"
EMPTY="$T/cases/empty.ckir4"
[ "$(python3 -B "$IR" run "$EMPTY")" = 70 ] || {
  echo "checked-IR-v4 reference: empty constructor result is not 70" >&2
  exit 1
}
python3 -B "$ELF" emit "$EMPTY" "$T/empty.elf" > "$T/empty-emit.out"
python3 -B "$ELF" check "$EMPTY" "$T/empty.elf" > "$T/empty-check.out"
python3 -B "$FIXTURE" check-empty-artifact "$EMPTY" "$T/empty.elf" \
  > "$T/empty-templates.out"
python3 -B "$FIXTURE" mutate-artifact "$T/canonical.elf" "$T/mismatched.elf"
if python3 -B "$ELF" check "$CANONICAL" "$T/mismatched.elf" \
    > "$T/mismatch.out" 2> "$T/mismatch.err"; then
  echo "checked-IR-v4 reference: constructor-template artifact mismatch accepted" >&2
  exit 1
fi

FINISHED=$(python3 -c 'import time; print(time.time())')
python3 - "$STARTED" "$FINISHED" "$CASE_COUNT" "$T/templates.out" \
  "$T/empty-templates.out" <<'PY'
from pathlib import Path
import sys

started, finished = map(float, sys.argv[1:3])
print(
    "checked-IR-v4 reference: nested ConstructRecord -> Call -> Copy result 70, "
    f"{sys.argv[3]} isolated CKIR rejections including direct-edge, exact "
    "object/frame/templates, complete ELF byte mutation sweep, and explicit "
    f"artifact mismatch passed in {finished-started:.3f}s; "
    + Path(sys.argv[4]).read_text(encoding="ascii").strip()
    + "; " + Path(sys.argv[5]).read_text(encoding="ascii").strip()
)
PY
