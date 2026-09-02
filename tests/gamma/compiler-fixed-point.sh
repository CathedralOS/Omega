#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma compiler fixed point: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
COMPILER_SOURCE="$OMEGA_PATH_GAMMA/compiler/gamma_compiler.gamma"
COMPILER_TAPE="$OMEGA_PATH_GAMMA/compiler/gamma_compiler_bytecode.tape"
DELTA0_SOURCE="$GATE_DIR/fixtures/delta0_compiler.gamma"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null

PROGRAM=$COMPILER_SOURCE SUBJECT=$COMPILER_SOURCE python3 -c '
import os, struct, sys
from pathlib import Path
program = Path(os.environ["PROGRAM"]).read_bytes()
subject = Path(os.environ["SUBJECT"]).read_bytes()
sys.stdout.buffer.write(struct.pack("<I", len(program)) + program + subject)
' > "$TMP/bootstrap-request"
"$TMP/evaluator" < "$TMP/bootstrap-request" > "$TMP/t0.tape"
cmp "$COMPILER_TAPE" "$TMP/t0.tape"

stamp_seed "$COMPILER_TAPE" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/compiler" >/dev/null
"$TMP/compiler" < "$COMPILER_SOURCE" > "$TMP/t1.tape"
cmp "$COMPILER_TAPE" "$TMP/t1.tape"

PROGRAM=$COMPILER_SOURCE SUBJECT=$DELTA0_SOURCE python3 -c '
import os, struct, sys
from pathlib import Path
program = Path(os.environ["PROGRAM"]).read_bytes()
subject = Path(os.environ["SUBJECT"]).read_bytes()
sys.stdout.buffer.write(struct.pack("<I", len(program)) + program + subject)
' > "$TMP/delta0-request"
"$TMP/evaluator" < "$TMP/delta0-request" > "$TMP/delta0-seeded.tape"
"$TMP/compiler" < "$DELTA0_SOURCE" > "$TMP/delta0-native.tape"
cmp "$TMP/delta0-seeded.tape" "$TMP/delta0-native.tape"

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
echo "Gamma compiler fixed point: $LINES-line source reproduced exact $(wc -c < "$COMPILER_TAPE" | tr -d ' ')-byte compiler tape; Delta0 outputs agree"