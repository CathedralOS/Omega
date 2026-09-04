#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Staged Delta compiler: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
COMPILER="$OMEGA_REPO_ROOT/source/delta/compiler/delta_compiler.gamma"
SOURCE="$GATE_DIR/nullary_match.delta"
EXPECTED="$GATE_DIR/nullary_match.gamma"
PAYLOAD_SOURCE="$GATE_DIR/payload_match.delta"
PAYLOAD_EXPECTED="$GATE_DIR/payload_match.gamma"
RECURSIVE_SOURCE="$GATE_DIR/recursive_match.delta"
RECURSIVE_EXPECTED="$GATE_DIR/recursive_match.gamma"
LIST_SOURCE="$GATE_DIR/list_match.delta"
LIST_EXPECTED="$GATE_DIR/list_match.gamma"
BYTES_SOURCE="$GATE_DIR/bytes_rope.delta"
BYTES_EXPECTED="$GATE_DIR/bytes_rope.gamma"

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null

COMPILER="$COMPILER" SOURCE="$SOURCE" EXPECTED="$EXPECTED" \
    PAYLOAD_SOURCE="$PAYLOAD_SOURCE" PAYLOAD_EXPECTED="$PAYLOAD_EXPECTED" \
    RECURSIVE_SOURCE="$RECURSIVE_SOURCE" RECURSIVE_EXPECTED="$RECURSIVE_EXPECTED" \
    LIST_SOURCE="$LIST_SOURCE" LIST_EXPECTED="$LIST_EXPECTED" \
    BYTES_SOURCE="$BYTES_SOURCE" BYTES_EXPECTED="$BYTES_EXPECTED" \
    EVALUATOR="$TMP/evaluator" python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path

compiler = Path(os.environ["COMPILER"]).read_bytes()
source = Path(os.environ["SOURCE"]).read_bytes()
expected = Path(os.environ["EXPECTED"]).read_bytes()
payload_source = Path(os.environ["PAYLOAD_SOURCE"]).read_bytes()
payload_expected = Path(os.environ["PAYLOAD_EXPECTED"]).read_bytes()
recursive_source = Path(os.environ["RECURSIVE_SOURCE"]).read_bytes()
recursive_expected = Path(os.environ["RECURSIVE_EXPECTED"]).read_bytes()
list_source = Path(os.environ["LIST_SOURCE"]).read_bytes()
list_expected = Path(os.environ["LIST_EXPECTED"]).read_bytes()
bytes_source = Path(os.environ["BYTES_SOURCE"]).read_bytes()
bytes_expected = Path(os.environ["BYTES_EXPECTED"]).read_bytes()

for name, data, lines, size, digest in (
    ("compiler", compiler, 1022, 40278, "04e459a8407f559cdc55083b71c2ff4f5c8328dbc7ff05682009644e6150a836"),
    ("source", source, 7, 195, "3fb6a3ef60b54c8b77b066edeec32a4c77fd9fb5ede8a64c997cbc8b7a9a1fec"),
    ("receipt", expected, 3, 159, "ace9d225806cd36712201fadd87031de99bd068cacde8b0140446122a3567663"),
    ("payload source", payload_source, 7, 186, "31affd043cd04144a6a6adf5353ef4080eaf34524cfc64d0d08f0c60d12c7802"),
    ("payload receipt", payload_expected, 3, 229, "ebea130d8dcb88aac7d2389adf1f655669adfc13217ecdeb1d247aa97224305b"),
    ("recursive source", recursive_source, 7, 187, "2122553bd7a2e7635df523eeaf0b7518fbaf71b4cfdbd1050aa190055182c3dd"),
    ("recursive receipt", recursive_expected, 3, 246, "8725427391f6ec805adde6dbf9e8bd24b3049f63b54e2c1c9b980eb307c4600e"),
    ("list source", list_source, 8, 221, "a86dd12c78f488de2ba4adea71ba90ee29057e97d805ce627befe48c939e3ac3"),
    ("list receipt", list_expected, 3, 324, "64812b78fde8e1aee5fca648af7cfc3a46bff7cac010d91f166c8df4b9125b0e"),
    ("bytes source", bytes_source, 24, 782, "5bcc5e89ff630bb9d5012b275e5fec4157e1c0959be93f5f6b6e36ce7028e5da"),
    ("bytes receipt", bytes_expected, 7, 1033, "dac8b39fa720de0bf4800c426ef7b0c69255d45643655f54dc847199115474df"),
):
    if len(data.splitlines()) != lines or len(data) != size:
        raise SystemExit(f"{name} size changed")
    if hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

def evaluate(program, sealed_input=b""):
    request = struct.pack("<I", len(program)) + program + sealed_input
    process = subprocess.Popen(
        [os.environ["EVALUATOR"]], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(request, timeout=30)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("selected Gamma evaluation timed out")
    return process.returncode, output

if evaluate(compiler, source) != (0, expected):
    raise SystemExit("nullary ADT lowering disagrees with exact Gamma receipt")
if evaluate(expected) != (0, b"\x09"):
    raise SystemExit("lowered nullary match did not produce 9")
if evaluate(compiler, payload_source) != (0, payload_expected):
    raise SystemExit("payload ADT lowering disagrees with exact Gamma receipt")
if evaluate(payload_expected) != (0, b"\x09"):
    raise SystemExit("lowered payload match did not produce 9")
if evaluate(compiler, recursive_source) != (0, recursive_expected):
    raise SystemExit("recursive ADT lowering disagrees with exact Gamma receipt")
if evaluate(recursive_expected) != (0, b"\x03"):
    raise SystemExit("lowered recursive match did not produce depth 3")
if evaluate(compiler, list_source) != (0, list_expected):
    raise SystemExit("two-field recursive List lowering disagrees with receipt")
if evaluate(list_expected) != (0, b"\x09"):
    raise SystemExit("lowered List match did not produce sum 9")
if evaluate(compiler, bytes_source) != (0, bytes_expected):
    raise SystemExit("Bytes-rope lowering disagrees with exact Gamma receipt")
if evaluate(bytes_expected) != (0, b"B"):
    raise SystemExit("Bytes-rope indexing did not produce 0x42")
empty_bytes = bytes_expected.replace(
    b"(bytes_get (bytes_concat (bytes_single 65) (bytes_single 66)) 1)",
    b"(bytes_get (bytes_empty) 0)",
)
if evaluate(empty_bytes) != (2, b""):
    raise SystemExit("empty Bytes-rope indexing did not trap")
none_source = payload_source.replace(b"(Some 9)", b"None")
none_status, none_receipt = evaluate(compiler, none_source)
if none_status != 0 or evaluate(none_receipt) != (0, b"\x07"):
    raise SystemExit("padded nullary value in payload ADT did not produce fallback 7")

identity = b"(def main () Int 7)\n"
if evaluate(compiler, identity) != (0, identity + b"\n"):
    raise SystemExit("ordinary scalar Gamma was not preserved")

textual_ascii_whitespace = b"\t(def main () Int 7)\r\n"
if evaluate(compiler, textual_ascii_whitespace) != (0, identity + b"\n"):
    raise SystemExit("admitted textual-ASCII whitespace did not compile")

shared_namespace = b"(data Token (Token Int))\n(def main () Int 7)\n"
if evaluate(compiler, shared_namespace) != (0, identity + b"\n"):
    raise SystemExit("type and constructor namespaces were incorrectly merged")

long_name = b"x" * 200
long_identifier = b"(def " + long_name + b" () Int 0)\n" + identity
if evaluate(compiler, long_identifier) != (0, long_identifier + b"\n"):
    raise SystemExit("bytewise name trie exhausted context on a long identifier")

malformed = {
    "unknown field type": b"(data Bad (Bad Missing))\n(def main () Int 0)\n",
    "missing payload argument": b"(data Option (None) (Some Int))\n(def main () Int (Some))\n",
    "missing payload binder": b"(data Option (None) (Some Int))\n(def main () Int (match None (None 0) (Some 1)))\n",
    "non-exhaustive match": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Left 7)))\n",
    "out-of-order match": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Right 9) (Left 7)))\n",
    "duplicate type": b"(data A (X))\n(data A (Y))\n(def main () Int 0)\n",
    "duplicate constructor": b"(data A (X))\n(data B (X))\n(def main () Int 0)\n",
    "duplicate function": b"(def f () Int 0)\n(def f () Int 1)\n(def main () Int 0)\n",
    "data after function": b"(def f () Int 0)\n(data A (X))\n(def main () Int 0)\n",
    "empty data": b"(data A)\n(def main () Int 0)\n",
    "missing main": b"(def f () Int 0)\n",
    "nul source byte": b"(def main () Int 0)\x00\n",
    "disallowed control byte": b"(def main () Int 0)\x08\n",
    "del source byte": b"(def main () Int 0)\x7f\n",
    "non-ASCII source byte": b"(def main () Int 0)\x80\n",
}
for name, candidate in malformed.items():
    status, _ = evaluate(compiler, candidate)
    if status != 2:
        raise SystemExit(f"{name} did not trap in the staged compiler")

stress = (
    b"".join(
        f"(def f{index} () Int {index % 200})\n".encode("ascii")
        for index in range(3000)
    )
    + b"(def main () Int (f2999))\n"
)
stress_status, stress_receipt = evaluate(compiler, stress)
if stress_status != 0 or len(stress) != 66266 or len(stress_receipt) != 66267:
    raise SystemExit("3,001-function staged transformation failed")
if evaluate(stress_receipt) != (0, b"\xc7"):
    raise SystemExit("3,001-function staged receipt did not produce 199")
PY

echo "Staged Delta compiler: source envelope, globals, recursive ADTs, and Bytes-shaped ropes pass"
