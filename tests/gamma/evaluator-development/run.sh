#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Direct Beta Gamma evaluator: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
SYMBOLIC="$GATE_DIR/gamma_evaluator.sbeta"
RESOLVER="$GATE_DIR/resolve.py"
AUGMENTER="$OMEGA_REPO_ROOT/tests/gamma/self-augmentation-experiment/constant_augmenter.gamma"
AUGMENTED="$OMEGA_REPO_ROOT/tests/gamma/self-augmentation-experiment/program.gamma1"
EXPANDED="$OMEGA_REPO_ROOT/tests/gamma/self-augmentation-experiment/program.gamma"
RECURSIVE="$OMEGA_REPO_ROOT/tests/delta/functional-compiler-experiment/scalar_recursive.delta"

python3 "$RESOLVER" "$SYMBOLIC" "$TMP/evaluator.beta"
materialize_beta_compiler "$TMP/beta" >/dev/null
"$TMP/beta" < "$TMP/evaluator.beta" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null

SYMBOLIC="$SYMBOLIC" RESOLVER="$RESOLVER" BETA="$TMP/evaluator.beta" \
    TAPE="$TMP/evaluator.tape" EVALUATOR="$TMP/evaluator" \
    AUGMENTER="$AUGMENTER" AUGMENTED="$AUGMENTED" EXPANDED="$EXPANDED" \
    RECURSIVE="$RECURSIVE" python3 - <<'PY'
import hashlib
import os
import re
import signal
import struct
import subprocess
from pathlib import Path

artifacts = (
    ("SYMBOLIC", 32096, "5536cc82d08aff023ed3092838b203b5a1e06d686e56681ed778b6b0bfd7a184"),
    ("RESOLVER", 2302, "71bca1be08a58ae8596b0f829d48ee43f48d963829ea8a21208197be0598d3c8"),
    ("BETA", 39423, "3cecc17595639fbd1b7ddd7748d7033896596f56feef1d014a31303613a3d134"),
    ("TAPE", 7690, "008ad07e8db094d644c52d205f3a55229a0df04ace3bd170872439e1878cd7a8"),
)
for name, size, digest in artifacts:
    data = Path(os.environ[name]).read_bytes()
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise SystemExit(f"{name} identity changed")

beta_lines = Path(os.environ["BETA"]).read_text().splitlines()
labels = {}
for line in beta_lines:
    match = re.fullmatch(r"(0x[0-9a-f]+): ; ([a-z][a-z0-9_]*):", line)
    if match:
        labels[match.group(1)] = match.group(2)
control = re.compile(r"\b(?:jmp|jz|jnz|jlt|jeq|call)\b[^;]*\b(0x[0-9a-f]+)\b")
for number, line in enumerate(beta_lines, 1):
    match = control.search(line)
    if not match:
        continue
    target = match.group(1)
    expected_comment = f"; -> {labels.get(target, '<missing>')}:"
    if expected_comment not in line:
        raise SystemExit(f"Beta control target lacks label comment on line {number}")

def run(source, sealed_input=b"", timeout=20):
    request = struct.pack("<I", len(source)) + source + sealed_input
    process = subprocess.Popen(
        [os.environ["EVALUATOR"]], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(request, timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("direct Beta evaluator timed out")
    return process.returncode, output

positive = {
    "literal": (b"(def main () Int 42)\n", b"", b"*"),
    "character": (b"(def main () Int 'A')\n", b"", b"A"),
    "character punctuation": (b"(def main () Int (write '('))\n", b"", b"(("),
    "character newline": (b"(def main () Int (write '\\n'))\n", b"", b"\n\n"),
    "character space": (b"(def main () Int (write '\\s'))\n", b"", b"  "),
    "call": (
        b"(def id ((x Int)) Int x)\n(def main () Int (id 7))\n",
        b"", b"\x07",
    ),
    "forward": (
        b"(def main () Int (answer))\n(def answer () Int 9)\n",
        b"", b"\x09",
    ),
    "let-if": (
        b"(def main () Int (let x Int 6 (if (lt x 7) (+ x 5) 0)))\n",
        b"", b"\x0b",
    ),
    "nested-let": (
        b"(def main () Int (let first Int 1 (let second Int 5 (let third Int 9 third))))\n",
        b"", b"\x09",
    ),
    "recursive": (Path(os.environ["RECURSIVE"]).read_bytes(), b"", b"\x0f"),
    "io": (
        b"(def main () Int (write (read (- (input) 1))))\n",
        b"Z", b"ZZ",
    ),
    "pairs": (
        b"(def main () Int (first (second (pair 0 (pair 9 7)))))\n",
        b"", b"\x09",
    ),
}
for name, (source, sealed_input, expected) in positive.items():
    if run(source, sealed_input) != (0, expected):
        raise SystemExit(f"{name} did not evaluate as expected")

negative = {
    "missing main": (b"(def other () Int 0)\n", 1),
    "duplicate function": (
        b"(def main () Int 0)\n(def main () Int 1)\n", 1,
    ),
    "unknown variable": (b"(def main () Int missing)\n", 1),
    "arity mismatch": (
        b"(def id ((x Int)) Int x)\n(def main () Int (id))\n", 1,
    ),
    "read out of bounds": (b"(def main () Int (read 0))\n", 2),
    "unreachable unknown": (
        b"(def bad () Int missing)\n(def main () Int 0)\n", 1,
    ),
    "unreachable arity": (
        b"(def id ((x Int)) Int x)\n(def bad () Int (id))\n(def main () Int 0)\n", 1,
    ),
    "empty character": (b"(def main () Int '')\n", 1),
    "wide character": (b"(def main () Int 'ab')\n", 1),
    "unknown character escape": (b"(def main () Int '\\x')\n", 1),
    "unused return escape": (b"(def main () Int '\\r')\n", 1),
    "unused tab escape": (b"(def main () Int '\\t')\n", 1),
    "unused quote escape": (b"(def main () Int '\\'')\n", 1),
    "unused slash escape": (b"(def main () Int '\\\\')\n", 1),
}
for name, (source, status) in negative.items():
    if run(source) != (status, b""):
        raise SystemExit(f"{name} did not reject quietly with status {status}")

def countdown(depth):
    return (
        f"(def loop ((n Int)) Int "
        f"(if (eq n 0) 0 (loop (- n 1))))\n"
        f"(def main () Int (loop {depth}))\n"
    ).encode("ascii")

if run(countdown(100000), timeout=30) != (0, b"\x00"):
    raise SystemExit("deep proper-tail recursion was not constant-space")

augmenter = Path(os.environ["AUGMENTER"]).read_bytes()
augmented = Path(os.environ["AUGMENTED"]).read_bytes()
expanded = Path(os.environ["EXPANDED"]).read_bytes()
if run(augmenter, augmented) != (0, expanded):
    raise SystemExit("Delta-authored augmentation did not produce exact source")
if run(expanded) != (0, b"*"):
    raise SystemExit("expanded Delta program did not produce 42")
PY

echo "Direct Beta Gamma evaluator: scalar/effect profile and augmentation loop passed"
