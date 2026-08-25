#!/usr/bin/env sh
# Focused lexical regression: labels beginning r+digit are not registers.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
cd "$GATE_DIR"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "Alpha assembler register-label regression: skipped (requires Darwin arm64)"; exit 0 ;;
esac
command -v python3 >/dev/null 2>&1 || {
  echo "Alpha assembler register-label regression: skipped (python3 absent)"
  exit 0
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
START=$(python3 -c 'import time; print(time.time())')
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
BC="$T/bc"
stamp_beta_compiler "$BC" >/dev/null

expect_equal() {
  NAME=$1 SOURCE=$2
  "$ASM" < "$SOURCE" > "$T/$NAME.tape"
  python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" < "$SOURCE" > "$T/$NAME.ref"
  cmp "$T/$NAME.tape" "$T/$NAME.ref"
}

expect_exit() {
  NAME=$1 EXPECTED=$2
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME.exe" >/dev/null
  set +e
  "$T/$NAME.exe" > "$T/$NAME.stdout" 2> "$T/$NAME.stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "Alpha assembler register-label regression: $NAME exited $ACTUAL, expected $EXPECTED" >&2
    exit 1
  }
  [ ! -s "$T/$NAME.stdout" ] && [ ! -s "$T/$NAME.stderr" ]
}

# This is the exact Beta-produced shape that exposed `call r5x` being encoded
# as address 5 rather than resolving the procedure label.
printf '%s\n' 'proc r5x() { return 252 } proc main() { return r5x() }' \
  | "$BC" > "$T/beta-r5x.alpha"
grep -q '^call r5x$' "$T/beta-r5x.alpha"
expect_equal beta-r5x "$T/beta-r5x.alpha"
python3 - "$T/beta-r5x.tape" <<'PY'
from pathlib import Path
import sys
tape = Path(sys.argv[1]).read_bytes()
if len(tape) != 220 or tape[171] != 0x13 or int.from_bytes(tape[172:180], "little") != 83:
    raise SystemExit("Beta r5x call is not canonical target 83")
PY
expect_exit beta-r5x 252

printf '%s\n' \
  '        jmp r256x' \
  'r256x:' \
  '        jmp r0foo' \
  'r5x:' \
  '        imm r0, 252' \
  '        ret' \
  'r0foo:' \
  '        call r5x' \
  '        halt r0' > "$T/label-prefixes.alpha"
expect_equal label-prefixes "$T/label-prefixes.alpha"
expect_exit label-prefixes 252

printf '%s\n' \
  '        imm r0, 7' \
  '        imm r255, 245' \
  '        add r0, r255' \
  '        halt r0' > "$T/register-bounds.alpha"
expect_equal register-bounds "$T/register-bounds.alpha"
expect_exit register-bounds 252

# Canonical assembler operand errors use the existing status 7. The reference
# independently rejects the same out-of-range register grammar.
printf '%s\n' 'imm r256, 0' > "$T/register-over.alpha"
set +e
"$ASM" < "$T/register-over.alpha" > "$T/register-over.tape"
ASM_STATUS=$?
python3 "$OMEGA_PATH_ALPHA_ASSEMBLER/asm_ref.py" < "$T/register-over.alpha" \
  > "$T/register-over.ref" 2> "$T/register-over.stderr"
REF_STATUS=$?
set -e
[ "$ASM_STATUS" -eq 7 ] && [ "$REF_STATUS" -ne 0 ]

ELAPSED=$(python3 - "$START" <<'PY'
import sys, time
print(f"{time.time() - float(sys.argv[1]):.3f}")
PY
)
echo "Alpha assembler register-label regression: Beta call r5x, r0foo/r5x/r256x labels, r0/r255, and r256 rejection passed (${ELAPSED}s)"
