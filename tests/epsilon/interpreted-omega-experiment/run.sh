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
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/compiler_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/epsilon/evaluator_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/omega/compiler_env.sh"
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
# Bound materializers refuse before writing when the canonical manifest,
# members, or packed closure differ from the audited edge records.
materialize_delta_compiler "$DELTA"
materialize_delta_support "$TMP/support.bin"
materialize_epsilon_evaluator "$EPSILON"

if grep -Eq 'EpsilonAlpha|epsilon_alpha_' "$EPSILON"; then
    echo "Interpreted Omega experiment: Epsilon still owns Alpha encoding" >&2
    exit 1
fi

[ "$(grep -Fc 'builder.roots.bind(alpha_bootstrap::ProgramEntry, Main::main);' "$OMEGA_BUILD")" -eq 1 ] || {
    echo "Interpreted Omega experiment: alpha_bootstrap is not one ordinary root" >&2
    exit 1
}

materialize_omega_compiler "$TMP/compiler.epsilon"
grep -F 'data AlphaTapeBuffer {' "$TMP/compiler.epsilon" >/dev/null || {
    echo "Interpreted Omega experiment: Omega D does not own Alpha tape construction" >&2
    exit 1
}

EPSILON_LINES=$(wc -l < "$EPSILON" | tr -d ' ')
EPSILON_BYTES=$(wc -c < "$EPSILON" | tr -d ' ')
[ "$EPSILON_LINES" -eq 12097 ]
[ "$EPSILON_BYTES" -eq 617354 ]

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
import time
from pathlib import Path

artifacts = {
    "evaluator source": (
        Path(os.environ["EPSILON"]).read_bytes(),
        617354,
        "4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e",
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
    (compiler / "representations.epsilon", 32026,
     "bb0b30da622065d298ebfc2bcb865ff3f6fd51467d259331282464c25c4060ee"),
    (compiler / "lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    (test_directory / "customers/omega_lexical/main.epsilon", 1486,
     "6ce07453269102f7f468241d1a066a21cbe08c2a0652bb460c9c34d2f6ef11b2"),
), 36032, "463aeb99dcfccc937b9b3ba55aa05d5d89ddc006757e38c244985a582461814a", b"\x00\x00\x00\x00\x00A")

# Independent literal oracle, not a host encoder. Only the actual successful
# observation's suffix is stamped below. Offsets follow Alpha SEMANTICS.md.
alpha_program = bytes.fromhex(
    "01 ff ffffffffffffffff"  # 0: imm EOF sentinel
    "01 fe 0100000000000000"  # 10: imm counter increment
    "01 fd 0000000000000000"  # 20: imm counter
    "11 00"                  # 30: read byte
    "10 00 ff 4a00000000000000"  # 32: jeq EOF -> halt at 74
    "13 4c00000000000000"     # 43: call writer at 76
    "03 fd fe"               # 52: increment counter
    "0e 00 1e00000000000000"  # 55: jnz -> read at 30
    "0c 1e00000000000000"     # 65: zero-byte back edge
    "00 fd"                  # 74: halt with count
    "12 00"                  # 76: write byte
    "14"                     # 78: ret
)
add_customer("Omega D Alpha tape buffers", (
    (compiler / "representations.epsilon", 32026,
     "bb0b30da622065d298ebfc2bcb865ff3f6fd51467d259331282464c25c4060ee"),
    (compiler / "alpha_tape.epsilon", 30832,
     "26e943b2386e1f27761951af92d163cdaa54f32bdd990aa4694b91f10cb095a3"),
    (test_directory / "customers/omega_alpha_tape/main.epsilon", 7274,
     "a186166b32d38cfafdc02fc47f1ad46f2af7d18ed15b5bb0f18b0caa525bb35d"),
), 70132, "d9b2cf034d21d4575f3ef71f2dc92e8bd18cfaaa55121b9faab7b8c931f13e91",
    b"\x00\x00\x00\x00\x00ABCDEFGH\x0c\x09\x00\x00\x00\x00\x00\x00\x00\x14"
    + alpha_program)

add_customer("Omega D request and UTF-8", (
    (compiler / "representations.epsilon", 32026,
     "bb0b30da622065d298ebfc2bcb865ff3f6fd51467d259331282464c25c4060ee"),
    (compiler / "request_and_utf8.epsilon", 23678,
     "fe55376ff4c64ca61a045fee84c7856d4eefdda502832b967f680517150577e5"),
    (test_directory / "customers/omega_request/main.epsilon", 9510,
     "0f838c478d6497cb4ca92d35f2aeed167b61eca4ab245847fa04da0fc2f7b0b7"),
), 65214, "7e42622802efb866699579f86d7de6f938989e36cf7db761cea4b6f38dba079f", b"\x00\x00\x00\x00\x00A\n")

add_customer("Omega D request invocation fields", (
    (compiler / "representations.epsilon", 32026,
     "bb0b30da622065d298ebfc2bcb865ff3f6fd51467d259331282464c25c4060ee"),
    (compiler / "request_and_utf8.epsilon", 23678,
     "fe55376ff4c64ca61a045fee84c7856d4eefdda502832b967f680517150577e5"),
    (test_directory / "customers/omega_request_invocation/main.epsilon", 5930,
     "b02e8f6f973a55ead1131a28cb939af0d6c31501d38bd7b15cba2cff4e6710bd"),
), 61634, "01e1585b6f85414393166c08ff45b77982826c5d08281350a30ab5f08ff99c29", b"\x00\x00\x00\x00\x00A\n")

add_customer("Omega D numeric-base sums", (
    (compiler / "representations.epsilon", 32026,
     "bb0b30da622065d298ebfc2bcb865ff3f6fd51467d259331282464c25c4060ee"),
    (compiler / "lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    (test_directory / "customers/omega_numeric_base/main.epsilon", 1479,
     "abf50c23d589624d59b7b3603918d5ab76e6a6192594c50534fbee6cdf334386"),
), 36025, "ec1ad469c5a6d8d3505020f7104f5f8a7590ee27056d2a24270785edaacaa97f", b"\x00\x00\x00\x00\x00A")

add_customer("Omega D lexer", (
    (compiler / "representations.epsilon", 32026,
     "bb0b30da622065d298ebfc2bcb865ff3f6fd51467d259331282464c25c4060ee"),
    (compiler / "request_and_utf8.epsilon", 23678,
     "fe55376ff4c64ca61a045fee84c7856d4eefdda502832b967f680517150577e5"),
    (compiler / "lexical_classification.epsilon", 2520,
     "12a3775f19ac6030bcca609acbf530ee64a09111cc24d7e941292e0d05fd996f"),
    (compiler / "lexer.epsilon", 44649,
     "e16e7a42ee0848ff56b06dff4e9900569ae57724a281c0d3bada847717412ba6"),
    (test_directory / "customers/omega_lexer/main.epsilon", 6771,
     "e4a262f1b011402970f958afbc6c950882bb75906fc7244b3ea19c8d489a0e06"),
), 109644, "95b15745e93609a5c164b5842218782653edc779280ea932a6293fda7deede4f", b"\x00\x00\x00\x00\x00A")

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

status, receipt = evaluate(compiler, request)
if status != 0 or len(receipt) != 721484:
    raise SystemExit(
        f"evaluator slice returned {status} with {len(receipt)} bytes "
        f"and SHA-256 {hashlib.sha256(receipt).hexdigest()}"
    )
if hashlib.sha256(receipt).hexdigest() != (
    "71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082"
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
    if name == "Omega D Alpha tape buffers":
        Path(os.environ["EPSILON_ALPHA_TAPE"]).write_bytes(observation[-len(alpha_program):])
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
