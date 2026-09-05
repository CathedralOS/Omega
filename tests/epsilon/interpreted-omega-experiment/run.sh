#!/usr/bin/env sh
set -eu

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
SOURCE_CLOSURE_MATERIALIZER="$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py"
OMEGA_BUILD="$OMEGA_PATH_OMEGA/build.omg"
DELTA="$OMEGA_PATH_DELTA_COMPILER_SOURCE"
DRIVER="$TEST_DIR/empty_entry_driver.delta"

command -v python3 >/dev/null 2>&1 || {
    echo "Interpreted Omega experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
EPSILON="$TMP/epsilon_compiler.delta"
python3 "$SOURCE_CLOSURE_MATERIALIZER" "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON"

if grep -Eq 'EpsilonAlpha|epsilon_alpha_' "$EPSILON"; then
    echo "Interpreted Omega experiment: Epsilon still owns Alpha encoding" >&2
    exit 1
fi

[ "$(grep -Fc 'builder.roots.bind(alpha_bootstrap::ProgramEntry, Main::main);' "$OMEGA_BUILD")" -eq 1 ] || {
    echo "Interpreted Omega experiment: alpha_bootstrap is not one ordinary root" >&2
    exit 1
}

python3 "$SOURCE_CLOSURE_MATERIALIZER" "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$TMP/omega_compiler.epsilon"
grep -F 'data AlphaTapeBuffer {' "$TMP/omega_compiler.epsilon" >/dev/null || {
    echo "Interpreted Omega experiment: Omega D does not own Alpha tape construction" >&2
    exit 1
}

EPSILON_LINES=$(wc -l < "$EPSILON" | tr -d ' ')
EPSILON_BYTES=$(wc -c < "$EPSILON" | tr -d ' ')
[ "$EPSILON_LINES" -eq 10092 ]
[ "$EPSILON_BYTES" -eq 507516 ]

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
        507516,
        "13897864aedd63d815df5aad1c68a23d5e585df2645dcbd907641b0adad5846a",
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

# This customer uses whole, unchanged D members. The host only checks identities
# and concatenates bytes; no function extraction or source translation occurs.
omega_compiler = Path(os.environ["OMEGA_PATH_OMEGA_COMPILER"])
customer_members = (
    (omega_compiler / "representations.epsilon", 30905,
     "7b2b1ca57752256e9b10446ea8a2469075d9a0cac11ffe97f2037340528064ed"),
    (omega_compiler / "lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    (test_directory / "customers/omega_lexical/main.epsilon", 1486,
     "6ce07453269102f7f468241d1a066a21cbe08c2a0652bb460c9c34d2f6ef11b2"),
)
customer_source = b""
for path, size, digest in customer_members:
    data = path.read_bytes()
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"Omega D lexical customer member identity changed: {path.name}")
    customer_source += data
if len(customer_source) != 34911 or hashlib.sha256(customer_source).hexdigest() != (
    "45447a0cc81353c88d341354b444537bfb113b304fcab50659d774a5fab08e1b"
):
    raise SystemExit("Omega D lexical customer packed identity changed")
controls["Omega D lexical helpers"] = (customer_source, b"A\x00")

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
if status != 0 or len(receipt) != 599984:
    raise SystemExit(
        f"evaluator slice returned {status} with {len(receipt)} bytes "
        f"and SHA-256 {hashlib.sha256(receipt).hexdigest()}"
    )
if hashlib.sha256(receipt).hexdigest() != (
    "1799457c1c9f1475675d470a012838f094d284c212e74cc6267e9bab37800e1c"
):
    raise SystemExit(
        "evaluator receipt identity changed to "
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

echo "Interpreted Omega experiment: scalar calls, storage, state transfers, and exact D lexical customer pass"
