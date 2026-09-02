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
COMPILER="$GATE_DIR/delta_sm_compiler.gamma"
SAMPLE="$GATE_DIR/sample.delta"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null
stamp_seed "$OMEGA_PATH_GAMMA/compiler/gamma_compiler_bytecode.tape" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/gamma-compiler" >/dev/null

COMPILER=$COMPILER SAMPLE=$SAMPLE EVALUATOR="$TMP/evaluator" \
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
if len(native_delta.stdout) != 19872:
    raise SystemExit(f"native Delta compiler is {len(native_delta.stdout)} bytes")
if hashlib.sha256(native_delta.stdout).hexdigest() != "48d7204145c83d32bcd5094083c00d7e737dcb1201395b1abe737691086f6747":
    raise SystemExit("native Delta compiler identity changed")
(temporary / "delta-compiler.tape").write_bytes(native_delta.stdout)
'

stamp_seed "$TMP/delta-compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/delta-compiler" >/dev/null

COMPILER=$COMPILER SAMPLE=$SAMPLE EVALUATOR="$TMP/evaluator" \
    NATIVE_DELTA="$TMP/delta-compiler" TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

compiler = Path(os.environ["COMPILER"]).read_bytes()
sample = Path(os.environ["SAMPLE"]).read_text()
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

expected_hash = "65e8c17e790b75dea9c9f560a86197f24b38f10f196e35a6edeb49da4f749579"
left = interpreted(sample)
right = native(sample)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("interpreted/native successful compilation disagrees")
if len(left.stdout) != 453 or hashlib.sha256(left.stdout).hexdigest() != expected_hash:
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
'

stamp_seed "$TMP/sample.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/sample" >/dev/null
"$TMP/sample" < /dev/null > "$TMP/sample.out"
[ ! -s "$TMP/sample.out" ]

echo "Delta state-machine experiment: 564-line Gamma compiler produced identical 19,872-byte native compiler and 453-byte sample outputs; 8 rejection twins agree"
