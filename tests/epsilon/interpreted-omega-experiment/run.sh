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
[ "$EPSILON_LINES" -eq 8658 ]
[ "$EPSILON_BYTES" -eq 430747 ]

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EPSILON="$EPSILON" DELTA="$DELTA" DRIVER="$DRIVER" FIXTURE="$FIXTURE" \
    EVALUATOR="$TMP/evaluator" python3 - <<'PY'
import hashlib
import os
import struct
import subprocess
from pathlib import Path

artifacts = {
    "evaluator source": (
        Path(os.environ["EPSILON"]).read_bytes(),
        430747,
        "92a5b0a246da317eec98ba3990500b7b2b213c693e746a3fd167df6f47e9ae6c",
    ),
    "slice driver": (
        Path(os.environ["DRIVER"]).read_bytes(),
        386,
        "cec74f37e0fc5957c74cbf4af4df4ae160b326c781c49d8f4c5f6902cdb61141",
    ),
    "empty entry": (
        Path(os.environ["FIXTURE"]).read_bytes(),
        245,
        "48daba569ebc62f31dabe8a11c61e381a6fc6dc38726099b70554edd79c77899",
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
if status != 0 or len(receipt) != 507153:
    raise SystemExit("empty-entry evaluator slice did not compile")
if hashlib.sha256(receipt).hexdigest() != (
    "dc9a4fd06045e49fc5d7e520d675c60962298569c2c9c2373e86e8a859ebee6f"
):
    raise SystemExit("empty-entry evaluator receipt identity changed")
if evaluate(receipt, artifacts["empty entry"][0], timeout=120) != (0, b"\x07"):
    raise SystemExit("accepted empty Main entry did not execute as exit zero")
PY

echo "Interpreted Omega experiment: alpha_bootstrap ownership and empty Epsilon entry execution pass"
