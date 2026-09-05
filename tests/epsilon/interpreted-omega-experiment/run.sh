#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
EPSILON="$OMEGA_REPO_ROOT/bootstrap/epsilon/compiler/epsilon_compiler.delta"
OMEGA_D_SOURCES="$OMEGA_REPO_ROOT/bootstrap/omega/omega_compiler.epsilon.sources"
OMEGA_D_MATERIALIZER="$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/materialize_source_closure.py"
OMEGA_BUILD="$OMEGA_REPO_ROOT/source/omega/build.omg"
DELTA="$OMEGA_REPO_ROOT/bootstrap/delta/compiler/delta_compiler.gamma"
DRIVER="$TEST_DIR/empty_entry_driver.delta"

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
[ "$EPSILON_LINES" -eq 9566 ]
[ "$EPSILON_BYTES" -eq 477364 ]

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EPSILON="$EPSILON" DELTA="$DELTA" DRIVER="$DRIVER" TEST_DIR="$TEST_DIR" \
    EVALUATOR="$TMP/evaluator" python3 - <<'PY'
import csv
import hashlib
import os
import struct
import subprocess
from pathlib import Path

artifacts = {
    "evaluator source": (
        Path(os.environ["EPSILON"]).read_bytes(),
        477364,
        "d37d153743a74ecc0b496c8467b7b438e52f16a7ee4e91a8eee2daf0211960de",
    ),
    "slice driver": (
        Path(os.environ["DRIVER"]).read_bytes(),
        485,
        "2842b404a9a4f6d98ebf1e37377af18ef1e9dd5244d860a225b631adb19dabfd",
    ),
}
for name, (data, size, digest) in artifacts.items():
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

test_directory = Path(os.environ["TEST_DIR"])
controls = {}
with (test_directory / "fixtures.tsv").open(encoding="ascii", newline="") as manifest:
    rows = csv.DictReader(manifest, delimiter="\t")
    if rows.fieldnames != ["fixture", "bytes", "sha256", "expected_hex"]:
        raise SystemExit("fixture manifest header changed")
    for row in rows:
        name = row["fixture"]
        if Path(name).name != name or not name.endswith(".epsilon") or name in controls:
            raise SystemExit(f"invalid or repeated fixture identity: {name}")
        data = (test_directory / name).read_bytes()
        if len(data) != int(row["bytes"]) or hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise SystemExit(f"{name} identity changed")
        controls[name] = (data, bytes.fromhex(row["expected_hex"]))
if set(controls) != {path.name for path in test_directory.glob("*.epsilon")}:
    raise SystemExit("fixture manifest does not cover the exact Epsilon fixture inventory")

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
if status != 0 or len(receipt) != 563256:
    raise SystemExit(
        f"scalar-storage evaluator slice returned {status} with {len(receipt)} bytes "
        f"and SHA-256 {hashlib.sha256(receipt).hexdigest()}"
    )
if hashlib.sha256(receipt).hexdigest() != (
    "e31996b88043eeacc263659336c2643ffa3800883675285aab3c1305974c4598"
):
    raise SystemExit(
        "scalar-storage evaluator receipt identity changed to "
        + hashlib.sha256(receipt).hexdigest()
    )
print(f"Epsilon evaluator: exact {len(receipt)}-byte receipt reconstructed", flush=True)
for name, (source, expected) in controls.items():
    status, observation = evaluate(receipt, source, timeout=120)
    if (status, observation) != (0, expected):
        raise SystemExit(
            f"{name}: expected status 0 and {expected.hex()}, "
            f"received status {status} and {observation.hex()}"
        )
print(f"Epsilon execution: {len(controls)} exact observations pass", flush=True)
PY

echo "Interpreted Omega experiment: fixed arrays, scalar fields, locals, and Console execution pass"
