#!/usr/bin/env sh
set -eu

EPSILON_SELECTED_CUSTOMER=
if [ "$#" -ne 0 ]; then
    if [ "$#" -ne 2 ] || [ "$1" != "--customer" ] || [ -z "$2" ]; then
        echo "usage: sh run.sh [--customer 'Omega D customer name']" >&2
        exit 2
    fi
    EPSILON_SELECTED_CUSTOMER=$2
fi

TEST_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
GATE_DIR=$TEST_DIR
GATE_LABEL="Interpreted Omega experiment"
GATE_ENVS="delta/compiler_env.sh epsilon/evaluator_env.sh omega/compiler_env.sh"
. "$GATE_DIR/../../gate_env.sh"
OMEGA_BUILD="$OMEGA_PATH_OMEGA/application/construction/build.omg"
DRIVER="$TEST_DIR/execution_driver.delta"

. "$GATE_DIR/../../gate_host.sh"

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
EPSILON="$TMP/epsilon_compiler.delta"
DELTA="$TMP/delta_compiler.gamma"
# Bound materializers refuse before writing when the canonical manifest,
# members, or packed closure differ from the audited edge records.
materialize_delta_compiler "$DELTA"
materialize_delta_support "$TMP/support.bin"
materialize_epsilon_evaluator "$EPSILON"

if grep -Eq 'EpsilonAlpha|epsilon_alpha_' "$EPSILON"; then
    echo "Interpreted Omega experiment: Epsilon still owns Alpha encoding" >&2
    exit 1
fi

if [ "$(grep -Fc 'Build::bind(builder, alpha_bootstrap::ProgramEntry,' "$OMEGA_BUILD")" -ne 1 ] ||
    ! grep -Fq 'Build::bind(builder, alpha_bootstrap::ProgramEntry, entry_start);' "$OMEGA_BUILD"; then
    echo "Interpreted Omega experiment: construction must bind one alpha_bootstrap root to entry_start" >&2
    exit 1
fi

materialize_omega_compiler "$TMP/compiler.epsilon"
grep -F 'data AlphaTapeBuffer {' "$TMP/compiler.epsilon" >/dev/null || {
    echo "Interpreted Omega experiment: Omega D does not own Alpha tape construction" >&2
    exit 1
}

EPSILON_LINES=$(wc -l < "$EPSILON" | tr -d ' ')
EPSILON_BYTES=$(wc -c < "$EPSILON" | tr -d ' ')
[ "$EPSILON_LINES" -eq 12092 ]
[ "$EPSILON_BYTES" -eq 616939 ]

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EPSILON="$EPSILON" DELTA="$DELTA" DRIVER="$DRIVER" TEST_DIR="$TEST_DIR" \
    EPSILON_SELECTED_CUSTOMER="$EPSILON_SELECTED_CUSTOMER" \
    EPSILON_ALPHA_TAPE="$TMP/customer.tape" \
    SUPPORT="$TMP/support.bin" \
    EVALUATOR="$TMP/evaluator" python3 - <<'PY'
import csv
import hashlib
import os
import struct
import subprocess
import sys
import time
from pathlib import Path

artifacts = {
    "evaluator source": (
        Path(os.environ["EPSILON"]).read_bytes(),
        616939,
        "a2830415a4d98176e9bd97e7f28a9c4512d926fdb5c502d73e19d874ed58ca4c",
    ),
    "slice driver": (
        Path(os.environ["DRIVER"]).read_bytes(),
        2150,
        "ab79416b4278d3b132a4fd432485f1b90e868c0549af96e604e2e8da570d1baa",
    ),
}
for name, (data, size, digest) in artifacts.items():
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

test_directory = Path(os.environ["TEST_DIR"])
controls = {}
with (test_directory / "fixtures.tsv").open(encoding="ascii", newline="") as manifest:
    rows = csv.DictReader(manifest, delimiter="\t")
    if rows.fieldnames != ["fixture", "bytes", "sha256", "expected_hex", "stdin_hex"]:
        raise SystemExit("fixture manifest header changed")
    for row in rows:
        name = row["fixture"]
        if Path(name).name != name or not name.endswith(".epsilon") or name in controls:
            raise SystemExit(f"invalid or repeated fixture identity: {name}")
        data = (test_directory / name).read_bytes()
        if len(data) != int(row["bytes"]) or hashlib.sha256(data).hexdigest() != row["sha256"]:
            raise SystemExit(f"{name} identity changed")
        controls[name] = (
            data, bytes.fromhex(row["stdin_hex"]), bytes.fromhex(row["expected_hex"])
        )
if set(controls) != {path.name for path in test_directory.glob("*.epsilon")}:
    raise SystemExit("fixture manifest does not cover the exact Epsilon fixture inventory")

# The canonical and diagnostic entries exercise the same pinned D members and
# literal observations under their respective request envelopes.
sys.path.insert(0, str(test_directory.parent / "refinement"))
sys.path.insert(0, str(test_directory.parent / "d-composition"))
from customers import ALPHA_PROGRAM, member_customers

for customer in member_customers():
    controls[customer.name] = (customer.source, customer.stdin, customer.expected)

selected_customer = os.environ["EPSILON_SELECTED_CUSTOMER"]
if selected_customer:
    if not selected_customer.startswith("Omega D ") or selected_customer not in controls:
        raise SystemExit(f"unknown exact Omega D customer: {selected_customer}")
    controls = {selected_customer: controls[selected_customer]}
    print(f"Selected Epsilon customer: {selected_customer}", flush=True)

compiler = Path(os.environ["DELTA"]).read_bytes()
support = Path(os.environ["SUPPORT"]).read_bytes()
subject = artifacts["evaluator source"][0] + artifacts["slice driver"][0]
request = (
    b"DCREQ\x01\x00\x00"
    + struct.pack("<I", 1)
    + struct.pack("<I", len(subject))
    + subject
    + support
)

def evaluate(program, sealed_input=b"", timeout=300):
    framed = struct.pack("<I", len(program)) + program + sealed_input
    process = subprocess.run(
        [os.environ["EVALUATOR"]], input=framed, stdout=subprocess.PIPE,
        timeout=timeout,
    )
    return process.returncode, process.stdout

def positive_seconds(name, default):
    try:
        seconds = int(os.environ.get(name, default))
    except ValueError:
        seconds = 0
    if seconds <= 0:
        raise SystemExit(f"{name} must be a positive integer")
    return seconds

receipt_timeout = positive_seconds("OMEGA_EPSILON_RECEIPT_SECONDS", 600)
customer_timeout = positive_seconds("OMEGA_DCOMP_OBSERVATION_SECONDS", 14400)
status, receipt = evaluate(compiler, request, timeout=receipt_timeout)
if status != 0 or len(receipt) != 719006:
    raise SystemExit(
        f"evaluator slice returned {status} with {len(receipt)} bytes "
        f"and SHA-256 {hashlib.sha256(receipt).hexdigest()}"
    )
if hashlib.sha256(receipt).hexdigest() != (
    "f368dfc90787025cdde8067ca81b83778ceec47791055320235e89e0d845f340"
):
    raise SystemExit(
        "evaluator receipt identity changed to "
        + hashlib.sha256(receipt).hexdigest()
    )
print(f"Epsilon evaluator: exact {len(receipt)}-byte receipt reconstructed", flush=True)
driver_controls = (
    ("empty frame", b"", b"\x05"),
    ("one header byte", b"\x00", b"\x05"),
    ("two header bytes", b"\x00\x00", b"\x05"),
    ("three header bytes", b"\x00\x00\x00", b"\x05"),
    ("source exceeds frame", struct.pack("<I", 2) + b"A", b"\x05"),
    ("maximum source length exceeds frame", b"\xff\xff\xff\xffA", b"\x05"),
    ("empty Epsilon source", struct.pack("<I", 0), b"\x02\x08\x00\x00\x00\x00"),
)
for name, application_input, expected in driver_controls:
    status, observation = evaluate(receipt, application_input)
    if (status, observation) != (0, expected):
        raise SystemExit(
            f"{name}: expected status 0 and {expected.hex()}, "
            f"received status {status} and {observation.hex()}"
        )
print(f"Epsilon development framing: {len(driver_controls)} observations pass", flush=True)
for name, (source, stdin, expected) in controls.items():
    # Development-only separation of source and stdin, not the final Epsilon envelope.
    application_input = struct.pack("<I", len(source)) + source + stdin
    started = time.monotonic()
    timeout = customer_timeout if name.startswith("Omega D ") else 300
    status, observation = evaluate(receipt, application_input, timeout=timeout)
    if (status, observation) != (0, expected):
        raise SystemExit(
            f"{name}: expected status 0 and {expected.hex()}, "
            f"received status {status} and {observation.hex()}"
        )
    if name.startswith("Omega D "):
        print(
            f"{name}: exact observation passes in {time.monotonic() - started:.3f}s",
            flush=True,
        )
    if name == "Omega D Alpha tape buffers":
        Path(os.environ["EPSILON_ALPHA_TAPE"]).write_bytes(observation[-len(ALPHA_PROGRAM):])
print(f"Epsilon execution: {len(controls)} exact diagnostic results pass", flush=True)
PY

if [ -z "$EPSILON_SELECTED_CUSTOMER" ] || [ "$EPSILON_SELECTED_CUSTOMER" = 'Omega D Alpha tape buffers' ]; then
    stamp_seed "$TMP/customer.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/customer.exe"
    python3 - "$TMP/customer.exe" <<'PY'
import subprocess
import sys

controls = ((b"", 0), (b"\x00", 1), (b"\x80\xff", 2), (b"\x00\x80\xff", 3))
for sealed_input, expected_exit in controls:
    process = subprocess.run(
        [sys.argv[1]], input=sealed_input, stdout=subprocess.PIPE, timeout=30,
    )
    if (process.returncode, process.stdout) != (expected_exit, sealed_input):
        raise SystemExit(
            f"D-produced Alpha for stdin {sealed_input.hex()}: "
            f"expected exit {expected_exit} and stdout {sealed_input.hex()}, "
            f"received exit {process.returncode} and stdout {process.stdout.hex()}"
        )
print(f"D-produced Alpha: {len(controls)} exact echo/count executions pass", flush=True)
PY
fi

if [ -n "$EPSILON_SELECTED_CUSTOMER" ]; then
    echo "Interpreted Omega customer: $EPSILON_SELECTED_CUSTOMER passes"
else
    echo "Interpreted Omega experiment: values, views, sums, state transfers, and exact D customers pass"
fi
