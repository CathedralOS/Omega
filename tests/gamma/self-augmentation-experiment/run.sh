#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma self-augmentation: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
AUGMENTER="$GATE_DIR/constant_augmenter.gamma"
SOURCE="$GATE_DIR/program.gamma1"
EXPECTED="$GATE_DIR/program.gamma"

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null

EVALUATOR="$TMP/evaluator" AUGMENTER="$AUGMENTER" SOURCE="$SOURCE" \
    EXPECTED="$EXPECTED" python3 - <<'PY'
import os
import signal
import struct
import subprocess
from pathlib import Path


def evaluate(source, sealed_input=b""):
    request = struct.pack("<I", len(source)) + source + sealed_input
    process = subprocess.Popen(
        [os.environ["EVALUATOR"]], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(request, timeout=20)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("Gamma evaluation timed out")
    return process.returncode, output

augmenter = Path(os.environ["AUGMENTER"]).read_bytes()
source = Path(os.environ["SOURCE"]).read_bytes()
expected = Path(os.environ["EXPECTED"]).read_bytes()
if evaluate(augmenter, source) != (0, expected):
    raise SystemExit("Gamma augmenter did not produce the exact receipt")
if evaluate(expected) != (0, b"*"):
    raise SystemExit("augmented Gamma program did not produce 42")
PY

echo "Gamma self-augmentation: const receipt and result 42 passed"
