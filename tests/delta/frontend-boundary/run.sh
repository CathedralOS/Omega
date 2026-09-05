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

FRONTEND_BOUNDARY_TMP="$FRONTEND_BOUNDARY_TMP" PYTHONPATH="$GATE_DIR" python3 -B - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path
from depth_fixtures import fixtures as depth_fixtures
from name_fixtures import fixtures as name_fixtures
from name_roles import fixtures as name_roles
from census_cursors import fixtures as census_cursors
from parameter_cursors import fixtures as parameter_cursors
from catalog_replacements import fixtures as catalog_replacements

directory = Path(os.environ["FRONTEND_BOUNDARY_TMP"])
compiler = (directory / "compiler.gamma").read_bytes()
identity = (len(compiler.splitlines()), len(compiler), hashlib.sha256(compiler).hexdigest())
if identity != (
    3381, 154591, "8d50deb306f03dd7a25c1aefc91f3701e8e2f8711d2e0e265058cd38864a145d"
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

# Declaration judgments consume the completed global census and retained nodes.
# Prefixes author exact type/binder coordinates; no host lookup chooses a reason.
declarations = (
    ("unknown parameter type without main", 11,
     b"(def helper ((value ", b"Unknown", b")) Int 0)\n"),
    ("unknown result type without main", 11,
     b"(def helper () ", b"Unknown", b" 0)\n"),
    ("unknown constructor field without main", 11,
     b"(data Item (Item ", b"Unknown", b"))\n(def helper () Int 0)\n"),
    ("unknown later constructor field", 11,
     b"(data Item (Item Int Bytes ", b"Unknown", b"))\n" + identity_source),
    ("unknown field in later constructor", 11,
     b"(data Item (Empty) (Full Int ", b"Unknown", b"))\n" + identity_source),
    ("constructor spelling does not declare a type", 11,
     b"(data Item (OnlyConstructor))\n(def helper ((value ",
     b"OnlyConstructor", b")) Int 0)\n"),
    ("type prefix is not an exact declaration", 11,
     b"(data ItemLong (Empty))\n(def helper () ", b"Item", b" Empty)\n"),
    ("type suffix is not an exact declaration", 11,
     b"(data Item (Empty))\n(def helper () ", b"ItemLong", b" Empty)\n"),
    ("first parameter type before later parameter type", 11,
     b"(def helper ((first ", b"Unknown", b") (second Missing)) Int 0)\n"),
    ("parameter type before result type", 11,
     b"(def helper ((value ", b"Unknown", b")) Missing 0)\n"),
    ("earlier constructor type before later signature", 11,
     b"(data Item (Item ", b"Unknown", b"))\n(def helper () Missing 0)\n"),
    ("earlier function result before later parameter", 11,
     b"(def first () ", b"Unknown", b" 0)\n(def second ((value Missing)) Int 0)\n"),
    ("later signature before earlier body failure", 11,
     b"(def first () Int missing)\n(def second () ", b"Unknown", b" 0)\n"),
    ("unknown main signature before entry schema", 11,
     b"(def main ((source Bytes)) ", b"Unknown", b" source)\n"),
    ("type coordinate after comments and whitespace", 11,
     b"; leading\r\n(def helper ((value ; annotation\r\n\t",
     b"Unknown", b")) Int 0)\n"),
    ("duplicate parameter without main", 9,
     b"(def helper ((value Int) (", b"value", b" Int)) Int 0)\n"),
    ("duplicate parameter before its unknown type", 9,
     b"(def helper ((value Int) (", b"value", b" Unknown)) Int 0)\n"),
    ("earlier unknown type before duplicate parameter", 11,
     b"(def helper ((value ", b"Unknown", b") (value Int)) Int 0)\n"),
    ("nonadjacent parameter conflict", 9,
     b"(def helper ((value Int) (other Bytes) (", b"value", b" Int)) Int 0)\n"),
    ("duplicate parameter before unknown result and body", 9,
     b"(def helper ((value Int) (", b"value", b" Int)) Unknown missing)\n"),
    ("later parameter conflict before earlier body failure", 9,
     b"(def first () Int missing)\n(def second ((value Int) (",
     b"value", b" Int)) Int 0)\n"),
    ("global duplicate before earlier parameter conflict", 8,
     b"(def helper ((value Int) (value Int)) Int 0)\n(def ",
     b"helper", b" () Int 0)\n"),
)
for name, code, prefix, offending, suffix in declarations:
    cases.append((name, prefix + offending + suffix, rejection(code, len(prefix))))

# Body typing owns exact judgments after declaration resolution. These are
# authored prefix/suffix pairs, not coordinates recovered by a host parser.
semantic = (
    ("unknown let annotation", 11,
     b"(def helper () Int (let value ", b"Unknown 0 0))\n"),
    ("active let conflict", 9,
     b"(def helper ((value Int)) Int (let ", b"value Int 0 value))\n"),
    ("unknown body name without main", 14,
     b"(def helper () Int ", b"missing)\n"),
    ("wrong present entry with body error", 14,
     b"(def main () Int ", b"missing)\n"),
    ("wrong present entry with body type error", 15,
     b"(def main () Int ", b"(bytes_empty))\n"),
    ("ordinary body error before valid entry", 14,
     b"(def helper () Int ", b"missing)\n(def main ((source Bytes)) Bytes source)\n"),
    ("known function argument count", 16,
     b"(def helper ((value Int)) Int value)\n(def main () Int ", b"(helper))\n"),
    ("known constructor argument count", 16,
     b"(data Item (Item Int))\n(def main () Item ", b"(Item))\n"),
    ("known pattern payload count", 16,
     b"(data Item (Item Int))\n(def main () Int (match (Item 0) (", b"(Item) 0)))\n"),
    ("arithmetic argument count", 16,
     b"(def main () Int ", b"(+ 1))\n"),
    ("bytes builtin argument count", 16,
     b"(def main () Bytes ", b"(bytes_single))\n"),
    ("unknown function name", 13,
     b"(def probe () Int (", b"missing 0))\n"),
    ("unknown function precedes argument", 13,
     b"(def probe () Int (", b"missing other))\n"),
    ("unknown constructor atom", 12,
     b"(def probe () Int ", b"Missing)\n"),
    ("unknown constructor application", 12,
     b"(def probe () Int (", b"Missing other))\n"),
    ("unknown pattern constructor", 12,
     b"(data Choice (First))\n(def probe () Int (match First (", b"Missing missing)))\n"),
    ("function name is not a value", 14,
     b"(def helper () Int 0)\n(def probe () Int ", b"helper)\n"),
    ("unknown local spelling prefix", 14,
     b"(def probe ((value_long Int)) Int ", b"value)\n"),
    ("unknown local spelling suffix", 14,
     b"(def probe ((value Int)) Int ", b"value_long)\n"),
    ("let initializer uses outer environment", 14,
     b"(def probe () Int (let value Int ", b"value value))\n"),
    ("let annotation precedes conflict", 11,
     b"(def probe ((value Int)) Int (let value ", b"Unknown missing value))\n"),
    ("let conflict precedes initializer", 9,
     b"(def probe ((value Int)) Int (let ", b"value Int missing value))\n"),
    ("nested active let conflict", 9,
     b"(def probe () Int (let value Int 0 (let ", b"value Int 1 value)))\n"),
    ("let initializer type mismatch", 15,
     b"(def probe () Int (let value Int ", b"(bytes_empty) value))\n"),
    ("let local does not escape expression", 14,
     b"(def probe () Int (+ (let value Int 0 value) ", b"value))\n"),
    ("if condition type mismatch", 15,
     b"(def probe () Int (if ", b"(bytes_empty) 0 1))\n"),
    ("if condition precedes branch", 15,
     b"(def probe () Int (if ", b"(bytes_empty) missing other))\n"),
    ("if branch type mismatch", 15,
     b"(def probe () Int (if 1 0 ", b"(bytes_empty)))\n"),
    ("if true branch error precedes false branch", 14,
     b"(def probe () Int (if 0 ", b"missing (bytes_empty)))\n"),
    ("if unselected branch still checked", 14,
     b"(def probe () Int (if 1 0 ", b"missing))\n"),
    ("operator first argument type mismatch", 15,
     b"(def probe () Int (+ ", b"(bytes_empty) 0))\n"),
    ("operator second argument type mismatch", 15,
     b"(def probe () Int (+ 0 ", b"(bytes_empty)))\n"),
    ("operator expected argument before missing argument", 14,
     b"(def probe () Int (+ ", b"missing))\n"),
    ("operator extra argument not typed", 16,
     b"(def probe () Int ", b"(+ 0 1 missing))\n"),
    ("function first argument before later type", 15,
     b"(def helper ((first Int) (second Int)) Int first)\n(def probe () Int (helper ", b"(bytes_empty) missing))\n"),
    ("function expected argument before missing argument", 14,
     b"(def helper ((first Int) (second Int)) Int first)\n(def probe () Int (helper ", b"missing))\n"),
    ("function extra argument not typed", 16,
     b"(def helper ((value Int)) Int value)\n(def probe () Int ", b"(helper 0 missing))\n"),
    ("zero arity function extra argument not typed", 16,
     b"(def helper () Int 0)\n(def probe () Int ", b"(helper missing))\n"),
    ("constructor expected argument before missing argument", 14,
     b"(data Pair (Pair Int Int))\n(def probe () Pair (Pair ", b"missing))\n"),
    ("constructor argument type mismatch", 15,
     b"(data Pair (Pair Int Bytes))\n(def probe () Pair (Pair 0 ", b"1))\n"),
    ("constructor extra argument not typed", 16,
     b"(data Item (Item Int))\n(def probe () Item ", b"(Item 0 missing))\n"),
    ("nonnullary constructor atom arity", 16,
     b"(data Item (Item Int))\n(def probe () Item ", b"Item)\n"),
    ("nominal types remain distinct", 15,
     b"(data First (First Int))\n(data Second (Second Int))\n(def helper ((value First)) Int 0)\n(def probe () Int (helper ", b"(Second 0)))\n"),
    ("bytes single argument mismatch", 15,
     b"(def probe () Int (bytes_single ", b"(bytes_empty)))\n"),
    ("bytes length argument mismatch", 15,
     b"(def probe () Int (bytes_length ", b"0))\n"),
    ("bytes get first type before second name", 15,
     b"(def probe () Int (bytes_get ", b"0 missing))\n"),
    ("bytes get second argument mismatch", 15,
     b"(def probe () Int (bytes_get (bytes_empty) ", b"(bytes_empty)))\n"),
    ("bytes concat second argument mismatch", 15,
     b"(def probe () Int (bytes_concat (bytes_empty) ", b"0))\n"),
    ("bytes empty extra argument not typed", 16,
     b"(def probe () Int ", b"(bytes_empty missing))\n"),
    ("declared function body result mismatch", 15,
     b"(def probe () Int ", b"(bytes_empty))\n"),
    ("first body error before later body error", 14,
     b"(def first () Int ", b"missing)\n(def second () Int other)\n"),
    ("match scrutinee mismatch", 15,
     b"(data Item (Item))\n(def probe () Int (match ", b"0 (Item 0)))\n"),
    ("match scrutinee error before pattern lookup", 14,
     b"(data Item (Item))\n(def probe () Int (match ", b"missing (Unknown 0)))\n"),
    ("pattern owner mismatch", 15,
     b"(data First (First))\n(data Second (Second))\n(def probe () Int (match First (", b"Second 0)))\n"),
    ("pattern owner precedes payload arity", 15,
     b"(data First (First))\n(data Second (Second Int))\n(def probe () Int (match First ((", b"Second) missing)))\n"),
    ("pattern arity precedes binders and body", 16,
     b"(data Item (Item Int))\n(def probe () Int (match (Item 0) (", b"(Item first second) missing)))\n"),
    ("duplicate pattern binder", 10,
     b"(data Pair (Pair Int Int))\n(def probe () Int (match (Pair 0 1) ((Pair value ", b"value) value)))\n"),
    ("pattern binder conflicts with outer parameter", 9,
     b"(data Item (Item Int))\n(def probe ((value Int)) Int (match (Item 0) ((Item ", b"value) value)))\n"),
    ("pattern binder conflicts with outer let", 9,
     b"(data Item (Item Int))\n(def probe () Int (let value Int 0 (match (Item 0) ((Item ", b"value) value))))\n"),
    ("pattern binder conflict precedes body", 9,
     b"(data Item (Item Int))\n(def probe ((value Int)) Int (match (Item 0) ((Item ", b"value) missing)))\n"),
    ("pattern local does not escape match", 14,
     b"(data Item (Item Int))\n(def probe () Int (+ (match (Item 0) ((Item value) value)) ", b"value))\n"),
    ("duplicate match case", 17,
     b"(data Choice (First) (Second))\n(def probe () Int (match First (First 0) (", b"First 1) (Second 2)))\n"),
    ("duplicate case precedes body", 17,
     b"(data Choice (First) (Second))\n(def probe () Int (match First (First 0) (", b"First missing)))\n"),
    ("pattern arity precedes duplicate case", 16,
     b"(data Item (Item Int))\n(def probe () Int (match (Item 0) ((Item first) first) (", b"(Item) missing)))\n"),
    ("match arm result mismatch", 15,
     b"(data Choice (First) (Second))\n(def probe () Int (match First (First 0) (Second ", b"(bytes_empty))))\n"),
    ("match body error precedes missing coverage", 14,
     b"(data Choice (First) (Second))\n(def probe () Int (match First (First ", b"missing)))\n"),
    ("nonexhaustive match", 18,
     b"(data Choice (First) (Second))\n(def probe () Int ", b"(match First (First 0)))\n"),
    ("nested missing coverage is independent", 18,
     b"(data Choice (First) (Second))\n(def probe () Int (match First (First ", b"(match Second (Second 0))) (Second 1)))\n"),
    ("unselected match arm still checked", 14,
     b"(data Choice (First) (Second))\n(def probe () Int (match First (First 0) (Second ", b"missing)))\n"),
    ("mixed constructor first field type", 15,
     b"(data Leaf (Leaf Int))\n(data Packet (Packet Int Bytes Leaf))\n(def probe () Packet (Packet ", b"(bytes_empty) (bytes_empty) (Leaf 0)))\n"),
    ("mixed constructor middle field type", 15,
     b"(data Leaf (Leaf Int))\n(data Packet (Packet Int Bytes Leaf))\n(def probe () Packet (Packet 0 ", b"1 (Leaf 0)))\n"),
    ("mixed constructor last field type", 15,
     b"(data Leaf (Leaf Int))\n(data Packet (Packet Int Bytes Leaf))\n(def probe () Packet (Packet 0 (bytes_empty) ", b"1))\n"),
    ("mixed pattern first binder type", 15,
     b"(data Leaf (Leaf Int))\n(data Packet (Packet Int Bytes Leaf))\n(def probe ((packet Packet)) Int (match packet ((Packet first middle last) (bytes_length ", b"first))))\n"),
    ("mixed pattern middle binder type", 15,
     b"(data Leaf (Leaf Int))\n(data Packet (Packet Int Bytes Leaf))\n(def probe ((packet Packet)) Int (match packet ((Packet first middle last) (+ ", b"middle 0))))\n"),
    ("mixed pattern last binder type", 15,
     b"(data Leaf (Leaf Int))\n(data Packet (Packet Int Bytes Leaf))\n(def probe ((packet Packet)) Int (match packet ((Packet first middle last) (bytes_length ", b"last))))\n"),
)
for name, code, prefix, suffix in semantic:
    cases.append((name, prefix + suffix, rejection(code, len(prefix))))

# Both structural and semantic traversal visit all nested nodes before the
# innermost unknown name rejects. Successful emission at this depth is not
# claimed: this fixture finishes in the checker, before Gamma emission.
deep_semantic_prefix = (
    b"(def helper ((value Int)) Int value)\n(def probe () Int "
    + b"(helper " * 1000
)
cases.append(("deep semantic traversal",
              deep_semantic_prefix + b"missing" + b")" * 1001,
              rejection(14, len(deep_semantic_prefix))))

deep_valid_source = (
    b"(def helper () Int " + b"(if 1 " * 1000 + b"0"
    + b" 0)" * 1000 + b")"
)
cases.append(("deep valid frontend before missing entry", deep_valid_source,
              rejection(19, 0, space=0)))

# Mixed fixed payload shapes test the final field after every earlier field
# has resolved, without changing the language's arbitrary constructor arity.
wide_field_types = b" ".join([b"Int", b"Bytes"] * 32)
wide_field_arguments = b" ".join([b"0", b"(bytes_empty)"] * 31 + [b"0"])
wide_field_prefix = (
    b"(data Wide (Empty) (Wide " + wide_field_types + b"))\n"
    b"(def probe () Wide (Wide " + wide_field_arguments + b" "
)
cases.append(("wide mixed constructor last field mismatch",
              wide_field_prefix + b"0))\n", rejection(15, len(wide_field_prefix))))

cases.extend(depth_fixtures(rejection))
name_rejections, name_accepted = name_fixtures(rejection)
cases.extend(name_rejections)
role_rejections, role_accepted = name_roles(rejection)
cases.extend(role_rejections)
cursor_rejections, cursor_accepted = census_cursors(rejection)
cases.extend(cursor_rejections)
parameter_rejections, parameter_accepted = parameter_cursors(rejection)
cases.extend(parameter_rejections)
replacement_rejections, replacement_accepted = catalog_replacements(rejection)
cases.extend(replacement_rejections)

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
    ("nominal parameters and result retain resolved types",
     b"(data Item (Item Bytes))\n"
     b"(def wrap ((source Bytes)) Item (Item source))\n"
     b"(def unwrap ((item Item)) Bytes (match item ((Item value) value)))\n"
     b"(def main ((source Bytes)) Bytes (unwrap (wrap source)))\n"),
    ("parameter spelling prefixes remain distinct",
     b"(def helper ((value Bytes) (value_more Bytes)) Bytes value_more)\n"
     b"(def main ((source Bytes)) Bytes (helper (bytes_empty) source))\n"),
    ("parameter spelling reuse across functions",
     b"(def first ((value Bytes)) Bytes value)\n"
     b"(def second ((value Bytes)) Bytes (first value))\n"
     b"(def main ((source Bytes)) Bytes (second source))\n"),
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

# Scope restoration and field-type order remain observable on successful code.
accepted += (
    ("outer let binder remains absent throughout initializer",
     b"(def main ((source Bytes)) Bytes "
     b"(let value Bytes (let value Bytes source value) value))\n"),
    ("disjoint let siblings restore outer scope",
     b"(def main ((source Bytes)) Bytes\n  (bytes_concat (let value Bytes source value)\n    (let value Bytes (bytes_empty) value)))\n"),
    ("disjoint if branches reuse local spelling",
     b"(def main ((source Bytes)) Bytes\n  (if 1 (let value Bytes source value)\n    (let value Bytes (bytes_empty) value)))\n"),
    ("disjoint match arms reuse payload spelling",
     b"(data Choice (First Bytes) (Second Bytes))\n(def unwrap ((choice Choice)) Bytes\n  (match choice ((First value) value) ((Second value) value)))\n(def main ((source Bytes)) Bytes (unwrap (First source)))\n"),
    ("nested matches retain independent coverage",
     b"(data Choice (First) (Second))\n(def main ((source Bytes)) Bytes\n  (match First\n    (First (match Second (First (bytes_empty)) (Second source)))\n    (Second (bytes_empty))))\n"),
    ("pattern locals restore across sibling matches",
     b"(data Item (Item Bytes))\n(def main ((source Bytes)) Bytes\n  (bytes_concat\n    (match (Item source) ((Item value) value))\n    (match (Item (bytes_empty)) ((Item value) value))))\n"),
    ("mixed nominal constructor field order and separate owners",
     b"(data Leaf (Leaf Int))\n(data Packet (Empty) (Packet Int Bytes Leaf) (Other Leaf Bytes Int))\n(def extract ((packet Packet)) Bytes\n  (match packet\n    (Empty (bytes_empty))\n    ((Packet number bytes leaf)\n      (match leaf ((Leaf count) (if (eq (+ number count) 12) bytes (bytes_empty)))))\n    ((Other leaf bytes number)\n      (match leaf ((Leaf count) (if (eq (+ number count) 12) bytes (bytes_empty)))))))\n(def main ((source Bytes)) Bytes\n  (bytes_concat (extract (Packet 5 source (Leaf 7)))\n    (bytes_concat (extract (Other (Leaf 7) (bytes_empty) 5)) (extract Empty))))\n"),
)
wide_field_binders = b" ".join(
    f"value{index}".encode("ascii") for index in range(64)
)
wide_value_arguments = b" ".join([b"0", b"source"] * 32)
accepted += (("mixed wide payload and nullary case",
              b"(data Wide (Empty) (Wide " + wide_field_types + b"))\n"
              b"(def unwrap ((value Wide)) Bytes (match value (Empty (bytes_empty)) "
              b"((Wide " + wide_field_binders + b") value63)))\n"
              b"(def main ((source Bytes)) Bytes "
              b"(bytes_concat (unwrap (Wide " + wide_value_arguments + b")) "
              b"(unwrap Empty)))\n"),)
accepted += name_accepted
accepted += role_accepted
accepted += cursor_accepted
accepted += parameter_accepted
accepted += replacement_accepted
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
