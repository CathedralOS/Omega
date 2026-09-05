#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta resource boundary: skipped (python3 absent)"
    exit 0
}

RESOURCE_BOUNDARY_TMP=$(mktemp -d)
trap 'rm -rf -- "$RESOURCE_BOUNDARY_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$RESOURCE_BOUNDARY_TMP/compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
materialize_gamma_evaluator "$RESOURCE_BOUNDARY_TMP/evaluator" >/dev/null

RESOURCE_BOUNDARY_TMP="$RESOURCE_BOUNDARY_TMP" GATE_DIR="$GATE_DIR" \
    PYTHONPATH="$GATE_DIR" python3 -B - <<'PY'
import csv
import hashlib
import os
import signal
import struct
import subprocess
import time
from pathlib import Path
from constructor_rows import fixtures as constructor_rows
from function_rows import fixtures as function_rows
from type_rows import fixtures as type_rows
from environment_rows import fixtures as environment_rows
from match_coverage import fixtures as match_coverage
from syntax_storage import fixtures as syntax_storage

directory = Path(os.environ["RESOURCE_BOUNDARY_TMP"])
compiler = (directory / "compiler.gamma").read_bytes()
with (Path(os.environ["GATE_DIR"]) / "compiler.tsv").open(newline="") as stream:
    rows = list(csv.DictReader(stream, delimiter="\t"))
if len(rows) != 1:
    raise SystemExit("Delta resource boundary: expected one canonical compiler identity")
identity = (len(compiler.splitlines()), len(compiler), hashlib.sha256(compiler).hexdigest())
expected_identity = (int(rows[0]["lines"]), int(rows[0]["bytes"]), rows[0]["sha256"])
if identity != expected_identity:
    raise SystemExit(f"Delta resource boundary compiler identity changed: {identity}")

function_cases = function_rows()
constructor_cases = constructor_rows()
type_cases = type_rows()
environment_cases = environment_rows()
coverage_cases = match_coverage()
syntax_cases = syntax_storage()
cases = (
    function_cases + constructor_cases + type_cases + environment_cases
    + coverage_cases + syntax_cases
)
for name, source, size, digest, expected in cases:
    if len(source) != size or hashlib.sha256(source).hexdigest() != digest:
        raise SystemExit(f"Delta resource boundary {name}: fixture identity changed")
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(source)) + source
    started = time.monotonic()
    process = subprocess.Popen(
        [str(directory / "evaluator")], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(
            struct.pack("<I", len(compiler)) + compiler + request, timeout=300
        )
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(
            f"Delta resource boundary {name}: selected Gamma timed out after "
            f"{time.monotonic() - started:.3f}s; no compiler resource judgment"
        )
    elapsed = time.monotonic() - started
    if error or (process.returncode, output) != expected:
        raise SystemExit(
            f"Delta resource boundary {name}: after {elapsed:.3f}s expected "
            f"{expected[0]}/{expected[1].hex()}, got raw status {process.returncode}, "
            f"{len(output)} stdout bytes, prefix {output[:80].hex()}, stderr={error!r}"
        )
    print(f"Delta resource boundary: {name}: exact DCOUT in {elapsed:.3f}s", flush=True)
print(
    f"Delta resource boundary: {len(cases)} exact observations passed "
    f"({len(function_cases)} function-row, {len(constructor_cases)} constructor-row, "
    f"{len(type_cases)} type-row, {len(environment_cases)} active-environment, "
    f"{len(coverage_cases)} match-coverage, {len(syntax_cases)} syntax-storage)"
)
PY
