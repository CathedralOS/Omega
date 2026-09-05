#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta lowering plan: skipped (python3 absent)"
    exit 0
}

LOWERING_PLAN_TMP=$(mktemp -d)
trap 'rm -rf -- "$LOWERING_PLAN_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$LOWERING_PLAN_TMP/compiler.gamma" \
    --prefix "$GATE_DIR/height_driver.gamma"
materialize_gamma_evaluator "$LOWERING_PLAN_TMP/evaluator" >/dev/null

LOWERING_PLAN_TMP="$LOWERING_PLAN_TMP" GATE_DIR="$GATE_DIR" \
    PYTHONPATH="$GATE_DIR" python3 -B - <<'PY'
import csv
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path
from fixtures import fixtures

directory = Path(os.environ["LOWERING_PLAN_TMP"])
compiler = (directory / "compiler.gamma").read_bytes()
with (Path(os.environ["GATE_DIR"]) / "compiler.tsv").open(newline="") as stream:
    rows = list(csv.DictReader(stream, delimiter="\t"))
if len(rows) != 1:
    raise SystemExit("Delta lowering plan: expected one exact compiler identity")
expected_identity = (int(rows[0]["lines"]), int(rows[0]["bytes"]), rows[0]["sha256"])
actual_identity = (len(compiler.splitlines()), len(compiler), hashlib.sha256(compiler).hexdigest())
if actual_identity != expected_identity:
    raise SystemExit(f"Delta lowering plan compiler identity changed: {actual_identity}")

cases = fixtures()
for name, source, heights in cases:
    process = subprocess.Popen(
        [str(directory / "evaluator")], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(
            struct.pack("<I", len(compiler)) + compiler + source, timeout=30
        )
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"Delta lowering plan {name}: selected Gamma timed out")
    expected = b"".join(struct.pack("<I", height) for height in heights) + b"\x00"
    if process.returncode != 0 or error or output != expected:
        raise SystemExit(
            f"Delta lowering plan {name}: expected 0/{expected.hex()}, "
            f"got {process.returncode}/{output.hex()}, stderr={error!r}"
        )
print(f"Delta lowering plan: {len(cases)} exact expanded-height observations passed")
PY
