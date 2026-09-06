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
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$TEST_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"
SOURCE_CLOSURE_MATERIALIZER="$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py"
OMEGA_BUILD="$OMEGA_PATH_OMEGA/build.omg"
DRIVER="$TEST_DIR/execution_driver.delta"

command -v python3 >/dev/null 2>&1 || {
    echo "Interpreted Omega experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
EPSILON="$TMP/epsilon_compiler.delta"
DELTA="$TMP/delta_compiler.gamma"
python3 "$SOURCE_CLOSURE_MATERIALIZER" "$OMEGA_PATH_DELTA_COMPILER_SOURCES" \
    "$DELTA" --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
python3 "$SOURCE_CLOSURE_MATERIALIZER" "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON"

if grep -Eq 'EpsilonAlpha|epsilon_alpha_' "$EPSILON"; then
    echo "Interpreted Omega experiment: Epsilon still owns Alpha encoding" >&2
    exit 1
fi

[ "$(grep -Fc 'builder.roots.bind(alpha_bootstrap::ProgramEntry, Main::main);' "$OMEGA_BUILD")" -eq 1 ] || {
    echo "Interpreted Omega experiment: alpha_bootstrap is not one ordinary root" >&2
    exit 1
}

python3 "$SOURCE_CLOSURE_MATERIALIZER" "$OMEGA_PATH_OMEGA_COMPILER_SOURCES" "$TMP/compiler.epsilon"
grep -F 'data AlphaTapeBuffer {' "$TMP/compiler.epsilon" >/dev/null || {
    echo "Interpreted Omega experiment: Omega D does not own Alpha tape construction" >&2
    exit 1
}

EPSILON_LINES=$(wc -l < "$EPSILON" | tr -d ' ')
EPSILON_BYTES=$(wc -c < "$EPSILON" | tr -d ' ')
[ "$EPSILON_LINES" -eq 11984 ]
[ "$EPSILON_BYTES" -eq 610428 ]

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null
EPSILON="$EPSILON" DELTA="$DELTA" DRIVER="$DRIVER" TEST_DIR="$TEST_DIR" \
    EPSILON_SELECTED_CUSTOMER="$EPSILON_SELECTED_CUSTOMER" \
    EVALUATOR="$TMP/evaluator" python3 - <<'PY'
import csv
import hashlib
import os
import struct
import subprocess
import time
from pathlib import Path

artifacts = {
    "evaluator source": (
        Path(os.environ["EPSILON"]).read_bytes(),
        610428,
        "656c57b59d1ea8923343496b032d10ef8980e89e3c39b3855feae5498611a1c0",
    ),
    "slice driver": (
        Path(os.environ["DRIVER"]).read_bytes(),
        2565,
        "ba509602e6873117e59ffc544ada6c8aa16e20b08311e69a01b7cb3897199b38",
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

# These customers use whole, unchanged D members. The host only checks identities
# and concatenates bytes; no function extraction or source translation occurs.
compiler = Path(os.environ["OMEGA_PATH_OMEGA_COMPILER"])

def add_customer(name, members, expected_size, expected_digest, expected_output):
    source = b""
    for path, size, digest in members:
        data = path.read_bytes()
        if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
            raise SystemExit(f"{name} member identity changed: {path.name}")
        source += data
    if len(source) != expected_size or hashlib.sha256(source).hexdigest() != expected_digest:
        raise SystemExit(f"{name} packed identity changed")
    controls[name] = (source, b"", expected_output)

add_customer("Omega D lexical helpers", (
    (compiler / "representations.epsilon", 30905,
     "7b2b1ca57752256e9b10446ea8a2469075d9a0cac11ffe97f2037340528064ed"),
    (compiler / "lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    (test_directory / "customers/omega_lexical/main.epsilon", 1486,
     "6ce07453269102f7f468241d1a066a21cbe08c2a0652bb460c9c34d2f6ef11b2"),
), 34911, "45447a0cc81353c88d341354b444537bfb113b304fcab50659d774a5fab08e1b", b"\x00\x00\x00\x00\x00A")

add_customer("Omega D Alpha tape buffers", (
    (compiler / "representations.epsilon", 30905,
     "7b2b1ca57752256e9b10446ea8a2469075d9a0cac11ffe97f2037340528064ed"),
    (compiler / "alpha_tape.epsilon", 30828,
     "302bddf1161fa06b07d1aba914f1e84209006a03020e50127c2db22c0daba59d"),
    (test_directory / "customers/omega_alpha_tape/main.epsilon", 1797,
     "0ccf1ef98023c4e19038bb0bcde4fd27140dbed47df166874e84d2e8930c348f"),
), 63530, "08284b839b374d8e611bba77cd63d4093f1f72d7034dca7b8b1aeb26cec6c5b5", b"\x00\x00\x00\x00\x00ABCDEFGH")

add_customer("Omega D request and UTF-8", (
    (compiler / "representations.epsilon", 30905,
     "7b2b1ca57752256e9b10446ea8a2469075d9a0cac11ffe97f2037340528064ed"),
    (compiler / "request_and_utf8.epsilon", 7384,
     "663acd44f754150f9cfea7bf3e08afda6b13aeca99b4a7fc08141ae66da89abe"),
    (test_directory / "customers/omega_request/main.epsilon", 2253,
     "e85bfe363cae2b313db528d9776dd4e11a185199c3d453293421da674af17121"),
), 40542, "e47a07296fb205934fa013b4aae29d35d51d0e8f1ee08eaf5f7de9c69c2bb099", b"\x00\x00\x00\x00\x00A\n")

add_customer("Omega D numeric-base sums", (
    (compiler / "representations.epsilon", 30905,
     "7b2b1ca57752256e9b10446ea8a2469075d9a0cac11ffe97f2037340528064ed"),
    (compiler / "lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    (test_directory / "customers/omega_numeric_base/main.epsilon", 1479,
     "abf50c23d589624d59b7b3603918d5ab76e6a6192594c50534fbee6cdf334386"),
), 34904, "0f7f338afa427419fd6cea2e0f0536263d06ff8e41f1720b2e077cb702d1be0e", b"\x00\x00\x00\x00\x00A")

add_customer("Omega D lexer", (
    (compiler / "representations.epsilon", 30905,
     "7b2b1ca57752256e9b10446ea8a2469075d9a0cac11ffe97f2037340528064ed"),
    (compiler / "request_and_utf8.epsilon", 7384,
     "663acd44f754150f9cfea7bf3e08afda6b13aeca99b4a7fc08141ae66da89abe"),
    (compiler / "lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    (compiler / "lexer.epsilon", 44649,
     "e16e7a42ee0848ff56b06dff4e9900569ae57724a281c0d3bada847717412ba6"),
    (test_directory / "customers/omega_lexer/main.epsilon", 6771,
     "e4a262f1b011402970f958afbc6c950882bb75906fc7244b3ea19c8d489a0e06"),
), 92229, "d53f8f57eb7963c1a3126d206edc9b3b6c2bd4c2fd19c0989cf68053c7abf4bd", b"\x00\x00\x00\x00\x00A")

selected_customer = os.environ["EPSILON_SELECTED_CUSTOMER"]
if selected_customer:
    if not selected_customer.startswith("Omega D ") or selected_customer not in controls:
        raise SystemExit(f"unknown exact Omega D customer: {selected_customer}")
    controls = {selected_customer: controls[selected_customer]}
    print(f"Selected Epsilon customer: {selected_customer}", flush=True)

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
if status != 0 or len(receipt) != 711597:
    raise SystemExit(
        f"evaluator slice returned {status} with {len(receipt)} bytes "
        f"and SHA-256 {hashlib.sha256(receipt).hexdigest()}"
    )
if hashlib.sha256(receipt).hexdigest() != (
    "8b5cea511a5d286212848b2c4f919d2bde4d815a91f46be556e557d7ffb17217"
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
    status, observation = evaluate(receipt, application_input)
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
print(f"Epsilon execution: {len(controls)} exact diagnostic results pass", flush=True)
PY

if [ -n "$EPSILON_SELECTED_CUSTOMER" ]; then
    echo "Interpreted Omega customer: $EPSILON_SELECTED_CUSTOMER passes"
else
    echo "Interpreted Omega experiment: values, views, sums, state transfers, and exact D customers pass"
fi
