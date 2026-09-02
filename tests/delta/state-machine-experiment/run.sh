#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta state-machine experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
COMPILER="$OMEGA_PATH_DELTA/compiler/experiments/state_machine/delta_compiler.gamma"
SAMPLE="$GATE_DIR/sample.delta"
PARSER="$GATE_DIR/nested_parser.delta"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null
stamp_seed "$OMEGA_PATH_GAMMA/compiler/gamma_compiler_bytecode.tape" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/gamma-compiler" >/dev/null

COMPILER=$COMPILER SAMPLE=$SAMPLE PARSER=$PARSER EVALUATOR="$TMP/evaluator" \
    NATIVE_GAMMA="$TMP/gamma-compiler" TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

compiler = Path(os.environ["COMPILER"]).read_bytes()
sample_path = Path(os.environ["SAMPLE"])
sample = sample_path.read_bytes()
temporary = Path(os.environ["TMP"])

def interpreted_compile(subject: bytes):
    request = struct.pack("<I", len(compiler)) + compiler + subject
    return subprocess.run(
        [os.environ["EVALUATOR"]], input=request, stdout=subprocess.PIPE
    )

def native_compile(subject: bytes):
    return subprocess.run(
        [str(temporary / "delta-compiler")], input=subject, stdout=subprocess.PIPE
    )

native_delta = subprocess.run(
    [os.environ["NATIVE_GAMMA"]], input=compiler, stdout=subprocess.PIPE
)
if native_delta.returncode != 0:
    raise SystemExit(f"native Gamma compilation exited {native_delta.returncode}")
if len(native_delta.stdout) != 22339:
    raise SystemExit(f"native Delta compiler is {len(native_delta.stdout)} bytes")
if hashlib.sha256(native_delta.stdout).hexdigest() != "ad81791af359259b2039f93b82807e539d0b3e2f2ca5771a3b966b3d3eee46fa":
    raise SystemExit("native Delta compiler identity changed")
(temporary / "delta-compiler.tape").write_bytes(native_delta.stdout)
'

stamp_seed "$TMP/delta-compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/delta-compiler" >/dev/null

COMPILER=$COMPILER SAMPLE=$SAMPLE PARSER=$PARSER EVALUATOR="$TMP/evaluator" \
    NATIVE_DELTA="$TMP/delta-compiler" TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

compiler = Path(os.environ["COMPILER"]).read_bytes()
sample = Path(os.environ["SAMPLE"]).read_text()
parser = Path(os.environ["PARSER"]).read_text()
temporary = Path(os.environ["TMP"])

def interpreted(subject: str):
    data = subject.encode("ascii")
    request = struct.pack("<I", len(compiler)) + compiler + data
    return subprocess.run(
        [os.environ["EVALUATOR"]], input=request, stdout=subprocess.PIPE
    )

def native(subject: str):
    return subprocess.run(
        [os.environ["NATIVE_DELTA"]],
        input=subject.encode("ascii"),
        stdout=subprocess.PIPE,
    )

expected_hash = "0bb738bae6ab3edca36fe27483dcbaec0d1d85f50d2024af6165344eb284a456"
left = interpreted(sample)
right = native(sample)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("interpreted/native successful compilation disagrees")
if len(left.stdout) != 523 or hashlib.sha256(left.stdout).hexdigest() != expected_hash:
    raise SystemExit("representative output identity changed")
(temporary / "sample.tape").write_bytes(left.stdout)

cases = {
    "duplicate-name": sample.replace("local count word", "local one word"),
    "unknown-type": sample.replace("local count word", "local count Missing"),
    "cross-machine-variable": sample.replace(
        "sub result value result", "sub result value one"
    ),
    "field-type-mismatch": sample.replace(
        "field-set pair left count", "field-set pair left mode", 1
    ),
    "array-index-bounds": sample.replace(
        "index-set slots 0 count", "index-set slots 4 count", 1
    ),
    "nonexhaustive-switch": sample.replace(
        "switch mode Mode stop done go loop endswitch",
        "switch mode Mode stop done endswitch",
    ),
    "cross-machine-state": sample.replace(
        "brzero count done loop", "brzero count decrement_entry loop"
    ),
    "unknown-statement": sample.replace("const one 1", "nonsense one 1"),
}
for name, source in cases.items():
    left = interpreted(source)
    right = native(source)
    if left.returncode != 2 or right.returncode != 2:
        raise SystemExit(
            f"{name}: statuses {left.returncode}/{right.returncode}, expected 2/2"
        )
    if left.stdout != right.stdout:
        raise SystemExit(f"{name}: interpreted/native prefixes disagree")

left = interpreted(parser)
right = native(parser)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("nested parser interpreted/native compilation disagrees")
if len(left.stdout) != 1919:
    raise SystemExit(f"nested parser tape is {len(left.stdout)} bytes")
if hashlib.sha256(left.stdout).hexdigest() != "e5fa32384acdf04a5e956500142a92088229ba8f65e88e0596d90606bdaa9343":
    raise SystemExit("nested parser tape identity changed")
(temporary / "parser.tape").write_bytes(left.stdout)
'

stamp_seed "$TMP/sample.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/sample" >/dev/null
"$TMP/sample" < /dev/null > "$TMP/sample.out"
[ ! -s "$TMP/sample.out" ]

stamp_seed "$TMP/parser.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/parser" >/dev/null
PARSER="$TMP/parser" python3 -c '
import os
import subprocess

cases = [
    ("nested", b"{a{b}c}", 0, "03"),
    ("shadowing", b"{a{a}}", 0, "02"),
    ("spaces", b"{ a { b } c }", 0, "03"),
    ("duplicate", b"{aa}", 1, "0102"),
    ("unclosed", b"{a", 1, "0202"),
    ("unmatched-close", b"}", 1, "0200"),
    ("empty", b"", 0, "00"),
    ("name-overflow", b"{abcdefghijklmnopq}", 2, ""),
    ("scope-overflow", b"{{{{{{{{{", 2, ""),
]
for name, data, expected_status, expected_hex in cases:
    result = subprocess.run([os.environ["PARSER"]], input=data, stdout=subprocess.PIPE)
    if result.returncode != expected_status or result.stdout.hex() != expected_hex:
        raise SystemExit(
            f"{name}: {result.returncode}/{result.stdout.hex()}, "
            f"expected {expected_status}/{expected_hex}"
        )
'

echo "Delta state-machine experiment: 636-line Gamma compiler produced identical 22,339-byte native compiler; 523-byte state and 1,919-byte parser customers pass with 8 rejection twins"
