#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Forth-Gamma experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
SYMBOLIC="$GATE_DIR/forth_gamma_evaluator.sbeta"
COMPILER="$GATE_DIR/delta_compiler.fgamma"
RESOLVER="$OMEGA_REPO_ROOT/tests/gamma/evaluator-development/resolve.py"
LEGACY_EVALUATOR="$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE"
LEGACY_COMPILER="$OMEGA_REPO_ROOT/source/delta/bootstrap/concatenative-compiler/delta_compiler.gamma"

materialize_beta_compiler "$TMP/beta" >/dev/null
python3 "$GATE_DIR/import_legacy.py" "$LEGACY_EVALUATOR" "$TMP/legacy.sbeta"
python3 "$RESOLVER" "$TMP/legacy.sbeta" "$TMP/legacy.beta"
"$TMP/beta" < "$TMP/legacy.beta" > "$TMP/imported-legacy.tape"
"$TMP/beta" < "$LEGACY_EVALUATOR" > "$TMP/legacy.tape"
cmp "$TMP/imported-legacy.tape" "$TMP/legacy.tape"

python3 "$GATE_DIR/rewrite_values.py" "$LEGACY_COMPILER" "$TMP/compiler.fgamma" >/dev/null
cmp "$TMP/compiler.fgamma" "$COMPILER"
python3 "$RESOLVER" "$SYMBOLIC" "$TMP/evaluator.beta"
"$TMP/beta" < "$TMP/evaluator.beta" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null

METRICS=$(python3 "$GATE_DIR/measure.py" \
    "$SYMBOLIC" "$COMPILER" \
    "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" \
    "$OMEGA_REPO_ROOT/source/delta/compiler/delta_compiler.gamma")
EXPECTED_METRICS='forth_beta=890,723,122,312,203,87
forth_compiler_lines=1451
forth_compiler_definitions=555
forth_compiler_values=49
forth_compiler_tokens=5860
forth_compiler_branches=204
forth_compiler_jumps=171
forth_compiler_stack_ops=77
forth_compiler_cell_ops=20
functional_beta=1325,1065,165,479,208,203
functional_compiler_lines=852
functional_compiler_definitions=80
functional_compiler_lets=314'
[ "$METRICS" = "$EXPECTED_METRICS" ] || {
    printf '%s\n' "$METRICS"
    echo "Forth-Gamma audit metrics changed" >&2
    exit 1
}

EVALUATOR="$TMP/evaluator" BETA="$TMP/evaluator.beta" \
    TAPE="$TMP/evaluator.tape" SYMBOLIC="$SYMBOLIC" COMPILER="$COMPILER" \
    GATE_DIR="$GATE_DIR" python3 - <<'PY'
import hashlib
import os
import signal
import struct
import subprocess
from pathlib import Path


def invoke(program, sealed_input=b"", timeout=180):
    request = struct.pack("<I", len(program)) + program + sealed_input
    process = subprocess.Popen(
        [os.environ["EVALUATOR"]], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(request, timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        return 124, b""
    return process.returncode, output


artifacts = (
    ("symbolic", Path(os.environ["SYMBOLIC"]).read_bytes(), 20880,
     "ccca6d3d8e316ec9aa2d271e373452307a8a9527fdd30ee6d823bcf99f13704b"),
    ("resolved Beta", Path(os.environ["BETA"]).read_bytes(), 25347,
     "203296f0fbfd08efca2e7714454d37505e1656323cd099ed84e41be309cacf6c"),
    ("tape", Path(os.environ["TAPE"]).read_bytes(), 5145,
     "200f81eb423cf7107daf76ba1db65e7403b4249127b9a3122fa31c1461da4feb"),
    ("compiler", Path(os.environ["COMPILER"]).read_bytes(), 55623,
     "59d3a2debd6778a8f40b0d7683eab803ad1822505ed6f76c09b45e72160a2311"),
)
for name, data, size, digest in artifacts:
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

primitive_cases = {
    "value": (b'value x\n: main 0x41 to x x output-byte ;', 0, b"A"),
    "zero value": (b'value x\n: main x output-byte ;', 0, b"\x00"),
    "duplicate value": (b'value x\nvalue x\n: main ;', 1, b""),
    "builtin value": (b'value dup\n: main ;', 1, b""),
    "assign word": (b': x ;\n: main 0x1 to x ;', 2, b""),
    "plain text": (b': main text "hello world" ;', 0, b"hello world"),
    "syntax text": (b': main text ": x ; # not comment" ;', 0, b": x ; # not comment"),
    "newline text": (b': main text "A\\nB" ;', 0, b"A\nB"),
    "bad text escape": (b': main text "\\t" ;', 2, b""),
    "unclosed text": (b': main text "oops ;', 1, b""),
}
for name, (source, status, output) in primitive_cases.items():
    if invoke(source) != (status, output):
        raise SystemExit(f"Forth-Gamma primitive failed: {name}")

compiler = Path(os.environ["COMPILER"]).read_bytes()
fixture_root = Path(os.environ["GATE_DIR"]).parent / "staged-compiler"
rope = (fixture_root / "bytes_rope.delta").read_bytes()
rope = rope.replace(b"bytes_", b"rope_").replace(b"(read (input))", b"(/ 1 0)")
fixtures = {
    "nat": (
        (fixture_root / "recursive_match.delta").read_bytes(), b"\x03", 2868,
        "2905cd61a0e484c6f1c85f78a3f71489967779d00bed3f3f35cdd77c0b88102c",
    ),
    "list": (
        (fixture_root / "list_match.delta").read_bytes(), b"\x09", 3367,
        "2b6ee0525bf22d5bafc90b4090fd7b8c14881d42530c7bab9cb1108ea33c2c48",
    ),
    "rope": (
        rope, b"B", 6112,
        "0cf7af3df731563d5d2c51fa6b02b8565ec98f3eee60a90d3e55630cd435cd74",
    ),
}
for name, (source, expected, size, digest) in fixtures.items():
    status, receipt = invoke(compiler, source)
    if status != 0 or len(receipt) != size or hashlib.sha256(receipt).hexdigest() != digest:
        raise SystemExit(f"Forth-Gamma receipt changed: {name}")
    if invoke(receipt) != (0, expected):
        raise SystemExit(f"Forth-Gamma result changed: {name}")

malformed = {
    "unknown field": b"(data Bad (Bad Missing))\n(def main () Int 0)\n",
    "missing argument": b"(data Option (None) (Some Int))\n(def main () Int (Some))\n",
    "missing binder": b"(data Option (None) (Some Int))\n(def main () Int (match None (None 0) (Some 1)))\n",
    "nonexhaustive": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Left 7)))\n",
    "out of order": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Right 9) (Left 7)))\n",
}
for name, source in malformed.items():
    if invoke(compiler, source) != (2, b""):
        raise SystemExit(f"Forth-Gamma malformed source accepted: {name}")

stress = (
    b"".join(f"(def f{index} () Int {index % 200})\n".encode() for index in range(300))
    + b"(def main () Int (f299))\n"
)
status, stress_receipt = invoke(compiler, stress)
if status != 0 or len(stress_receipt) != 65579:
    raise SystemExit("Forth-Gamma 301-function transformation failed")
if hashlib.sha256(stress_receipt).hexdigest() != "eb6e8c30dc9fcdc9ec7bed2ceea3f71a37f5b31a8a0254fa9b2e08e0351b87ff":
    raise SystemExit("Forth-Gamma 301-function receipt changed")
if invoke(stress_receipt) != (0, b"\x63"):
    raise SystemExit("Forth-Gamma 301-function result changed")

tail = b"""(data List (Nil) (Cons Int List))
(def make ((n Int) (items List)) List
  (if (eq n 0) items (make (- n 1) (Cons 1 items))))
(def walk ((items List)) Int
  (match items (Nil 0) ((Cons head tail) (walk tail))))
(def main () Int (walk (make 100000 Nil)))
"""
status, tail_receipt = invoke(compiler, tail)
if status != 0 or len(tail_receipt) != 3210:
    raise SystemExit("Forth-Gamma tail receipt changed")
if hashlib.sha256(tail_receipt).hexdigest() != "4c8085674ca0cdb2ad964534531884f4d8486b1b4aa3305a75121f30f574d3a4":
    raise SystemExit("Forth-Gamma tail receipt identity changed")
if invoke(tail_receipt) != (0, b"\x00"):
    raise SystemExit("Forth-Gamma proper-tail traversal failed")

empty_rope = rope.replace(
    b"(rope_get (rope_concat (rope_single 65) (rope_single 66)) 1)",
    b"(rope_get (rope_empty) 0)",
)
status, empty_receipt = invoke(compiler, empty_rope)
if status != 0 or invoke(empty_receipt) != (2, b""):
    raise SystemExit("Forth-Gamma empty-rope trap failed")

# Whole-program name and stack-effect validation remain intentionally absent.
if invoke(b": bad absent ; : main ;") != (0, b""):
    raise SystemExit("expected unreachable-name validation gap changed")
PY

echo "Forth-Gamma experiment: 890-line / 5,145-byte interpreter and 1,451-line Delta compiler passed matched semantics; 3,001 functions exceed 600s"
