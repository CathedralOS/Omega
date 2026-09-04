#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Direct Beta Delta evaluator: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
SYMBOLIC="$GATE_DIR/direct_delta_evaluator.sbeta"
GAMMA_BETA="$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE"
RESOLVER="$OMEGA_REPO_ROOT/tests/gamma/evaluator-development/resolve.py"

python3 "$RESOLVER" "$SYMBOLIC" "$TMP/evaluator.beta"
materialize_beta_compiler "$TMP/beta" >/dev/null
"$TMP/beta" < "$TMP/evaluator.beta" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null

EVALUATOR="$TMP/evaluator" BETA="$TMP/evaluator.beta" \
    TAPE="$TMP/evaluator.tape" SYMBOLIC="$SYMBOLIC" \
    GAMMA_BETA="$GAMMA_BETA" GATE_DIR="$GATE_DIR" python3 - <<'PY'
import hashlib
import os
import re
import signal
import struct
import subprocess
from pathlib import Path


def run(source, timeout=30):
    request = struct.pack("<I", len(source)) + source
    process = subprocess.Popen(
        [os.environ["EVALUATOR"]], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, start_new_session=True,
    )
    try:
        output, _ = process.communicate(request, timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise SystemExit("direct Beta Delta evaluator timed out")
    return process.returncode, output

fixtures = {
    "recursive_match": b"\x03",
    "list_match": b"\x09",
    "bytes_rope": b"B",
}
fixture_root = Path(os.environ["GATE_DIR"]).parent / "staged-compiler"
for stem, expected in fixtures.items():
    source = (fixture_root / f"{stem}.delta").read_bytes()
    if run(source) != (0, expected):
        raise SystemExit(f"direct evaluator disagrees on {stem}")

stress = (
    b"".join(
        f"(def f{index} () Int {index % 200})\n".encode("ascii")
        for index in range(3000)
    )
    + b"(def main () Int (f2999))\n"
)
if run(stress) != (0, b"\xc7"):
    raise SystemExit("direct evaluator failed the 3,001-function witness")

tail_match = b"""(data List (Nil) (Cons Int List))
(def make ((n Int) (items List)) List
  (if (eq n 0) items (make (- n 1) (Cons 1 items))))
(def walk ((items List)) Int
  (match items (Nil 0) ((Cons head tail) (walk tail))))
(def main () Int (walk (make 100000 Nil)))
"""
if run(tail_match) != (0, b"\x00"):
    raise SystemExit("tail-through-match was not constant-space")

malformed = {
    "unknown field type": b"(data Bad (Bad Missing))\n(def main () Int 0)\n",
    "non-exhaustive match": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Left 7)))\n",
    "out-of-order match": b"(data Choice (Left) (Right))\n(def main () Int (match Left (Right 9) (Left 7)))\n",
}
for name, source in malformed.items():
    if run(source) != (1, b""):
        raise SystemExit(f"direct validation did not reject: {name}")

beta = Path(os.environ["BETA"]).read_bytes()
tape = Path(os.environ["TAPE"]).read_bytes()
symbolic = Path(os.environ["SYMBOLIC"]).read_bytes()
identities = (
    (symbolic, 47037, "74787e1cde79878495eb02f16cb6b978c6c1bac196c2f1521babc83e42833e47"),
    (beta, 57904, "7738ccfcc79bcb185d4c91a2184178a09f15a2208a26c8797cb32ea427eca823"),
    (tape, 11004, "b1bab79c80fa5522040c6b8d41814f2a237fdd9a2dc1f998a87edc3dd97365a6"),
)
for artifact, size, digest in identities:
    if len(artifact) != size or hashlib.sha256(artifact).hexdigest() != digest:
        raise SystemExit("direct evaluator artifact identity changed")

def metrics(path):
    lines = Path(path).read_text().splitlines()
    instructions = []
    labels = 0
    for line in lines:
        if re.fullmatch(r"0x[0-9a-f]+: ; [a-z][a-z0-9_]*:", line):
            labels += 1
            continue
        code = line.split(";", 1)[0].strip()
        if re.fullmatch(r"[a-z][a-z0-9_]*:", code):
            labels += 1
        elif code:
            instructions.append(code.split()[0])
    branches = sum(op in {"jmp", "jz", "jnz", "jlt", "jeq"} for op in instructions)
    calls = instructions.count("call")
    control = branches + calls + instructions.count("ret")
    return len(lines), len(instructions), labels, control, branches, calls

if metrics(os.environ["GAMMA_BETA"]) != (1325, 1065, 165, 479, 208, 203):
    raise SystemExit("Gamma comparison metrics changed")
if metrics(os.environ["SYMBOLIC"]) != (2019, 1655, 262, 836, 346, 387):
    raise SystemExit("direct Delta comparison metrics changed")
if len(beta.splitlines()) != 2019 or len(tape) != 11004:
    raise SystemExit("direct evaluator prototype measurement changed")
PY

echo "Direct Beta Delta evaluator: retained recursive-data, scale, and proper-tail cases pass at 2,019 lines / 11,004 bytes; selected lexical/global coverage remains unmatched"
