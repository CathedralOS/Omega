#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
EPSILON="$OMEGA_REPO_ROOT/source/epsilon/compiler/epsilon_compiler.delta"
OMEGA_D="$OMEGA_REPO_ROOT/source/omega/omega_compiler.epsilon"
OMEGA_BUILD="$OMEGA_REPO_ROOT/source/omega/build.omg"
DELTA="$OMEGA_REPO_ROOT/source/delta/compiler/delta_compiler.gamma"
DRIVER="$TEST_DIR/empty_entry_driver.delta"
FIXTURE="$TEST_DIR/empty_entry.epsilon"
WRITE_EXIT="$TEST_DIR/write_exit.epsilon"
BYTE_RANGE="$TEST_DIR/byte_range.epsilon"
LET_EXIT="$TEST_DIR/let_exit.epsilon"
ASSERTION="$TEST_DIR/assertion.epsilon"
NONBOOLEAN="$TEST_DIR/nonboolean.epsilon"

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

grep -F 'data AlphaTapeBuffer {' "$OMEGA_D" >/dev/null || {
    echo "Interpreted Omega experiment: Omega D does not own Alpha tape construction" >&2
    exit 1
}

EPSILON_LINES=$(wc -l < "$EPSILON" | tr -d ' ')
EPSILON_BYTES=$(wc -c < "$EPSILON" | tr -d ' ')
[ "$EPSILON_LINES" -eq 8863 ]
[ "$EPSILON_BYTES" -eq 441260 ]

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EPSILON="$EPSILON" DELTA="$DELTA" DRIVER="$DRIVER" FIXTURE="$FIXTURE" \
    WRITE_EXIT="$WRITE_EXIT" BYTE_RANGE="$BYTE_RANGE" \
    LET_EXIT="$LET_EXIT" ASSERTION="$ASSERTION" NONBOOLEAN="$NONBOOLEAN" \
    EVALUATOR="$TMP/evaluator" python3 - <<'PY'
import hashlib
import os
import struct
import subprocess
from pathlib import Path

artifacts = {
    "evaluator source": (
        Path(os.environ["EPSILON"]).read_bytes(),
        441260,
        "33beb656e128e4dad9428bae27276a75942af43433234949baff949f73371c38",
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
if status != 0 or len(receipt) != 520260:
    raise SystemExit("scalar local evaluator slice did not compile")
if hashlib.sha256(receipt).hexdigest() != (
    "899820f58d08cf4e4d73313e2012ee6e166769f03c64c4b8a056330c9a3f4577"
):
    raise SystemExit("scalar local evaluator receipt identity changed")
controls = {
    "empty entry": b"\x00",
    "write then exit": b"A\x07",
    "byte range": b"A\x85",
    "let then exit": b"A\x07",
    "assertion": b"A\x88",
    "nonboolean": b"\x87",
}
for name, expected in controls.items():
    if evaluate(receipt, artifacts[name][0], timeout=120) != (0, expected):
        raise SystemExit(f"{name} did not produce its exact observation")
PY

echo "Interpreted Omega experiment: alpha_bootstrap, scalar locals, and Console execution pass"
