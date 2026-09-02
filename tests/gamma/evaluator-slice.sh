#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma evaluator: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null

assert_case() {
    NAME=$1 EXPECTED_STATUS=$2 SOURCE=$3 INPUT_HEX=$4 EXPECTED_HEX=$5
    SOURCE=$SOURCE INPUT_HEX=$INPUT_HEX python3 -c '
import os, struct, sys
source = os.environ["SOURCE"].encode("ascii")
invocation_input = bytes.fromhex(os.environ["INPUT_HEX"])
sys.stdout.buffer.write(struct.pack("<I", len(source)) + source + invocation_input)
' > "$TMP/request"
    set +e
    "$TMP/evaluator" < "$TMP/request" > "$TMP/output"
    ACTUAL_STATUS=$?
    set -e
    [ "$ACTUAL_STATUS" -eq "$EXPECTED_STATUS" ] || {
        echo "Gamma evaluator: $NAME status $ACTUAL_STATUS, expected $EXPECTED_STATUS" >&2
        exit 1
    }
    EXPECTED_HEX=$EXPECTED_HEX OUTPUT="$TMP/output" python3 -c '
import os
from pathlib import Path
actual = Path(os.environ["OUTPUT"]).read_bytes()
expected = bytes.fromhex(os.environ["EXPECTED_HEX"])
if actual != expected:
    raise SystemExit(f"output {actual.hex()}, expected {expected.hex()}")
' || {
        echo "Gamma evaluator: $NAME output mismatch" >&2
        exit 1
    }
    echo "ok - $NAME"
}

assert_case literal 0 ': main 0x2a output-byte ;' '' 2a
assert_case word-little-endian 0 ': main 0x102030405060708 output-word ;' '' 0807060504030201
assert_case comments 0 '# head
: main # body
  0x41 output-byte ;' '' 41
assert_case forward-call 0 ': main emit ; : emit 0x41 output-byte ;' '' 41
assert_case nested-call 0 ': one 0x1 + ; : two one one ; : main 0x3f two output-byte ;' '' 41
assert_case branch-true 0 ': yes 0x59 output-byte ; : no 0x4e output-byte ; : main 0x1 branch yes no ;' '' 59
assert_case branch-false 0 ': yes 0x59 output-byte ; : no 0x4e output-byte ; : main 0x0 branch yes no ;' '' 4e
assert_case tail-loop 0 ': loop 0x0 cell-get dup 0x0 = branch done step ; : step 0x1 - 0x0 cell-set jump loop ; : done drop 0x41 output-byte ; : main 0x186a0 0x0 cell-set jump loop ;' '' 41
assert_case ordinary-recursion 0 ': loop dup 0x0 = branch done recurse ; : recurse 0x1 - loop ; : done drop ; : main 0x2710 loop ;' '' ''
assert_case input 0 ': main input-length output-byte 0x0 input-get output-byte ;' 5a 015a
assert_case input-bounds 2 ': main 0x1 input-get ;' 5a ''
assert_case cell 0 ': main 0x41 0x0 cell-set 0x0 cell-get output-byte ;' '' 41
assert_case cell-bounds 2 ': main 0x13e0000 cell-get ;' '' ''
assert_case stack-ops 0 ': main 0x40 0x1 over drop swap swap + dup drop output-byte ;' '' 41
assert_case signed-less 0 ': main 0xffffffffffffffff 0x0 < output-byte ;' '' 01
assert_case divide 0 ': main 0xfffffffffffffff9 0x2 / 0xfffffffffffffffd = output-byte ;' '' 01
assert_case divide-zero 2 ': main 0x1 0x0 / ;' '' ''
assert_case divide-overflow 2 ': main 0x8000000000000000 0xffffffffffffffff / ;' '' ''
assert_case underflow 2 ': main drop ;' '' ''
assert_case unknown-word 2 ': main absent ;' '' ''
assert_case duplicate-name 1 ': main ; : main ;' '' ''
assert_case builtin-collision 1 ': dup ; : main ;' '' ''
assert_case missing-main 1 ': other ;' '' ''
assert_case wide-literal 2 ': main 0x10000000000000000 ;' '' ''
assert_case failed-assertion 2 ': main output-position 0x1 assert-equal ;' '' ''
assert_case late-prefix 2 ': main 0x41 output-byte drop ;' '' 41

printf '\001\000\000\000\000' > "$TMP/request"
set +e
"$TMP/evaluator" < "$TMP/request" > "$TMP/output"
STATUS=$?
set -e
[ "$STATUS" -eq 1 ] && [ ! -s "$TMP/output" ]
echo "ok - invalid-source-byte"

printf '\020\000\000\000short' > "$TMP/request"
set +e
"$TMP/evaluator" < "$TMP/request" > "$TMP/output"
STATUS=$?
set -e
[ "$STATUS" -eq 1 ] && [ ! -s "$TMP/output" ]
echo "ok - invalid-source-length"

echo "Gamma evaluator: 28/28 cases passed ($(wc -l < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" | tr -d ' ') lines, $(wc -c < "$TMP/evaluator.tape" | tr -d ' ') tape bytes)"