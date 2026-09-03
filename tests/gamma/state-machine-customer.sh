#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null

make_request() {
    INPUT=$1 SOURCE="$GATE_DIR/fixtures/delta0_compiler.gamma" python3 -c '
import os, struct, sys
from pathlib import Path
source = Path(os.environ["SOURCE"]).read_bytes()
invocation_input = os.environ["INPUT"].encode("ascii")
sys.stdout.buffer.write(
    struct.pack("<I", len(source))
    + source
    + invocation_input
)
'
}

PROGRAM='S00I0003S0aI0101S14U0001S17N0014S21H00'
make_request "$PROGRAM" > "$TMP/request"
"$TMP/evaluator" < "$TMP/request" > "$TMP/program.tape"
EXPECTED='01000300000000000000010101000000000000000400010e0014000000000000000000'
EXPECTED=$EXPECTED PROGRAM="$TMP/program.tape" python3 -c '
import os
from pathlib import Path
actual = Path(os.environ["PROGRAM"]).read_bytes()
expected = bytes.fromhex(os.environ["EXPECTED"])
if actual != expected:
    raise SystemExit(f"emitted {actual.hex()}, expected {expected.hex()}")
'

stamp_seed "$TMP/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/program" >/dev/null
"$TMP/program" > "$TMP/program.out"
[ ! -s "$TMP/program.out" ]

for BAD in S01H00 X00; do
    make_request "$BAD" > "$TMP/request"
    set +e
    "$TMP/evaluator" < "$TMP/request" > "$TMP/rejected"
    STATUS=$?
    set -e
    [ "$STATUS" -eq 2 ] && [ ! -s "$TMP/rejected" ] || {
        echo "Gamma state-machine customer: $BAD was not a quiet authored trap" >&2
        exit 1
    }
done

echo "Gamma state-machine customer: 81-line compiler emitted and ran exact 35-byte countdown CFG"
