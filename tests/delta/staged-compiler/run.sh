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
COMPILER="$TMP/development.gamma"
CANONICAL_COMPILER="$TMP/compiler.gamma"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$COMPILER" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_DEVELOPMENT_ENTRY"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$CANONICAL_COMPILER" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
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
FORWARD_SOURCE="$GATE_DIR/forward_mutual_nominals.delta"
FORWARD_EXPECTED="$GATE_DIR/forward_mutual_nominals.gamma"
EPSILON_SOURCE="$TMP/epsilon_compiler.delta"
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_EPSILON_COMPILER_SOURCES" "$EPSILON_SOURCE"

materialize_gamma_evaluator "$TMP/evaluator" >/dev/null

COMPILER="$COMPILER" CANONICAL_COMPILER="$CANONICAL_COMPILER" \
    SOURCE="$SOURCE" EXPECTED="$EXPECTED" \
    PAYLOAD_SOURCE="$PAYLOAD_SOURCE" PAYLOAD_EXPECTED="$PAYLOAD_EXPECTED" \
    RECURSIVE_SOURCE="$RECURSIVE_SOURCE" RECURSIVE_EXPECTED="$RECURSIVE_EXPECTED" \
    LIST_SOURCE="$LIST_SOURCE" LIST_EXPECTED="$LIST_EXPECTED" \
    BYTES_SOURCE="$BYTES_SOURCE" BYTES_EXPECTED="$BYTES_EXPECTED" \
    FORWARD_SOURCE="$FORWARD_SOURCE" FORWARD_EXPECTED="$FORWARD_EXPECTED" \
    EPSILON_SOURCE="$EPSILON_SOURCE" \
    EVALUATOR="$TMP/evaluator" PYTHONPATH="$GATE_DIR" python3 -B - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path
from emitter_fixtures import fixtures as emitter_fixtures

compiler = Path(os.environ["COMPILER"]).read_bytes()
canonical_compiler = Path(os.environ["CANONICAL_COMPILER"]).read_bytes()
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
forward_source = Path(os.environ["FORWARD_SOURCE"]).read_bytes()
forward_expected = Path(os.environ["FORWARD_EXPECTED"]).read_bytes()
epsilon_source = Path(os.environ["EPSILON_SOURCE"]).read_bytes()

for name, data, lines, size, digest in (
    ("development compiler", compiler, 2613, 110437, "8bb7bb3e8d4d4ab27c33c7baddbe790150e0166bcf9e10479ed3abf081023783"),
    ("canonical compiler", canonical_compiler, 2620, 110660, "bd66cd03ba81f7332217c5d5fab63c15d3486e7a43b819430f39152e5e213a79"),
    ("source", source, 7, 195, "3fb6a3ef60b54c8b77b066edeec32a4c77fd9fb5ede8a64c997cbc8b7a9a1fec"),
    ("receipt", expected, 3, 165, "23cbae7abf00860445e72b9075d189adb841cf165bf8103f7f7bcd5c81aed74f"),
    ("payload source", payload_source, 7, 186, "31affd043cd04144a6a6adf5353ef4080eaf34524cfc64d0d08f0c60d12c7802"),
    ("payload receipt", payload_expected, 3, 230, "21e3f310ad474219c292308a6c88606f1bd1b57e6527adedfcd0c37565637c1e"),
    ("recursive source", recursive_source, 7, 187, "2122553bd7a2e7635df523eeaf0b7518fbaf71b4cfdbd1050aa190055182c3dd"),
    ("recursive receipt", recursive_expected, 3, 425, "680509f225be307830afa921a23e31f0977b78f7d7f951da8f6167bc26f554fd"),
    ("list source", list_source, 8, 221, "a86dd12c78f488de2ba4adea71ba90ee29057e97d805ce627befe48c939e3ac3"),
    ("list receipt", list_expected, 3, 502, "3f86a1436fa1c8f512476f27886a88d5e443f76974343532cf3a3d082b4509a0"),
    ("bytes source", bytes_source, 24, 767, "a4366165ddac1f1ffea603463ec9c3e04e91331b857d0b978b06863e62438b94"),
    ("bytes receipt", bytes_expected, 7, 1404, "bd0675bcca501256724fb91ab366672db066ac449e94fb917c9fcfd0ea505bb1"),
    ("forward nominal source", forward_source, 11, 397, "02dd0884c3ede6111468d6b6acb88f0c2e208fd2c9151dc75bf0f695091c3915"),
    ("forward nominal receipt", forward_expected, 3, 956, "68f42cfe7a81d65e8c5680715ddb5f5b2fda4fae3b2aaf677516bd9646f068bd"),
):
    if len(data.splitlines()) != lines or len(data) != size:
        raise SystemExit(f"{name} size changed")
    if hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

for retired_scanner in (
    b"find_type_owner_forms",
    b"find_constructor_owner_forms",
    b"find_constructor_tag_forms",
):
    if retired_scanner in compiler:
        raise SystemExit("whole-source nominal lookup scanner returned")

evaluation_count = 0

def evaluate(program, sealed_input=b""):
    global evaluation_count
    evaluation_count += 1
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
        raise SystemExit(
            f"selected Gamma evaluation {evaluation_count} timed out after 30s; "
            f"program={len(program)} bytes sha256={hashlib.sha256(program).hexdigest()}; "
            f"input={len(sealed_input)} bytes sha256={hashlib.sha256(sealed_input).hexdigest()}"
        )
    return process.returncode, output

def dcreq(profile, delta_source):
    return (
        b"DCREQ\x01\x00\x00"
        + struct.pack("<I", profile)
        + struct.pack("<I", len(delta_source))
        + delta_source
    )

conformance_identity = b"(def main ((source Bytes)) Bytes source)\n"
profile_status, profile_receipt = evaluate(
    canonical_compiler, dcreq(1, conformance_identity)
)
if profile_status != 0 or not profile_receipt.startswith(
    b"(def $application () Int 1)\n"
):
    raise SystemExit("ConformanceBytesV1 did not emit a marked application")
for payload in (b"", b"ABC"):
    if evaluate(profile_receipt, payload) != (0, payload):
        raise SystemExit("ConformanceBytesV1 did not preserve exact input bytes")

conformance_trap = (
    b"(def main ((source Bytes)) Bytes "
    b"(let ignored Int (bytes_get source -1) source))\n"
)
trap_status, trap_receipt = evaluate(canonical_compiler, dcreq(1, conformance_trap))
if trap_status != 0 or evaluate(trap_receipt) != (249, b""):
    raise SystemExit("ConformanceBytesV1 did not map an authored trap")

if evaluate(profile_receipt, b"A" * 4194305) != (253, b""):
    raise SystemExit("ConformanceBytesV1 did not reject adjacent input extent")

conformance_double = (
    b"(def main ((source Bytes)) Bytes (bytes_concat source source))\n"
)
double_status, double_receipt = evaluate(canonical_compiler, dcreq(1, conformance_double))
if double_status != 0:
    raise SystemExit("ConformanceBytesV1 output-extent fixture did not compile")
if evaluate(double_receipt, b"A" * 2097153) != (254, b""):
    raise SystemExit("ConformanceBytesV1 did not reject adjacent output extent")

wrong_schema = b"(def main () Int 7)\n"
if evaluate(canonical_compiler, dcreq(1, wrong_schema))[0] == 0:
    raise SystemExit("ConformanceBytesV1 admitted the wrong main schema")

malformed_requests = (
    b"DCREQ\x01\x00\x00" + struct.pack("<I", 0) + struct.pack("<I", 0),
    b"DCREQ\x01\x00\x00" + struct.pack("<I", 2) + struct.pack("<I", 0),
    b"DCREQ\x01\x00\x00" + struct.pack("<I", 1) + struct.pack("<I", 1),
    dcreq(1, conformance_identity) + b"x",
)
for malformed_request in malformed_requests:
    if evaluate(canonical_compiler, malformed_request)[0] == 0:
        raise SystemExit("malformed DCREQ unexpectedly compiled")

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
if evaluate(compiler, forward_source) != (0, forward_expected):
    raise SystemExit("forward/mutual nominal lowering disagrees with receipt")
if evaluate(forward_expected) != (0, b"\x07"):
    raise SystemExit("forward/mutual nominal receipt did not produce 7")
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

epsilon_data_prefix = epsilon_source.split(b"\n(def ", 1)[0] + b"\n"
if epsilon_data_prefix.count(b"(data ") < 100:
    raise SystemExit("Epsilon nominal customer prefix became trivial")
epsilon_census = epsilon_data_prefix + identity
epsilon_status, epsilon_receipt = evaluate(compiler, epsilon_census)
if (epsilon_status, epsilon_receipt) != (0, identity + b"\n"):
    raise SystemExit("Epsilon nominal customer census did not compile")
if evaluate(epsilon_receipt) != (0, b"\x07"):
    raise SystemExit("Epsilon nominal customer census receipt did not run")

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

nominal_types = (
    b"(data Box (Box Int))\n"
    b"(def get ((value Box)) Int "
    b"(match value ((Box payload) (+ payload 1))))\n"
    b"(def main () Int (get (Box 6)))\n"
)
nominal_status, nominal_receipt = evaluate(compiler, nominal_types)
if nominal_status != 0 or evaluate(nominal_receipt) != (0, b"\x07"):
    raise SystemExit("nominal constructor, pattern, call, or result types failed")

cached_signatures = b"""(data Box (Box Int))
(def first ((count Int) (bytes Bytes) (box Box)) Int
  (if (eq count 0)
    (+ (bytes_length bytes) (match box ((Box value) (+ value 1))))
    (second box bytes (- count 1))))
(def second ((box Box) (bytes Bytes) (count Int)) Int
  (if (eq count 0)
    (+ (bytes_length bytes) (match box ((Box value) (+ value 1))))
    (first (- count 1) bytes box)))
(def main () Int (first 1 (bytes_single 0) (Box 5)))
"""
signature_status, signature_receipt = evaluate(compiler, cached_signatures)
if signature_status != 0 or evaluate(signature_receipt) != (0, b"\x07"):
    raise SystemExit("cached ordered signatures changed forward/mutual calls")

reordered_match = b"""(data Choice (Left) (Middle) (Right))
(def choose ((value Choice)) Int
  (match value (Right 4) (Left 1) (Middle 2)))
(def main () Int (+ (choose Left) (+ (choose Middle) (choose Right))))
"""
reordered_status, reordered_receipt = evaluate(compiler, reordered_match)
if reordered_status != 0 or evaluate(reordered_receipt) != (0, b"\x07"):
    raise SystemExit("exhaustive reordered match changed meaning")

parenthesized_nullary = b"""(data Choice (Left) (Right))
(def choose ((value Choice)) Int
  (match value ((Left) 7) ((Right) 9)))
(def main () Int (choose Left))
"""
parenthesized_status, parenthesized_receipt = evaluate(
    compiler, parenthesized_nullary
)
if parenthesized_status != 0 or evaluate(parenthesized_receipt) != (0, b"\x07"):
    raise SystemExit("parenthesized nullary pattern did not compile")

proper_tail = b"""(data List (Nil) (Cons Int List))
(def make ((n Int) (items List)) List
  (if (eq n 0) items
    (let next Int (- n 1) (make next (Cons 1 items)))))
(def walk ((items List)) Int
  (match items (Nil 0) ((Cons head tail) (walk tail))))
(def main () Int (walk (make 100000 Nil)))
"""
tail_status, tail_receipt = evaluate(compiler, proper_tail)
if tail_status != 0 or len(tail_receipt) != 568:
    raise SystemExit("proper-tail witness did not lower")
if hashlib.sha256(tail_receipt).hexdigest() != "1d0bfd24332845ab7a1c483b53398a4a6fa7503b366a2cbb5cb5c922b1952f73":
    raise SystemExit("proper-tail receipt identity changed")
if evaluate(tail_receipt) != (0, b"\x00"):
    raise SystemExit("tail calls through if, let, or match consumed context")

for name, expression in (
    ("maximum addition", b"(eq (+ 9223372036854775807 0) 9223372036854775807)"),
    ("minimum subtraction", b"(eq (- -9223372036854775808 0) -9223372036854775808)"),
    ("large multiplication", b"(eq (* 3037000499 3037000499) 9223372030926249001)"),
    ("negative multiplication", b"(eq (* -3 4) -12)"),
    ("zero multiplication", b"(eq (* -9223372036854775808 0) 0)"),
):
    arithmetic_source = b"(def main () Int " + expression + b")\n"
    arithmetic_status, arithmetic_receipt = evaluate(compiler, arithmetic_source)
    if arithmetic_status != 0 or evaluate(arithmetic_receipt) != (0, b"\x01"):
        raise SystemExit(f"checked {name} changed a representable result")

for name, expression in (
    ("positive addition overflow", b"(+ 9223372036854775807 1)"),
    ("negative addition overflow", b"(+ -9223372036854775808 -1)"),
    ("positive subtraction overflow", b"(- 9223372036854775807 -1)"),
    ("negative subtraction overflow", b"(- -9223372036854775808 1)"),
    ("positive multiplication overflow", b"(* 3037000500 3037000500)"),
    ("signed multiplication overflow", b"(* -9223372036854775808 -1)"),
    ("division by zero", b"(/ 1 0)"),
    ("signed division overflow", b"(/ -9223372036854775808 -1)"),
    ("remainder by zero", b"(% 1 0)"),
    ("signed remainder overflow", b"(% -9223372036854775808 -1)"),
):
    arithmetic_source = b"(def main () Int " + expression + b")\n"
    arithmetic_status, arithmetic_receipt = evaluate(compiler, arithmetic_source)
    if arithmetic_status != 0 or evaluate(arithmetic_receipt) != (2, b""):
        raise SystemExit(f"checked {name} did not trap")

bytes_runtime = b"""(def $dbe () Int (pair 0 (pair 0 0)))
(def $dbs ((v Int)) Int (if (lt v 0) (/ 1 0) (if (lt v 256) (pair 1 (pair 1 v)) (/ 1 0))))
(def $dbl ((v Int)) Int (first v))
(def $dbc ((l Int) (r Int)) Int (let ll Int (first l) (let rl Int (first r) (let n Int (+ ll rl) (if (lt n ll) (/ 1 0) (pair n (pair 2 (pair l r))))))))
(def $dbg ((v Int) (i Int)) Int (if (lt i 0) (/ 1 0) (if (lt i (first v)) ($dbgi v i) (/ 1 0))))
(def $dbgi ((v Int) (i Int)) Int (let n Int (second v) (let t Int (first n) (if (eq t 1) (second n) (if (eq t 2) (let c Int (second n) (let l Int (first c) (let z Int (first l) (if (lt i z) ($dbgi l i) ($dbgi (second c) (- i z)))))) (/ 1 0))))))
"""

bytes_core = b"""(data Box (Box Bytes))
(def keep ((value Bytes)) Bytes (let retained Bytes value (if 1 retained value)))
(def unwrap ((value Box)) Bytes (match value ((Box payload) payload)))
(def main () Int
  (+ (bytes_length (bytes_empty))
    (bytes_get
      (bytes_concat (bytes_empty)
        (keep (unwrap (Box
          (bytes_concat (bytes_single 65) (bytes_single 66))))))
      1)))
"""
bytes_status, bytes_receipt = evaluate(compiler, bytes_core)
if bytes_status != 0 or not bytes_receipt.startswith(bytes_runtime):
    raise SystemExit("typed Bytes program did not lower through its private runtime")
if evaluate(bytes_receipt) != (0, b"\x42"):
    raise SystemExit("Bytes typing, construction, length, concatenation, or indexing failed")

byte_extrema = b"""(def main () Int
  (if (eq (bytes_get (bytes_single 0) 0) 0)
    (if (eq (bytes_get (bytes_single 255) 0) 255)
      (eq (bytes_length (bytes_concat (bytes_single 7) (bytes_empty))) 1)
      0)
    0))
"""
extrema_status, extrema_receipt = evaluate(compiler, byte_extrema)
if extrema_status != 0 or evaluate(extrema_receipt) != (0, b"\x01"):
    raise SystemExit("Bytes singleton extrema changed")

bytes_type_only = (
    b"(def keep ((value Bytes)) Bytes value)\n"
    b"(def main () Int 7)\n"
)
type_only_status, type_only_receipt = evaluate(compiler, bytes_type_only)
if type_only_status != 0 or b"$dbe" in type_only_receipt:
    raise SystemExit("Bytes type-only program acquired an unused runtime")
if evaluate(type_only_receipt) != (0, b"\x07"):
    raise SystemExit("Bytes type-only program changed scalar execution")

for name, expression in (
    ("negative singleton", b"(bytes_length (bytes_single -1))"),
    ("oversized singleton", b"(bytes_length (bytes_single 256))"),
    ("empty lookup", b"(bytes_get (bytes_empty) 0)"),
    ("negative lookup", b"(bytes_get (bytes_single 1) -1)"),
    ("equal-to-length lookup", b"(bytes_get (bytes_single 1) 1)"),
    ("past-length lookup", b"(bytes_get (bytes_single 1) 2)"),
):
    trap_source = b"(def main () Int " + expression + b")\n"
    trap_status, trap_receipt = evaluate(compiler, trap_source)
    if trap_status != 0 or evaluate(trap_receipt) != (2, b""):
        raise SystemExit(f"Bytes {name} did not trap")

logical_overflow = b"""(def double ((remaining Int) (value Bytes)) Bytes
  (if (eq remaining 0)
    (bytes_concat value value)
    (double (- remaining 1) (bytes_concat value value))))
(def main () Int (bytes_length (double 62 (bytes_single 1))))
"""
overflow_status, overflow_receipt = evaluate(compiler, logical_overflow)
if overflow_status != 0 or evaluate(overflow_receipt) != (2, b""):
    raise SystemExit("Bytes logical-length overflow did not trap")

deep_rope = b"""(def grow ((remaining Int) (value Bytes)) Bytes
  (if (eq remaining 0)
    value
    (grow (- remaining 1) (bytes_concat (bytes_empty) value))))
(def main () Int (bytes_get (grow 100000 (bytes_single 90)) 0))
"""
deep_status, deep_receipt = evaluate(compiler, deep_rope)
if deep_status != 0 or evaluate(deep_receipt) != (0, b"Z"):
    raise SystemExit("deep Bytes lookup consumed non-tail call context")

malformed = {
    "unknown field type": b"(data Bad (Bad Missing))\n(def main () Int 0)\n",
    "missing payload argument": b"(data Option (None) (Some Int))\n(def main () Int (Some))\n",
    "missing payload binder": b"(data Option (None) (Some Int))\n(def main () Int (match None (None 0) (Some 1)))\n",
    "non-exhaustive match": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Left 7)))\n",
    "duplicate match arm": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Left 7) (Left 9)))\n",
    "nullary pattern binder": b"(data Choice (Left) (Right))\n(def main () Int (match Left ((Left x) 7) (Right 9)))\n",
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
    "function argument type": b"(data Box (Box Int))\n(def f ((x Int)) Int x)\n(def main () Int (f (Box 1)))\n",
    "constructor argument type": b"(data Box (Box Int))\n(def main () Box (Box (Box 1)))\n",
    "let initializer type": b"(data Box (Box Int))\n(def main () Int (let x Int (Box 1) x))\n",
    "declared result type": b"(data Box (Box Int))\n(def main () Int (Box 1))\n",
    "if condition type": b"(data Box (Box Int))\n(def main () Int (if (Box 1) 1 2))\n",
    "if branch agreement": b"(data Box (Box Int))\n(def main () Int (if 1 1 (Box 2)))\n",
    "operator operand type": b"(data Box (Box Int))\n(def main () Int (+ (Box 1) 2))\n",
    "match scrutinee type": b"(data Box (Box Int))\n(def main () Int (match 1 ((Box x) x)))\n",
    "match constructor owner": b"(data A (A))\n(data B (B))\n(def main () Int (match A (B 1)))\n",
    "match arm type agreement": b"(data Choice (Left) (Right))\n(data Box (Box Int))\n(def main () Int (match Left (Left 1) (Right (Box 2))))\n",
    "Bytes used as Int": b"(def main () Int (bytes_empty))\n",
    "Int used as Bytes": b"(def keep ((x Bytes)) Bytes x)\n(def main () Int (bytes_length (keep 1)))\n",
    "bytes_single argument type": b"(data Box (Box))\n(def main () Int (bytes_length (bytes_single Box)))\n",
    "bytes_length argument type": b"(def main () Int (bytes_length 1))\n",
    "bytes_get value type": b"(def main () Int (bytes_get 1 0))\n",
    "bytes_get index type": b"(def main () Int (bytes_get (bytes_empty) (bytes_empty)))\n",
    "bytes_concat left type": b"(def main () Int (bytes_length (bytes_concat 1 (bytes_empty))))\n",
    "bytes_concat right type": b"(def main () Int (bytes_length (bytes_concat (bytes_empty) 1)))\n",
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
    "excess bytes_empty argument": b"(def main () Int (bytes_length (bytes_empty 0)))\n",
    "missing bytes_single argument": b"(def main () Int (bytes_length (bytes_single)))\n",
    "excess bytes_single argument": b"(def main () Int (bytes_length (bytes_single 0 1)))\n",
    "missing bytes_length argument": b"(def main () Int (bytes_length))\n",
    "excess bytes_length argument": b"(def main () Int (bytes_length (bytes_empty) (bytes_empty)))\n",
    "excess bytes_get argument": b"(def main () Int (bytes_get (bytes_empty) 0 1))\n",
    "missing bytes_concat argument": b"(def main () Int (bytes_length (bytes_concat (bytes_empty))))\n",
    "excess bytes_concat argument": b"(def main () Int (bytes_length (bytes_concat (bytes_empty) (bytes_empty) (bytes_empty))))\n",
}
for name, candidate in malformed.items():
    status, output = evaluate(compiler, candidate)
    if status != 2:
        raise SystemExit(f"{name} did not trap in the staged compiler")
    if output:
        raise SystemExit(f"{name} emitted before static rejection")

for name, candidate, authored_receipt, observation in emitter_fixtures():
    status, receipt = evaluate(compiler, candidate)
    if status != 0:
        raise SystemExit(f"emitter context {name}: compilation failed with {status}")
    if authored_receipt is not None and receipt != authored_receipt:
        raise SystemExit(f"emitter context {name}: authored receipt differs")
    if evaluate(compiler, candidate) != (0, receipt):
        raise SystemExit(f"emitter context {name}: repeated receipt differs")
    if evaluate(receipt) != (0, observation):
        raise SystemExit(f"emitter context {name}: generated observation differs")

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

echo "Staged Delta compiler: checked lowering and ConformanceBytesV1 profiles pass"
