#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Functional Delta compiler experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
COMPILER="$GATE_DIR/compiler.gamma"
CUSTOMER="$GATE_DIR/scalar_recursive.delta"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null
materialize_gamma_compiler "$TMP/gamma-compiler" >/dev/null
compile_gamma_source_to_tape "$TMP/gamma-compiler" "$TMP/beta-compiler" \
    "$COMPILER" "$TMP/delta-compiler.tape"

DELTA_COMPILER_TAPE="$TMP/delta-compiler.tape" python3 -c '
import hashlib
import os
from pathlib import Path

tape = Path(os.environ["DELTA_COMPILER_TAPE"]).read_bytes()
if len(tape) != 22214:
    raise SystemExit(f"scalar Functional Delta compiler is {len(tape)} bytes")
if hashlib.sha256(tape).hexdigest() != "cad8adf2652901325669be3569eb881f07c479b51540dabb50034f75749a96a7":
    raise SystemExit("scalar Functional Delta compiler identity changed")
'

stamp_seed "$TMP/delta-compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/delta-compiler" >/dev/null

COMPILER=$COMPILER CUSTOMER=$CUSTOMER EVALUATOR="$TMP/evaluator" \
    NATIVE_DELTA="$TMP/delta-compiler" TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

compiler = Path(os.environ["COMPILER"]).read_bytes()
customer = Path(os.environ["CUSTOMER"]).read_bytes()

def interpreted(source: bytes):
    request = struct.pack("<I", len(compiler)) + compiler + source
    return subprocess.run(
        [os.environ["EVALUATOR"]], input=request, stdout=subprocess.PIPE
    )

def native(source: bytes):
    return subprocess.run(
        [os.environ["NATIVE_DELTA"]], input=source, stdout=subprocess.PIPE
    )

left = interpreted(customer)
right = native(customer)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("recursive customer interpreted/native compilation disagrees")
if len(left.stdout) != 842:
    raise SystemExit(f"recursive customer tape is {len(left.stdout)} bytes")
if hashlib.sha256(left.stdout).hexdigest() != "9a085165e356fec02e3ab269ad22e4d5659e0a8bfaae34db77e3e811c87e5f2f":
    raise SystemExit("recursive customer tape identity changed")
Path(os.environ["TMP"], "customer.tape").write_bytes(left.stdout)

scalar = b"""(def zero () Int 0)
(def scalar ((value Int)) Int
    (if (lt (% (/ (* value 5) 4) 4) 4) 17 19))
(def main () Int (scalar 6))
"""
left = interpreted(scalar)
right = native(scalar)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit(
        "scalar matrix interpreted/native compilation disagrees: "
        f"statuses {left.returncode}/{right.returncode}, "
        f"sizes {len(left.stdout)}/{len(right.stdout)}"
    )
Path(os.environ["TMP"], "scalar.tape").write_bytes(left.stdout)

parameters = " ".join(f"(p{index} Int)" for index in range(13))
arguments = " ".join(str(index + 1) for index in range(13))
max_arity = (
    f"(def select ({parameters}) Int p12)\n"
    f"(def main () Int (select {arguments}))\n"
).encode("ascii")
left = interpreted(max_arity)
right = native(max_arity)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("maximum arity interpreted/native compilation disagrees")
Path(os.environ["TMP"], "max-arity.tape").write_bytes(left.stdout)

malformed = {
    "missing-main": b"(def other () Int 0)\n",
    "duplicate-function": b"(def main () Int 0)\n(def main () Int 1)\n",
    "unknown-local": b"(def main () Int missing)\n",
    "arity-mismatch": b"(def id ((x Int)) Int x)\n(def main () Int (id))\n",
    "non-int-type": b"(def main () Bytes 0)\n",
    "arity-above-thirteen": (
        f"(def too_many ({parameters} (extra Int)) Int extra)\n"
        "(def main () Int 0)\n"
    ).encode("ascii"),
}
for name, source in malformed.items():
    left = interpreted(source)
    right = native(source)
    if left.returncode != 2 or right.returncode != 2 or left.stdout != right.stdout:
        raise SystemExit(f"{name} was not rejected identically")
'

stamp_seed "$TMP/customer.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/customer" >/dev/null
set +e
"$TMP/customer"
CUSTOMER_STATUS=$?
set -e
[ "$CUSTOMER_STATUS" -eq 15 ]

stamp_seed "$TMP/scalar.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar" >/dev/null
set +e
"$TMP/scalar"
SCALAR_STATUS=$?
set -e
[ "$SCALAR_STATUS" -eq 17 ]

stamp_seed "$TMP/max-arity.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/max-arity" >/dev/null
set +e
"$TMP/max-arity"
MAX_ARITY_STATUS=$?
set -e
[ "$MAX_ARITY_STATUS" -eq 13 ]

echo "Functional Delta compiler experiment: 565-line Gamma compiler produced identical 22,214-byte native compiler; 9-line recursive customer compiled to 842 bytes and exited 15"