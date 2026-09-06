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
BETA="$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE"
TAPE="$OMEGA_PATH_GAMMA_EVALUATOR_TAPE"
AUGMENTER="$OMEGA_REPO_ROOT/tests/gamma/self-augmentation-experiment/constant_augmenter.gamma"
AUGMENTED="$OMEGA_REPO_ROOT/tests/gamma/self-augmentation-experiment/program.gamma1"
EXPANDED="$OMEGA_REPO_ROOT/tests/gamma/self-augmentation-experiment/program.gamma"
RECURSIVE="$OMEGA_REPO_ROOT/tests/delta/functional-compiler-experiment/scalar_recursive.delta"

stamp_seed "$TAPE" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null

PYTHONPATH="$GATE_DIR" BETA="$BETA" TAPE="$TAPE" EVALUATOR="$TMP/evaluator" \
    AUGMENTER="$AUGMENTER" AUGMENTED="$AUGMENTED" EXPANDED="$EXPANDED" \
    RECURSIVE="$RECURSIVE" python3 - <<'PY'
import hashlib
import os
import re
import signal
import struct
import subprocess
from pathlib import Path
from function_lookup import fixtures as function_lookup_fixtures

artifacts = (
    ("BETA", 46482, "6ef6ad5da234e61207bce4d8c262a596f3dfd19b55377121fb60978852408207"),
    ("TAPE", 8355, "591c8417ca82b38d544c2fcf67f85ae6ff3e01002e9d015421339b8dd216df2e"),
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
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, error = process.communicate(request, timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("direct Beta evaluator timed out")
    if error:
        raise SystemExit(f"direct Beta evaluator emitted stderr: {error!r}")
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
    "pair through tail call": (
        b"(def id ((x Int)) Int x)\n"
        b"(def relay ((x Int)) Int (id x))\n"
        b"(def main () Int (first (relay (pair 12 13))))\n",
        b"", b"\x0c",
    ),
}
for name, (source, sealed_input, expected) in positive.items():
    if run(source, sealed_input) != (0, expected):
        raise SystemExit(f"{name} did not evaluate as expected")

application_results = {
    "empty publication": (
        b"(def $application () Int 1)\n(def main () Int (pair 0 1))\n",
        (0, b""),
    ),
    "published nonzero outcome": (
        b"(def $application () Int 1)\n"
        b"(def main () Int (let emitted Int (write 65) (pair 2 1)))\n",
        (2, b"A"),
    ),
    "discarded failure output": (
        b"(def $application () Int 1)\n"
        b"(def main () Int (let emitted Int (write 65) (pair 249 0)))\n",
        (249, b""),
    ),
    "mapped authored trap": (
        b"(def $application () Int 1)\n(def main () Int (/ 1 0))\n",
        (249, b""),
    ),
}
for name, (source, expected) in application_results.items():
    if run(source) != expected:
        raise SystemExit(f"{name} did not publish as expected")

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
    "forged pair literal": (
        b"(def main () Int (let p Int (pair 7 8) (first 33554432)))\n", 2,
    ),
    "forged pair arithmetic": (
        b"(def main () Int (let p Int (pair 7 8) (first (+ 33554432 0))))\n", 2,
    ),
    "forged pair literal at current heap base": (
        b"(def main () Int (let p Int (pair 7 8) (first 268435456)))\n", 2,
    ),
    "forged pair arithmetic at current heap base": (
        b"(def main () Int (let p Int (pair 7 8) (first (+ 268435456 0))))\n", 2,
    ),
    "pair equality": (
        b"(def main () Int (eq (pair 1 2) (pair 1 2)))\n", 2,
    ),
    "pair condition": (
        b"(def main () Int (if (pair 1 2) 3 4))\n", 2,
    ),
    "pair read index": (
        b"(def main () Int (read (pair 1 2)))\n", 2,
    ),
    "pair write value": (
        b"(def main () Int (write (pair 1 2)))\n", 2,
    ),
    "unmarked pair main": (
        b"(def main () Int (pair 1 2))\n", 2,
    ),
    "pair application status": (
        b"(def $application () Int 1)\n"
        b"(def main () Int (pair (pair 1 2) 1))\n", 249,
    ),
    "unassigned application status": (
        b"(def $application () Int 1)\n(def main () Int (pair 255 0))\n", 249,
    ),
    "discarded complete application": (
        b"(def $application () Int 1)\n(def main () Int (pair 0 0))\n", 249,
    ),
    "empty nonzero publication": (
        b"(def $application () Int 1)\n(def main () Int (pair 2 1))\n", 249,
    ),
    "division by zero": (
        b"(def main () Int (/ 1 0))\n", 2,
    ),
    "remainder by zero": (
        b"(def main () Int (% 1 0))\n", 2,
    ),
    "division overflow": (
        b"(def main () Int (/ -9223372036854775808 -1))\n", 2,
    ),
    "remainder overflow": (
        b"(def main () Int (% -9223372036854775808 -1))\n", 2,
    ),
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

def function_census(count):
    return (
        b"".join(
            f"(def f{index} () Int 0)\n".encode("ascii")
            for index in range(count - 1)
        )
        + b"(def main () Int 0)\n"
    )

if run(function_census(4096)) != (0, b"\x00"):
    raise SystemExit("exact function-census capacity did not complete")
if run(function_census(4097)) != (3, b""):
    raise SystemExit("adjacent function-census capacity was not incomplete")

lookup_cases = function_lookup_fixtures()
for name, source, expected in lookup_cases:
    if run(source) != expected:
        raise SystemExit(f"{name} changed function lookup or failure ownership")
print(f"Direct Beta Gamma evaluator: {len(lookup_cases)} exact function-lookup controls passed")

def nested_add(depth):
    return b"(def main () Int " + b"(+ 0 " * depth + b"0" + b")" * depth + b")\n"

if run(nested_add(255)) != (0, b"\x00"):
    raise SystemExit("exact expression-nesting capacity did not complete")
if run(nested_add(256)) != (3, b""):
    raise SystemExit("adjacent expression-nesting capacity was not incomplete")

def ordinary_call_chain(depth):
    definitions = [
        f"(def f{index} () Int (+ 0 (f{index + 1})))\n".encode("ascii")
        for index in range(depth)
    ]
    definitions.append(f"(def f{depth} () Int 0)\n".encode("ascii"))
    definitions.append(b"(def main () Int (f0))\n")
    return b"".join(definitions)

if run(ordinary_call_chain(256)) != (0, b"\x00"):
    raise SystemExit("exact ordinary-call-context capacity did not complete")
if run(ordinary_call_chain(257)) != (3, b""):
    raise SystemExit("adjacent ordinary-call-context capacity was not incomplete")

augmenter = Path(os.environ["AUGMENTER"]).read_bytes()
augmented = Path(os.environ["AUGMENTED"]).read_bytes()
expanded = Path(os.environ["EXPANDED"]).read_bytes()
if run(augmenter, augmented) != (0, expanded):
    raise SystemExit("Delta-authored augmentation did not produce exact source")
if run(expanded) != (0, b"*"):
    raise SystemExit("expanded Delta program did not produce 42")
PY

echo "Direct Beta Gamma evaluator: semantics, exact bounded-depth capacities, and augmentation passed"
