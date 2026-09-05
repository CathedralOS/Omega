#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Epsilon runtime references: skipped (python3 absent)"
    exit 0
}

REFERENCE_TMP=$(mktemp -d)
trap 'rm -rf -- "$REFERENCE_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$REFERENCE_TMP/epsilon_compiler.delta"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$REFERENCE_TMP/delta_compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$GATE_DIR/runtime_references.delta.sources" "$REFERENCE_TMP/controls.delta"
materialize_gamma_evaluator "$REFERENCE_TMP/evaluator" >/dev/null

GATE_DIR="$GATE_DIR" REFERENCE_TMP="$REFERENCE_TMP" python3 - <<'PY'
import csv
import hashlib
import os
import struct
import subprocess
from pathlib import Path

gate = Path(os.environ["GATE_DIR"])
temporary = Path(os.environ["REFERENCE_TMP"])
compiler = (temporary / "delta_compiler.gamma").read_bytes()
subject = ((temporary / "epsilon_compiler.delta").read_bytes()
           + (temporary / "controls.delta").read_bytes())
expected = bytes.fromhex((gate / "expected.hex").read_text(encoding="ascii"))
with (gate / "receipt.tsv").open(encoding="ascii", newline="") as manifest:
    rows = csv.DictReader(manifest, delimiter="\t")
    if rows.fieldnames != ["bytes", "sha256"]:
        raise SystemExit("runtime reference receipt header changed")
    identities = list(rows)
if len(identities) != 1:
    raise SystemExit("runtime reference receipt needs one exact identity")

def evaluate(program, sealed_input):
    process = subprocess.run(
        [str(temporary / "evaluator")],
        input=struct.pack("<I", len(program)) + program + sealed_input,
        stdout=subprocess.PIPE, timeout=300,
    )
    return process.returncode, process.stdout

request = b"DCREQ\x01\x00\x00" + struct.pack("<II", 1, len(subject)) + subject
status, receipt = evaluate(compiler, request)
digest = hashlib.sha256(receipt).hexdigest()
if (status != 0 or len(receipt) != int(identities[0]["bytes"])
        or digest != identities[0]["sha256"]):
    raise SystemExit(
        f"runtime reference receipt changed: status {status}, "
        f"{len(receipt)} bytes, SHA-256 {digest}"
    )
print(f"Epsilon runtime references: exact {len(receipt)}-byte receipt reconstructed",
      flush=True)
status, observation = evaluate(receipt, b"")
if (status, observation) != (0, expected):
    raise SystemExit(
        f"runtime references expected status 0 and {expected.hex()}, "
        f"received status {status} and {observation.hex()}"
    )
print("Epsilon runtime references: exact lookup and fallback controls pass", flush=True)
PY
