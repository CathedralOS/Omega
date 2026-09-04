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
    ("compiler", compiler, 1457, 56262, "2e27e24df277ce4e436a18356d81dcaf351b764e4fcd4e12f622ae8984944819"),
    ("source", source, 7, 195, "3fb6a3ef60b54c8b77b066edeec32a4c77fd9fb5ede8a64c997cbc8b7a9a1fec"),
    ("receipt", expected, 3, 165, "23cbae7abf00860445e72b9075d189adb841cf165bf8103f7f7bcd5c81aed74f"),
    ("payload source", payload_source, 7, 186, "31affd043cd04144a6a6adf5353ef4080eaf34524cfc64d0d08f0c60d12c7802"),
    ("payload receipt", payload_expected, 3, 230, "21e3f310ad474219c292308a6c88606f1bd1b57e6527adedfcd0c37565637c1e"),
    ("recursive source", recursive_source, 7, 187, "2122553bd7a2e7635df523eeaf0b7518fbaf71b4cfdbd1050aa190055182c3dd"),
    ("recursive receipt", recursive_expected, 3, 251, "38d8eaa184ac8012317770c0bddcec1564b3ae5e95b06367d86ab175502937bf"),
    ("list source", list_source, 8, 221, "a86dd12c78f488de2ba4adea71ba90ee29057e97d805ce627befe48c939e3ac3"),
    ("list receipt", list_expected, 3, 328, "6502b4eae14e40f95a57b6b73057c826938ea751a98dac5679d25d79024d320d"),
    ("bytes source", bytes_source, 24, 767, "a4366165ddac1f1ffea603463ec9c3e04e91331b857d0b978b06863e62438b94"),
    ("bytes receipt", bytes_expected, 7, 1056, "c135f242f1d2321dfc030abd00da0d138649f8d11a6820e98555d246112ef4f6"),
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
    raise SystemExit("recursive rope lowering disagrees with exact Gamma receipt")
if evaluate(bytes_expected) != (0, b"B"):
    raise SystemExit("recursive rope indexing did not produce 0x42")
empty_bytes = bytes_expected.replace(
    b"(__d_rope_get (__d_rope_concat (__d_rope_single 65) (__d_rope_single 66)) 1)",
    b"(__d_rope_get (__d_rope_empty) 0)",
)
if evaluate(empty_bytes) != (2, b""):
    raise SystemExit("empty recursive rope indexing did not trap")
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
long_status, long_receipt = evaluate(compiler, long_identifier)
if long_status != 0 or evaluate(long_receipt) != (0, b"\x07"):
    raise SystemExit("bytewise name trie exhausted context on a long identifier")

for name, literal in (
    ("maximum Int", b"9223372036854775807"),
    ("minimum Int", b"-9223372036854775808"),
):
    boundary = b"(def main () Int " + literal + b")\n"
    if evaluate(compiler, boundary) != (0, boundary + b"\n"):
        raise SystemExit(f"{name} literal did not compile")

user_read = b"(def read ((x Int)) Int x)\n(def main () Int (read 7))\n"
user_read_status, user_read_receipt = evaluate(compiler, user_read)
if user_read_status != 0 or evaluate(user_read_receipt) != (0, b"\x07"):
    raise SystemExit("declared function named read did not resolve exactly")

authored_old_generated_prefix = (
    b"(data Choice (Left) (Right))\n"
    b"(def main () Int (let __m63 Int 7 "
    b"(match Left (Left __m63) (Right 9))))\n"
)
old_prefix_status, old_prefix_receipt = evaluate(
    compiler, authored_old_generated_prefix
)
if old_prefix_status != 0 or evaluate(old_prefix_receipt) != (0, b"\x07"):
    raise SystemExit("generated match binder captured authored __m63 local")

sibling_locals = b"(def main () Int (+ (let x Int 3 x) (let x Int 4 x)))\n"
sibling_status, sibling_receipt = evaluate(compiler, sibling_locals)
if sibling_status != 0 or evaluate(sibling_receipt) != (0, b"\x07"):
    raise SystemExit("disjoint sibling lets could not reuse a local name")

arm_locals = (
    b"(data Choice (Left Int) (Right Int))\n"
    b"(def choose ((value Choice)) Int "
    b"(match value ((Left x) x) ((Right x) x)))\n"
    b"(def main () Int (choose (Right 7)))\n"
)
arm_status, arm_receipt = evaluate(compiler, arm_locals)
if arm_status != 0 or evaluate(arm_receipt) != (0, b"\x07"):
    raise SystemExit("disjoint match arms could not reuse a local name")

local_function_homonym = (
    b"(def f ((x Int)) Int x)\n"
    b"(def apply ((f Int)) Int (f f))\n"
    b"(def main () Int (apply 7))\n"
)
homonym_status, homonym_receipt = evaluate(compiler, local_function_homonym)
if homonym_status != 0 or evaluate(homonym_receipt) != (0, b"\x07"):
    raise SystemExit("local and function grammar namespaces were merged")

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
    "uppercase function": b"(def Bad () Int 0)\n(def main () Int 0)\n",
    "reserved function": b"(def bytes_get () Int 0)\n(def main () Int 0)\n",
    "invalid type name": b"(data Bad-Type (X))\n(def main () Int 0)\n",
    "reserved type name": b"(data Int (X))\n(def main () Int 0)\n",
    "invalid constructor name": b"(data Bad (X!))\n(def main () Int 0)\n",
    "invalid parameter name": b"(def f ((9x Int)) Int 0)\n(def main () Int 0)\n",
    "duplicate parameter": b"(def f ((x Int) (x Int)) Int x)\n(def main () Int 0)\n",
    "unknown local": b"(def main () Int missing)\n",
    "function used as value": b"(def f () Int 7)\n(def main () Int f)\n",
    "self reference in let initializer": b"(def main () Int (let x Int x x))\n",
    "duplicate active let": b"(def main ((x Int)) Int (let x Int 1 x))\n",
    "escaped let local": b"(def main () Int (+ (let x Int 1 x) x))\n",
    "duplicate pattern local": b"(data Pair (Pair Int Int))\n(def main () Int (match (Pair 1 2) ((Pair x x) x)))\n",
    "pattern duplicates outer local": b"(data Box (Box Int))\n(def f ((x Int)) Int (match (Box 1) ((Box x) x)))\n(def main () Int (f 7))\n",
    "escaped pattern local": b"(data Box (Box Int))\n(def main () Int (+ (match (Box 1) ((Box x) x)) x))\n",
    "invalid result type": b"(def main () Missing 0)\n",
    "invalid let binder": b"(def main () Int (let Bad Int 0 0))\n",
    "invalid let type": b"(def main () Int (let x Missing 0 x))\n",
    "invalid pattern binder": b"(data O (S Int))\n(def main () Int (match (S 1) ((S Bad) Bad)))\n",
    "invalid application head": b"(def main () Int (foo-bar 1))\n",
    "malformed integer": b"(def main () Int 12x)\n",
    "positive integer overflow": b"(def main () Int 9223372036854775808)\n",
    "negative integer overflow": b"(def main () Int -9223372036854775809)\n",
    "unknown function": b"(def main () Int (missing 0))\n",
    "undeclared Gamma input": b"(def main () Int (input))\n",
    "undeclared Gamma read": b"(def main () Int (read 0))\n",
    "undeclared Gamma pair": b"(def main () Int (pair 1 2))\n",
    "missing function argument": b"(def f ((x Int)) Int x)\n(def main () Int (f))\n",
    "excess function argument": b"(def f ((x Int)) Int x)\n(def main () Int (f 1 2))\n",
    "missing if argument": b"(def main () Int (if 1 2))\n",
    "excess operator argument": b"(def main () Int (+ 1 2 3))\n",
    "missing Bytes builtin argument": b"(def main () Int (bytes_get 0))\n",
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
if stress_status != 0 or len(stress) != 66266 or len(stress_receipt) != 78271:
    raise SystemExit("3,001-function staged transformation failed")
if evaluate(stress_receipt) != (0, b"\xc7"):
    raise SystemExit("3,001-function staged receipt did not produce 199")
PY

echo "Staged Delta compiler: source envelope, lexical atoms, globals, local scopes, and recursive ADTs pass"
