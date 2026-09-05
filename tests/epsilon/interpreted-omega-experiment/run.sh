#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
EPSILON="$OMEGA_REPO_ROOT/source/epsilon/compiler/epsilon_compiler.delta"
OMEGA_D_SOURCES="$OMEGA_REPO_ROOT/source/omega/omega_compiler.epsilon.sources"
OMEGA_D_MATERIALIZER="$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/materialize_source_closure.py"
OMEGA_BUILD="$OMEGA_REPO_ROOT/source/omega/build.omg"
DELTA="$OMEGA_REPO_ROOT/source/delta/compiler/delta_compiler.gamma"
DRIVER="$TEST_DIR/empty_entry_driver.delta"
FIXTURE="$TEST_DIR/empty_entry.epsilon"
WRITE_EXIT="$TEST_DIR/write_exit.epsilon"
BYTE_RANGE="$TEST_DIR/byte_range.epsilon"
LET_EXIT="$TEST_DIR/let_exit.epsilon"
ASSERTION="$TEST_DIR/assertion.epsilon"
NONBOOLEAN="$TEST_DIR/nonboolean.epsilon"
CORE_ARITHMETIC="$TEST_DIR/core_arithmetic.epsilon"
ADD_OVERFLOW="$TEST_DIR/add_overflow.epsilon"
MULTIPLY_OVERFLOW="$TEST_DIR/multiply_overflow.epsilon"
NEGATE_OVERFLOW="$TEST_DIR/negate_overflow.epsilon"
FULL_SCALAR="$TEST_DIR/full_scalar.epsilon"
DIVISION_ZERO="$TEST_DIR/division_zero.epsilon"
DIVISION_OVERFLOW="$TEST_DIR/division_overflow.epsilon"
SHIFT_COUNT="$TEST_DIR/shift_count.epsilon"
SHORT_CIRCUIT="$TEST_DIR/short_circuit.epsilon"
SCALAR_FIELD="$TEST_DIR/scalar_field.epsilon"
FIXED_ARRAY="$TEST_DIR/fixed_array.epsilon"
BOUNDS_READ="$TEST_DIR/bounds_read.epsilon"
BOUNDS_WRITE="$TEST_DIR/bounds_write.epsilon"

command -v python3 >/dev/null 2>&1 || {
    echo "Interpreted Omega experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM

if grep -Eq 'EpsilonAlpha|epsilon_alpha_' "$EPSILON"; then
    echo "Interpreted Omega experiment: Epsilon still owns Alpha encoding" >&2
    exit 1
fi

[ "$(grep -Fc 'builder.roots.bind(alpha_bootstrap::ProgramEntry, Main::main);' "$OMEGA_BUILD")" -eq 1 ] || {
    echo "Interpreted Omega experiment: alpha_bootstrap is not one ordinary root" >&2
    exit 1
}

python3 "$OMEGA_D_MATERIALIZER" "$OMEGA_D_SOURCES" "$TMP/omega_compiler.epsilon"
grep -F 'data AlphaTapeBuffer {' "$TMP/omega_compiler.epsilon" >/dev/null || {
    echo "Interpreted Omega experiment: Omega D does not own Alpha tape construction" >&2
    exit 1
}

EPSILON_LINES=$(wc -l < "$EPSILON" | tr -d ' ')
EPSILON_BYTES=$(wc -c < "$EPSILON" | tr -d ' ')
[ "$EPSILON_LINES" -eq 9536 ]
[ "$EPSILON_BYTES" -eq 475537 ]

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EPSILON="$EPSILON" DELTA="$DELTA" DRIVER="$DRIVER" FIXTURE="$FIXTURE" \
    WRITE_EXIT="$WRITE_EXIT" BYTE_RANGE="$BYTE_RANGE" \
    LET_EXIT="$LET_EXIT" ASSERTION="$ASSERTION" NONBOOLEAN="$NONBOOLEAN" \
    CORE_ARITHMETIC="$CORE_ARITHMETIC" ADD_OVERFLOW="$ADD_OVERFLOW" \
    MULTIPLY_OVERFLOW="$MULTIPLY_OVERFLOW" \
    NEGATE_OVERFLOW="$NEGATE_OVERFLOW" \
    FULL_SCALAR="$FULL_SCALAR" DIVISION_ZERO="$DIVISION_ZERO" \
    DIVISION_OVERFLOW="$DIVISION_OVERFLOW" SHIFT_COUNT="$SHIFT_COUNT" \
    SHORT_CIRCUIT="$SHORT_CIRCUIT" \
    SCALAR_FIELD="$SCALAR_FIELD" \
    FIXED_ARRAY="$FIXED_ARRAY" BOUNDS_READ="$BOUNDS_READ" \
    BOUNDS_WRITE="$BOUNDS_WRITE" \
    EVALUATOR="$TMP/evaluator" python3 - <<'PY'
import hashlib
import os
import struct
import subprocess
from pathlib import Path

artifacts = {
    "evaluator source": (
        Path(os.environ["EPSILON"]).read_bytes(),
        475537,
        "f67a7fb6cf8806423e4f84c211894675ab5aae3be143d47f27c01fb117699a83",
    ),
    "slice driver": (
        Path(os.environ["DRIVER"]).read_bytes(),
        485,
        "2842b404a9a4f6d98ebf1e37377af18ef1e9dd5244d860a225b631adb19dabfd",
    ),
    "empty entry": (
        Path(os.environ["FIXTURE"]).read_bytes(),
        245,
        "48daba569ebc62f31dabe8a11c61e381a6fc6dc38726099b70554edd79c77899",
    ),
    "write then exit": (
        Path(os.environ["WRITE_EXIT"]).read_bytes(),
        309,
        "772d46bbf55e905229d8a1fb42847d37c3e303f8d50cb7badaf7d54387c9a2b3",
    ),
    "byte range": (
        Path(os.environ["BYTE_RANGE"]).read_bytes(),
        309,
        "9dd6f6fd2697de619b1cf7d86b78ab2d416eb33dd255d2de2a5dc822dd5bb037",
    ),
    "let then exit": (
        Path(os.environ["LET_EXIT"]).read_bytes(),
        349,
        "4d6f63fcd7892bc51d9594f2a674edff830e1bd48b23f8e00bc54f45c7ffe3c6",
    ),
    "assertion": (
        Path(os.environ["ASSERTION"]).read_bytes(),
        293,
        "f6c889a5cbb9acbaa8d31062ef946669db3d033eb5fab2f02dce4cf8b46b02ed",
    ),
    "nonboolean": (
        Path(os.environ["NONBOOLEAN"]).read_bytes(),
        258,
        "d4a855e7f6e7e60dc5d8f19c58ed0fcac284fae07ff52da1050980275b3ce5ea",
    ),
    "core arithmetic": (
        Path(os.environ["CORE_ARITHMETIC"]).read_bytes(),
        361,
        "bf50b1cfc161be22ec97c2868ef0ecbb178c839751b9556576e6f43bf58715d4",
    ),
    "add overflow": (
        Path(os.environ["ADD_OVERFLOW"]).read_bytes(),
        312,
        "9885dc8d65f2aba143803098cff97b00113367a0bfe5fe4ecd3ff53a65cc8c2d",
    ),
    "multiply overflow": (
        Path(os.environ["MULTIPLY_OVERFLOW"]).read_bytes(),
        280,
        "09baf86e69fd52bd54e8c84ec68bb66e3333f2ff8e32663f32d0f71fc8312d57",
    ),
    "negate overflow": (
        Path(os.environ["NEGATE_OVERFLOW"]).read_bytes(),
        281,
        "ecca508ce5e8851a140c8d641179bfafb7a7a7e8afb2fc97d4b8173d6144eea1",
    ),
    "full scalar": (
        Path(os.environ["FULL_SCALAR"]).read_bytes(),
        457,
        "7e65a18289c21d4d6e2fc0c16cf1ccbbec5c174830a13950bb9098f7c1044c77",
    ),
    "division zero": (
        Path(os.environ["DIVISION_ZERO"]).read_bytes(),
        272,
        "0e48f7cfc98585bcf0d388c410a6e07f9c884ec65ec6e071ea7c2f4beb49807f",
    ),
    "division overflow": (
        Path(os.environ["DIVISION_OVERFLOW"]).read_bytes(),
        283,
        "b9a5dc30e6a15697ba330ff6126e3da4b39fa15a549977d0820e1a0ab89cacd4",
    ),
    "shift count": (
        Path(os.environ["SHIFT_COUNT"]).read_bytes(),
        274,
        "16047d495435f581e186efe19594becedf032c15ed5f9315f63f2d4e1d436da8",
    ),
    "short circuit": (
        Path(os.environ["SHORT_CIRCUIT"]).read_bytes(),
        320,
        "3ec652c83dc00bf125412c33b46af3b18c6824a42fc7e5d43f4bb48704b60255",
    ),
    "scalar field": (
        Path(os.environ["SCALAR_FIELD"]).read_bytes(),
        432,
        "e414d325215dc544a4cfd9307b914fe5fe631f602545eea9341e86727b428d82",
    ),
    "fixed array": (
        Path(os.environ["FIXED_ARRAY"]).read_bytes(),
        540,
        "6a984a751b0243b8802d47be45ea5841dc2e5d23640bdbf598bdb6bf6bf9566a",
    ),
    "bounds read": (
        Path(os.environ["BOUNDS_READ"]).read_bytes(),
        333,
        "ffa640cf78dede53e825b2ba7d78b2c322b88262331f9ae329974d2221f67365",
    ),
    "bounds write": (
        Path(os.environ["BOUNDS_WRITE"]).read_bytes(),
        334,
        "45c3b25344e9bcb9622f094477c50a0a0b517c3bfb6cb43c04e74b6d2a80183c",
    ),
}
for name, (data, size, digest) in artifacts.items():
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

compiler = Path(os.environ["DELTA"]).read_bytes()
subject = artifacts["evaluator source"][0] + artifacts["slice driver"][0]
request = (
    b"DCREQ\x01\x00\x00"
    + struct.pack("<I", 1)
    + struct.pack("<I", len(subject))
    + subject
)

def evaluate(program, sealed_input=b"", timeout=300):
    framed = struct.pack("<I", len(program)) + program + sealed_input
    process = subprocess.run(
        [os.environ["EVALUATOR"]], input=framed, stdout=subprocess.PIPE,
        timeout=timeout,
    )
    return process.returncode, process.stdout

status, receipt = evaluate(compiler, request)
if status != 0 or len(receipt) != 561448:
    raise SystemExit(
        f"fixed-array evaluator slice returned {status} with {len(receipt)} bytes"
    )
if hashlib.sha256(receipt).hexdigest() != (
    "ae333f8be4296c59e99db27e7cce60d5e36f4f817f5fb57d8c1f8b1a372154f7"
):
    raise SystemExit(
        "fixed-array evaluator receipt identity changed to "
        + hashlib.sha256(receipt).hexdigest()
    )
controls = {
    "empty entry": b"\x00",
    "write then exit": b"A\x07",
    "byte range": b"A\x85",
    "let then exit": b"A\x07",
    "assertion": b"A\x88",
    "nonboolean": b"\x87",
    "core arithmetic": b"A\x00",
    "add overflow": b"A\x81",
    "multiply overflow": b"\x81",
    "negate overflow": b"\x81",
    "full scalar": b"A\x00",
    "division zero": b"\x82",
    "division overflow": b"\x83",
    "shift count": b"\x84",
    "short circuit": b"\x00",
    "scalar field": b"A\x00",
    "fixed array": b"A\x00",
    "bounds read": b"A\x86",
    "bounds write": b"A\x86",
}
for name, expected in controls.items():
    if evaluate(receipt, artifacts[name][0], timeout=120) != (0, expected):
        raise SystemExit(f"{name} did not produce its exact observation")
PY

echo "Interpreted Omega experiment: fixed arrays, scalar fields, locals, and Console execution pass"
