#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma1 augmentation experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
LOWERER="$GATE_DIR/lowerer.gamma"
SOURCE="$GATE_DIR/surface.gamma1"
EXPECTED="$GATE_DIR/surface.gamma"

materialize_beta_compiler "$TMP/beta" >/dev/null
materialize_gamma_compiler "$TMP/gamma" >/dev/null
"$TMP/beta" < "$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null
compile_gamma_source_to_tape "$TMP/gamma" "$TMP/beta" \
    "$LOWERER" "$TMP/lowerer.tape"
stamp_seed "$TMP/lowerer.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/lowerer" >/dev/null

LOWERER="$LOWERER" SOURCE="$SOURCE" EXPECTED="$EXPECTED" \
    EVALUATOR="$TMP/evaluator" NATIVE="$TMP/lowerer" \
    LOWERER_TAPE="$TMP/lowerer.tape" python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path

lowerer = Path(os.environ["LOWERER"]).read_bytes()
source = Path(os.environ["SOURCE"]).read_bytes()
expected = Path(os.environ["EXPECTED"]).read_bytes()
tape = Path(os.environ["LOWERER_TAPE"]).read_bytes()

if len(lowerer.splitlines()) != 193 or len(lowerer) != 6254:
    raise SystemExit("Gamma1 lowerer source identity changed")
if len(tape) != 6259:
    raise SystemExit("Gamma1 lowerer tape size changed")
if hashlib.sha256(tape).hexdigest() != "fdf05fbeeeed77ed462204ff2e1442c3461ed710cacf5c0359f76bd12810659a":
    raise SystemExit("Gamma1 lowerer tape identity changed")

def run(executable, data):
    process = subprocess.Popen(
        [executable], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        output, _ = process.communicate(data, timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"{executable} timed out")
    return process.returncode, output

request = struct.pack("<I", len(lowerer)) + lowerer + source
interpreted = run(os.environ["EVALUATOR"], request)
native = run(os.environ["NATIVE"], source)
if interpreted != native or native != (0, expected):
    raise SystemExit("Gamma1 interpreted/native lowering disagrees with receipt")
PY

"$TMP/lowerer" < "$SOURCE" > "$TMP/surface.gamma"
compile_gamma_source_to_tape "$TMP/gamma" "$TMP/beta" \
    "$TMP/surface.gamma" "$TMP/surface.tape"
stamp_seed "$TMP/surface.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/surface" >/dev/null

OUTPUT=$("$TMP/surface")
[ "$OUTPUT" = "frame" ]
echo "Gamma1 augmentation experiment: named cells, constants, and exact text lowered"
