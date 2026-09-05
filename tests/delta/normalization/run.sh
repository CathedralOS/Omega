#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta normalization: skipped (python3 absent)"
    exit 0
}

NORMALIZATION_TMP=$(mktemp -d)
trap 'rm -rf -- "$NORMALIZATION_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$NORMALIZATION_TMP/canonical.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$NORMALIZATION_TMP/diagnostic.gamma" \
    --prefix "$GATE_DIR/normalization_driver.gamma"
materialize_gamma_evaluator "$NORMALIZATION_TMP/evaluator" >/dev/null

NORMALIZATION_TMP="$NORMALIZATION_TMP" GATE_DIR="$GATE_DIR" \
    PYTHONPATH="$GATE_DIR" python3 -B - <<'PY'
import csv
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path
from fixtures import PAYLOAD, fixtures

directory = Path(os.environ["NORMALIZATION_TMP"])
with (Path(os.environ["GATE_DIR"]) / "compiler.tsv").open(newline="") as stream:
    rows = list(csv.DictReader(stream, delimiter="\t"))
if [row["name"] for row in rows] != ["canonical", "diagnostic"]:
    raise SystemExit("Delta normalization: expected canonical and diagnostic identities")
programs = {}
for row in rows:
    program = (directory / (row["name"] + ".gamma")).read_bytes()
    actual = (len(program.splitlines()), len(program), hashlib.sha256(program).hexdigest())
    expected = (int(row["lines"]), int(row["bytes"]), row["sha256"])
    if actual != expected:
        raise SystemExit(f"Delta normalization {row['name']} identity changed: {actual}")
    programs[row["name"]] = program


def evaluate(name, program, sealed_input):
    process = subprocess.Popen(
        [str(directory / "evaluator")], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input, timeout=30
        )
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"Delta normalization {name}: selected Gamma timed out")
    if error:
        raise SystemExit(f"Delta normalization {name}: stderr={error!r}")
    return process.returncode, output


cases = fixtures()
for name, source, status, output, helpers, count, maximum, digest, capture_maximum in cases:
    diagnostic_status, diagnostic = evaluate(name, programs["diagnostic"], source)
    if diagnostic_status != 0 or len(diagnostic) != 21 or diagnostic[-1:] != b"\x00":
        raise SystemExit(f"{name}: malformed normalization diagnostic {diagnostic_status}/{diagnostic.hex()}")
    original_count, original_height, normalized_count, normalized_height, parameters = struct.unpack(
        "<IIIII", diagnostic[:20]
    )
    if original_count != count or normalized_count < count or normalized_height > 255:
        raise SystemExit(f"{name}: incorrect counts/heights {diagnostic.hex()}")
    if helpers != (normalized_count > count) or helpers != (original_height > 255):
        raise SystemExit(f"{name}: incorrect helper extraction {diagnostic.hex()}")
    if maximum is not None and (original_height, normalized_height) != (maximum, maximum):
        raise SystemExit(f"{name}: exact boundary height changed {diagnostic.hex()}")
    if capture_maximum is not None and parameters != capture_maximum:
        raise SystemExit(f"{name}: repeated free binding was not captured exactly once: {parameters}")
    request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(source)) + source
    compiled, receipt = evaluate(name, programs["canonical"], request)
    if compiled != 0 or not receipt:
        raise SystemExit(f"{name}: compilation failed {compiled}/{receipt[:80].hex()}")
    if digest is not None and hashlib.sha256(receipt).hexdigest() != digest:
        raise SystemExit(f"{name}: fitting complete receipt changed")
    if evaluate(name, programs["canonical"], request) != (0, receipt):
        raise SystemExit(f"{name}: repeated compilation changed bytes")
    actual = evaluate(name, receipt, PAYLOAD)
    if actual != (status, output):
        raise SystemExit(
            f"{name}: expected application {status}/{output.hex()}, "
            f"got {actual[0]}/{actual[1].hex()}"
        )
print(
    f"Delta normalization: {len(cases)} source-owned height/helper observations, "
    f"{len(cases)} repeated compilations, and {len(cases)} application observations passed"
)
PY
