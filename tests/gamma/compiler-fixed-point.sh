#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma compiler fixed point: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
COMPILER_SOURCE="$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_SOURCE"
COMPILER_RECEIPT="$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_RECEIPT"
COMPILER_TAPE="$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_TAPE"
DIRECT_SOURCE="$GATE_DIR/gamma-to-beta-experiment/direct_compiler.gamma"
DELTA0_SOURCE="$GATE_DIR/fixtures/delta0_compiler.gamma"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null

PROGRAM=$COMPILER_SOURCE SUBJECT=$COMPILER_SOURCE python3 -c '
import os, struct, sys
from pathlib import Path
program = Path(os.environ["PROGRAM"]).read_bytes()
subject = Path(os.environ["SUBJECT"]).read_bytes()
sys.stdout.buffer.write(struct.pack("<I", len(program)) + program + subject)
' > "$TMP/bootstrap-request"
"$TMP/evaluator" < "$TMP/bootstrap-request" > "$TMP/t0.beta"
cmp "$COMPILER_RECEIPT" "$TMP/t0.beta"
"$TMP/beta-compiler" < "$TMP/t0.beta" > "$TMP/t0.tape"
cmp "$COMPILER_TAPE" "$TMP/t0.tape"

materialize_gamma_compiler "$TMP/compiler" >/dev/null
"$TMP/compiler" < "$COMPILER_SOURCE" > "$TMP/t1.beta"
cmp "$COMPILER_RECEIPT" "$TMP/t1.beta"
"$TMP/beta-compiler" < "$TMP/t1.beta" > "$TMP/t1.tape"
cmp "$COMPILER_TAPE" "$TMP/t1.tape"

compile_gamma_source_to_tape "$TMP/compiler" "$TMP/beta-compiler" \
    "$DIRECT_SOURCE" "$TMP/direct-compiler.tape"
stamp_seed "$TMP/direct-compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/direct-compiler" >/dev/null

PROGRAM=$COMPILER_SOURCE SUBJECT=$DELTA0_SOURCE python3 -c '
import os, struct, sys
from pathlib import Path
program = Path(os.environ["PROGRAM"]).read_bytes()
subject = Path(os.environ["SUBJECT"]).read_bytes()
sys.stdout.buffer.write(struct.pack("<I", len(program)) + program + subject)
' > "$TMP/delta0-request"
"$TMP/evaluator" < "$TMP/delta0-request" > "$TMP/delta0-seeded.beta"
"$TMP/compiler" < "$DELTA0_SOURCE" > "$TMP/delta0-native.beta"
cmp "$TMP/delta0-seeded.beta" "$TMP/delta0-native.beta"
"$TMP/beta-compiler" < "$TMP/delta0-native.beta" > "$TMP/delta0-native.tape"
"$TMP/direct-compiler" < "$DELTA0_SOURCE" > "$TMP/delta0-direct.tape"
cmp "$TMP/delta0-native.tape" "$TMP/delta0-direct.tape"

stamp_seed "$TMP/delta0-native.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/delta0" >/dev/null
printf 'S00I0003S0aI0101S14U0001S17N0014S21H00' | "$TMP/delta0" > "$TMP/countdown.tape"
EXPECTED=01000300000000000000010101000000000000000400010e0014000000000000000000 \
    OUTPUT="$TMP/countdown.tape" python3 -c '
import os
from pathlib import Path
actual = Path(os.environ["OUTPUT"]).read_bytes()
expected = bytes.fromhex(os.environ["EXPECTED"])
if actual != expected:
    raise SystemExit(f"output {actual.hex()}, expected {expected.hex()}")
'

LINES=$(COMPILER_SOURCE=$COMPILER_SOURCE python3 -c '
import os
from pathlib import Path
print(len(Path(os.environ["COMPILER_SOURCE"]).read_text().splitlines()))
')
echo "Gamma compiler reconstruction: $LINES-line source reproduced exact Beta receipt and $(wc -c < "$COMPILER_TAPE" | tr -d ' ')-byte compiler tape; direct comparator agrees on Delta0"