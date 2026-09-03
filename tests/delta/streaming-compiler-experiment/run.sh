#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Streaming Delta compiler experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
COMPILER="$GATE_DIR/compiler.gamma"
LOWERER="$OMEGA_REPO_ROOT/tests/gamma/gamma1-augmentation-experiment/lowerer.gamma"
RECURSIVE_SOURCE="$GATE_DIR/../functional-compiler-experiment/scalar_recursive.delta"
SURFACE_SOURCE="$GATE_DIR/../compiler-slice/scalar_surface.delta"

materialize_beta_compiler "$TMP/beta" >/dev/null
materialize_gamma_compiler "$TMP/gamma" >/dev/null
"$TMP/beta" < "$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null
compile_gamma_source_to_tape "$TMP/gamma" "$TMP/beta" \
    "$LOWERER" "$TMP/lowerer.tape"
stamp_seed "$TMP/lowerer.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/lowerer" >/dev/null
"$TMP/lowerer" < "$COMPILER" > "$TMP/compiler.gamma"
compile_gamma_source_to_tape "$TMP/gamma" "$TMP/beta" \
    "$TMP/compiler.gamma" "$TMP/compiler.tape"
stamp_seed "$TMP/compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/compiler" >/dev/null

"$TMP/compiler" < "$RECURSIVE_SOURCE" > "$TMP/recursive.gamma"
"$TMP/compiler" < "$SURFACE_SOURCE" > "$TMP/surface.gamma"

COMPILER_SOURCE="$COMPILER" COMPILER="$TMP/compiler.gamma" \
    COMPILER_TAPE="$TMP/compiler.tape" \
    EVALUATOR="$TMP/evaluator" NATIVE="$TMP/compiler" \
    RECURSIVE_SOURCE="$RECURSIVE_SOURCE" SURFACE_SOURCE="$SURFACE_SOURCE" \
    RECURSIVE_GAMMA="$TMP/recursive.gamma" SURFACE_GAMMA="$TMP/surface.gamma" \
    python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path

compiler_source = Path(os.environ["COMPILER_SOURCE"]).read_bytes()
compiler = Path(os.environ["COMPILER"]).read_bytes()
compiler_tape = Path(os.environ["COMPILER_TAPE"]).read_bytes()

if len(compiler_source.splitlines()) != 666 or len(compiler_source) != 27081:
    raise SystemExit("streaming Gamma1 compiler source identity changed")
if len(compiler.splitlines()) != 689 or len(compiler) != 29899:
    raise SystemExit("lowered streaming compiler identity changed")
if hashlib.sha256(compiler).hexdigest() != "f8a4a890838afc3d46d3bb23f92ee797eb5a0acd2530c926afb246bc721bba92":
    raise SystemExit("lowered streaming compiler hash changed")
if len(compiler_tape) != 22762:
    raise SystemExit("streaming compiler tape size changed")
if hashlib.sha256(compiler_tape).hexdigest() != "298066686ed17451af4772eb91d1e2468ddf16569ca798298feb4e99f37f4972":
    raise SystemExit("streaming compiler tape identity changed")

def run(executable, source):
    process = subprocess.Popen(
        [executable], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        output, _ = process.communicate(source, timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"{executable} timed out")
    return process.returncode, output

def interpreted(source):
    request = struct.pack("<I", len(compiler)) + compiler + source
    return run(os.environ["EVALUATOR"], request)

def native(source):
    return run(os.environ["NATIVE"], source)

for source_name, gamma_name in (
    ("RECURSIVE_SOURCE", "RECURSIVE_GAMMA"),
    ("SURFACE_SOURCE", "SURFACE_GAMMA"),
):
    source = Path(os.environ[source_name]).read_bytes()
    expected = Path(os.environ[gamma_name]).read_bytes()
    left = interpreted(source)
    right = native(source)
    if left != right or left != (0, expected):
        raise SystemExit(f"{source_name} interpreted/native disagreement")

malformed = {
    "missing main": b"(def other () Int 0)\n",
    "duplicate function": b"(def main () Int 0)\n(def main () Int 1)\n",
    "unknown local": b"(def main () Int missing)\n",
    "arity mismatch": b"(def id ((x Int)) Int x)\n(def main () Int (id))\n",
    "initializer self-reference": b"(def main () Int (let x Int x 0))\n",
}
for name, source in malformed.items():
    left = interpreted(source)
    right = native(source)
    if left != right or left != (2, b""):
        raise SystemExit(f"{name} was not rejected identically before output")

receipts = {
    "RECURSIVE_GAMMA": (1459, "3d74c10402d3287cc95ce4da32487a47a34354876d4468a2ce90eafce20e24b5"),
    "SURFACE_GAMMA": (4888, "d670e9fbd9d5482f8d8bf8839bffe3641f06fc7e8a90e9ea52cdca0a4872e29b"),
}
for name, (size, digest) in receipts.items():
    receipt = Path(os.environ[name]).read_bytes()
    if len(receipt) != size or hashlib.sha256(receipt).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")
PY

compile_gamma_source_to_tape "$TMP/gamma" "$TMP/beta" \
    "$TMP/recursive.gamma" "$TMP/recursive.tape"
compile_gamma_source_to_tape "$TMP/gamma" "$TMP/beta" \
    "$TMP/surface.gamma" "$TMP/surface.tape"
stamp_seed "$TMP/recursive.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/recursive" >/dev/null
stamp_seed "$TMP/surface.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/surface" >/dev/null

RECURSIVE="$TMP/recursive" SURFACE="$TMP/surface" \
    RECURSIVE_TAPE="$TMP/recursive.tape" SURFACE_TAPE="$TMP/surface.tape" \
    python3 - <<'PY'
import hashlib
import os
import signal
import subprocess
from pathlib import Path

def run(name):
    executable = os.environ[name]
    process = subprocess.Popen(
        [executable], stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit(f"{name} timed out")
    return process.returncode, output

if run("RECURSIVE") != (0, b"\x0f"):
    raise SystemExit("recursive receipt did not produce byte 15")
if run("SURFACE") != (0, b"\x15"):
    raise SystemExit("surface receipt did not produce byte 21")

for name, size, digest in (
    ("RECURSIVE_TAPE", 2554, "c49ea9d636d2f2de4c8d4bb9bf0cc45057d9df03c5b7db5b76d8f276ab3018d6"),
    ("SURFACE_TAPE", 6108, "1204cc19de3845be450183396116db5e0e7dbd3ee24da7711a96b79ab513122e"),
):
    tape = Path(os.environ[name]).read_bytes()
    if len(tape) != size or hashlib.sha256(tape).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")
PY

echo "Streaming Delta experiment: declaration-only rows, validation rescan, and direct emission passed"
