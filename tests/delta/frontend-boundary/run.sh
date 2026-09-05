#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/evaluator_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta frontend boundary: skipped (python3 absent)"
    exit 0
}

FRONTEND_BOUNDARY_TMP=$(mktemp -d)
trap 'rm -rf -- "$FRONTEND_BOUNDARY_TMP"' EXIT HUP INT TERM
python3 "$OMEGA_REPO_ROOT/tools/bootstrap/source_closure.py" \
    "$OMEGA_PATH_DELTA_COMPILER_SOURCES" "$FRONTEND_BOUNDARY_TMP/compiler.gamma" \
    --prefix "$OMEGA_PATH_DELTA_COMPILER_SOURCE"
materialize_gamma_evaluator "$FRONTEND_BOUNDARY_TMP/evaluator" >/dev/null

FRONTEND_BOUNDARY_TMP="$FRONTEND_BOUNDARY_TMP" python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path

directory = Path(os.environ["FRONTEND_BOUNDARY_TMP"])
compiler = (directory / "compiler.gamma").read_bytes()
identity = (len(compiler.splitlines()), len(compiler), hashlib.sha256(compiler).hexdigest())
if identity != (
    2693, 111236, "48526e2713778321efcc15121b70269c4c7d91cf007e31cad53f66ff18e47671"
):
    raise SystemExit(f"Delta compiler identity changed: {identity}")

REQUEST_MAGIC = b"DCREQ\x01\x00\x00"
OUTCOME_MAGIC = b"\xffDCOUT\x01\x00"
identity_source = b"(def main ((source Bytes)) Bytes source)\n"


def evaluate(program, sealed_input):
    process = subprocess.Popen(
        [str(directory / "evaluator")], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(
            struct.pack("<I", len(program)) + program + sealed_input, timeout=30
        )
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("Delta frontend boundary: selected Gamma timed out")
    if error:
        raise SystemExit(f"unexpected evaluator stderr: {error!r}")
    return process.returncode, output


def request(source):
    return REQUEST_MAGIC + struct.pack("<II", 1, len(source)) + source


def rejection(code, coordinate, space=1):
    frame = struct.pack(
        "<8sBBHIQQQ", OUTCOME_MAGIC, 1, space, 0, code, coordinate, 0, 0
    )
    assert len(frame) == 40
    return 1, frame


cases = []
for byte in (0, 1, 8, 11, 12, 14, 31, 127, 128, 255):
    cases.append((f"invalid source byte {byte}", bytes([byte]) + identity_source,
                  rejection(3, 0)))
cases.extend((
    ("Unicode BOM is source bytes", b"\xef\xbb\xbf" + identity_source, rejection(3, 0)),
    ("invalid byte inside comment", b"; text\x00\n" + identity_source, rejection(3, 6)),
    ("source byte before syntax", b"(\x00", rejection(3, 1)),
    ("first invalid source byte", b" \x00\xff" + identity_source, rejection(3, 1)),
    ("source byte before duplicate", b"\x00(def f () Int 0)\n(def f () Int 1)\n",
     rejection(3, 0)),
    ("later source byte before duplicate collection",
     b"(def f () Int 0)\n(def f () Int 1)\n\x00", rejection(3, 34)),
))

# Literal authored coordinates are fixture expectations. The host does not
# tokenize source, locate declarations, or select a diagnostic candidate.
for name, source, code, coordinate in (
    ("leading plus is not an integer", b"(def main () Int +123)\n", 4, 17),
    ("numeric suffix is malformed", b"(def main () Int 12suffix)\n", 4, 17),
    ("double minus is malformed", b"(def main () Int --1)\n", 4, 17),
    ("decimal point is malformed", b"(def main () Int 1.0)\n", 4, 17),
    ("punctuation is malformed", b"(def main () Int @)\n", 4, 17),
    ("positive adjacent integer overflow",
     b"(def main () Int 9223372036854775808)\n", 5, 17),
    ("negative adjacent integer overflow",
     b"(def main () Int -9223372036854775809)\n", 5, 17),
    ("long all-digit integer overflow",
     b"(def main () Int 9999999999999999999999999999999999999999)\n", 5, 17),
    ("suffix after overflowing prefix is malformed",
     b"(def main () Int 9999999999999999999999999999999999999999suffix)\n", 4, 17),
    ("lexical failure before later duplicate",
     b"(def f () Int +123)\n(def f () Int 0)\n", 4, 14),
    ("later lexical failure before duplicate collection",
     b"(def f () Int 0)\n(def f () Int +123)\n", 4, 31),
    ("lexical failure before unknown declaration type",
     b"(def main () Unknown +123)\n", 4, 21),
    ("later forbidden byte before earlier malformed token", b"+123\x00", 3, 4),
    ("first malformed token wins", b" \t12suffix +123", 4, 2),
    ("earlier overflow before later malformed token", b"9223372036854775808 +123", 5, 0),
    ("earlier malformed token before later overflow", b"+123 9223372036854775808", 4, 0),
):
    cases.append((name, source, rejection(code, coordinate)))

for name, source, code, coordinate in (
    ("duplicate type", b"(data Item (First))\n(data Item (Second))\n(def main ((source Bytes)) Bytes source)\n", 6, 26),
    ("duplicate constructor across owners", b"(data First (Item))\n(data Second (Item))\n(def main ((source Bytes)) Bytes source)\n", 7, 34),
    ("duplicate constructor in owner", b"(data First (Item) (Item))\n(def main ((source Bytes)) Bytes source)\n", 7, 20),
    ("duplicate function", b"(def helper () Int 1)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 27),
    ("duplicate main", b"(def main ((source Bytes)) Bytes source)\n(def main () Int 1)\n", 8, 46),
    ("unknown constructor type before duplicate type", b"(data First (One Unknown))\n(data First (Two))\n(def main ((source Bytes)) Bytes source)\n", 6, 33),
    ("unknown constructor type before duplicate constructor", b"(data First (One Unknown))\n(data Second (One))\n(def main ((source Bytes)) Bytes source)\n", 7, 41),
    ("unknown signature type before duplicate function", b"(def helper ((value Unknown)) Int 1)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 42),
    ("duplicate before unknown constructor type", b"(data First (One))\n(data First (Two Unknown))\n(def main ((source Bytes)) Bytes source)\n", 6, 25),
    ("duplicate before unknown signature type", b"(def helper () Int 1)\n(def helper ((value Unknown)) Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 27),
    ("body error before duplicate function", b"(def helper () Int missing)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 8, 33),
    ("earliest duplicate across namespaces", b"(data Item (One))\n(data Item (Two))\n(def helper () Int 1)\n(def helper () Int 2)\n(def main ((source Bytes)) Bytes source)\n", 6, 24),
    ("earlier constructor duplicate before later type duplicate", b"(data First (One))\n(data Second (One))\n(data First (Two))\n(def main ((source Bytes)) Bytes source)\n", 7, 33),
    ("wrong entry arity", b"(def main () Int 7)\n", 20, 5),
    ("wrong entry parameter", b"(def main ((source Int)) Bytes (bytes_empty))\n", 20, 5),
    ("wrong entry result", b"(def main ((source Bytes)) Int 7)\n", 20, 5),
    ("wrong entry two parameters", b"(def main ((left Bytes) (right Bytes)) Bytes left)\n", 20, 5),
    ("wrong entry name after declarations and comments",
     b"; prefix\n(data Item (Item))\n(def helper () Int 0)\n"
     b"(def ; entry\n  main () Int 7)\n", 20, 65),
):
    cases.append((name, source, rejection(code, coordinate)))

cases.append(("valid frontend without main", b"(def helper () Int 0)\n",
              rejection(19, 0, space=0)))
cases.append(("entry prefix is not main", b"(def mai () Int 0)\n",
              rejection(19, 0, space=0)))
cases.append(("entry suffix is not main", b"(def main_suffix () Int 0)\n",
              rejection(19, 0, space=0)))

# Each prefix is authored as part of the fixture construction, not discovered
# by parsing or searching its source. The next byte is the expected location:
# an offending child, the containing close for a missing child, or exact EOF.
structural = (
    ("empty source", b"", b"", b""),
    ("comment-only source", b"; no declarations", b"", b""),
    ("open delimiter at EOF", b"(", b"", b""),
    ("unclosed function at EOF", b"(def main () Int 0\n", b"", b""),
    ("unmatched closing delimiter", b"", b")", b""),
    ("later unmatched close before earlier name role",
     b"(def Main () Int 0)\n", b")", b""),
    ("later unmatched open before earlier name role",
     b"(def Main () Int 0)\n(", b"", b""),
    ("empty top-level list", b"(", b")", b""),
    ("top-level atom", b"", b"value", b""),
    ("nested declaration head", b"(", b"(def)", b" main () Int 0)"),
    ("data after function", b"(def helper () Int 0)\n",
     b"(data Item (Item))", b""),
    ("data without any function", b"(data Item (Item))\n", b"", b""),
    ("empty data declaration", b"(data Item", b")", b""),
    ("lowercase data name", b"(data ", b"item", b" (Item))"),
    ("constructor declaration is a list", b"(data Item ", b"Item", b")"),
    ("empty constructor declaration", b"(data Item (", b")", b")"),
    ("lowercase constructor name", b"(data Item (", b"item", b"))"),
    ("payload type name role", b"(data Item (Item ", b"lower", b"))"),
    ("uppercase function name", b"(def ", b"Main", b" () Int 0)"),
    ("reserved function name", b"(def ", b"if", b" () Int 0)"),
    ("parameter collection is a list", b"(def main ", b"value", b" Int 0)"),
    ("parameter row is a list", b"(def main (", b"value", b") Int 0)"),
    ("parameter missing type", b"(def main ((value", b")", b") Int 0)"),
    ("parameter extra child", b"(def main ((value Int ", b"Bytes", b")) Int 0)"),
    ("uppercase parameter name", b"(def main ((", b"Value", b" Int)) Int 0)"),
    ("parameter type name role", b"(def main ((value ", b"lower", b")) Int 0)"),
    ("result type name role", b"(def main () ", b"lower", b" 0)"),
    ("function missing body", b"(def main () Int", b")", b""),
    ("function extra body", b"(def main () Int 0 ", b"1", b")"),
    ("if missing branch", b"(def main () Int (if 1 0", b")", b")"),
    ("if extra branch", b"(def main () Int (if 1 0 0 ", b"1", b"))"),
    ("let missing body", b"(def main () Int (let value Int 0", b")", b")"),
    ("let extra body", b"(def main () Int (let value Int 0 value ", b"1", b"))"),
    ("let reserved binder", b"(def main () Int (let ", b"if", b" Int 0 0))"),
    ("let type name role", b"(def main () Int (let value ", b"lower", b" 0 0))"),
    ("match requires an arm", b"(def main () Int (match Item", b")", b")"),
    ("match arm is a list", b"(def main () Int (match Item ", b"Item", b"))"),
    ("match arm missing body", b"(def main () Int (match Item (Item", b")", b"))"),
    ("match arm extra body", b"(def main () Int (match Item (Item 0 ", b"1", b")))"),
    ("lowercase atomic pattern", b"(def main () Int (match Item (", b"item", b" 0)))"),
    ("empty payload pattern", b"(def main () Int (match Item ((", b")", b" 0)))"),
    ("pattern binder name role", b"(def main () Int (match Item ((Item ", b"Value", b") 0)))"),
    ("empty expression list", b"(def main () Int (", b")", b")"),
    ("nested expression head", b"(def main () Int (", b"(helper)", b" 0))"),
    ("bare subtraction token is not an atom", b"(def main () Int ", b"-", b")\n"),
    ("reserved word is not an atom", b"(def main () Int ", b"if", b")"),
    ("reserved declaration word is not a call head", b"(def main () Int (", b"data", b" 0))"),
    ("later role defect before duplicate collection",
     b"(def helper () Int 0)\n(def helper () Int 0)\n(def ", b"Main", b" () Int 0)"),
    ("role defect before unknown declaration type",
     b"(def main ((value Unknown)) Int ", b"-", b")"),
)
for name, prefix, offending, suffix in structural:
    cases.append((name, prefix + offending + suffix, rejection(4, len(prefix))))

# The parser and role walk both traverse all 1,000 nested call nodes before
# rejecting the inner operator atom. The host constructs, but never parses, it.
deep_prefix = b"(def main () Int " + b"(helper " * 1000
cases.append(("deep structural traversal", deep_prefix + b"-" + b")" * 1001,
              rejection(4, len(deep_prefix))))

# These unfinished frontend paths must remain evaluator-owned failures, not
# guessed DCOUT frames or schema diagnostics derived before frontend success.
for name, source in (
    ("unknown signature type without main", b"(def helper ((value Unknown)) Int 0)\n"),
    ("unknown constructor type without main", b"(data Item (Item Unknown))\n(def helper () Int 0)\n"),
    ("unknown body name without main", b"(def helper () Int missing)\n"),
    ("wrong present entry with body error", b"(def main () Int missing)\n"),
    ("wrong present entry with body type error", b"(def main () Int (bytes_empty))\n"),
    ("ordinary body error before valid entry", b"(def helper () Int missing)\n" + identity_source),
    ("known function argument count",
     b"(def helper ((value Int)) Int value)\n(def main () Int (helper))\n"),
    ("known constructor argument count",
     b"(data Item (Item Int))\n(def main () Item (Item))\n"),
    ("known pattern payload count",
     b"(data Item (Item Int))\n(def main () Int (match (Item 0) ((Item) 0)))\n"),
    ("arithmetic argument count", b"(def main () Int (+ 1))\n"),
    ("bytes builtin argument count", b"(def main () Bytes (bytes_single))\n"),
):
    cases.append((name, source, (249, b"")))

for name, source, expected in cases:
    actual = evaluate(compiler, request(source))
    if actual != expected:
        raise SystemExit(
            f"{name}: expected status {expected[0]} and {expected[1].hex()}, "
            f"got status {actual[0]}, {len(actual[1])} bytes, prefix {actual[1][:80].hex()}"
        )

accepted = (
    ("identity", identity_source),
    ("exact entry after longer function name",
     b"(def main_suffix () Int 0)\n" + identity_source),
    ("grammar-distinguished namespace reuse",
     b"(data Token (Token Int))\n"
     b"(def f ((f Bytes)) Bytes f)\n"
     b"(def main ((source Bytes)) Bytes (f source))\n"),
    ("forward and mutually visible data",
     b"(data Left (End) (ToRight Right))\n"
     b"(data Right (ToLeft Left))\n" + identity_source),
    ("forward and mutually visible functions",
     b"(def first ((value Int)) Int (if value (second 0) 0))\n"
     b"(def second ((value Int)) Int (if value (first 0) 0))\n"
     b"(def main ((source Bytes)) Bytes (if (first 1) (bytes_empty) source))\n"),
    ("textual ASCII whitespace and comment endings",
     b"; comment\r\t" + identity_source + b"; final comment"),
    ("exact minimum integer",
     b"(def main ((source Bytes)) Bytes "
     b"(if (eq -9223372036854775808 -9223372036854775808) source (bytes_empty)))\n"),
    ("exact maximum integer",
     b"(def main ((source Bytes)) Bytes "
     b"(if (eq 9223372036854775807 9223372036854775807) source (bytes_empty)))\n"),
    ("negative zero and leading zeros",
     b"(def main ((source Bytes)) Bytes "
     b"(if (eq (+ -0 0007) 7) source (bytes_empty)))\n"),
    ("binary addition and subtraction tokens",
     b"(def main ((source Bytes)) Bytes "
     b"(if (eq (+ (- 7 3) 2) 6) source (bytes_empty)))\n"),
    ("malformed numbers in LF comment",
     b"; +123 12suffix --1 1.0 9223372036854775808\n" + identity_source),
    ("malformed numbers in CR comment",
     b"; +123 12suffix --1 1.0 9223372036854775808\r" + identity_source),
    ("malformed numbers in CRLF comment",
     b"; +123 12suffix --1 1.0 9223372036854775808\r\n" + identity_source),
    ("malformed numbers in EOF comment",
     identity_source + b"; +123 12suffix --1 1.0 9223372036854775808"),
)
# Wide ordinary declarations and calls exercise counted child spines without
# selecting a private arity limit. The last argument preserves binary input.
wide_parameters = b" ".join(f"(value{index} Bytes)".encode("ascii") for index in range(200))
wide_arguments = b" ".join([b"source"] * 200)
accepted += (("two-hundred parameter and argument identity",
              b"(def wide (" + wide_parameters + b") Bytes value199)\n"
              b"(def main ((source Bytes)) Bytes (wide " + wide_arguments + b"))\n"),)
payload = b"\x00A\x80\xff"
for name, source in accepted:
    status, receipt = evaluate(compiler, request(source))
    if status != 0 or not receipt:
        raise SystemExit(f"{name}: expected nonempty successful receipt, got {status}, {receipt[:80].hex()}")
    if evaluate(compiler, request(source)) != (0, receipt):
        raise SystemExit(f"{name}: compilation did not reconstruct identical bytes")
    if evaluate(receipt, payload) != (0, payload):
        raise SystemExit(f"{name}: compiled application did not preserve exact binary input")

frames = sum(len(expected[1]) == 40 for _, _, expected in cases)
print(
    f"Delta frontend boundary: {frames} exact DCOUT controls, "
    f"{len(cases) - frames} evaluator-owned failures, "
    f"{len(accepted)} repeated accepted compilations and application observations passed"
)
PY
