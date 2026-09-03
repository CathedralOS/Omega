#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/delta/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta macro-extension experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
ELABORATOR="$GATE_DIR/schema_elaborator.gamma"
SOURCE="$GATE_DIR/../functional-compiler-experiment/scalar_recursive.delta"
EXPANSION="$GATE_DIR/scalar_recursive.gamma"
SCALAR_ELABORATOR="$OMEGA_PATH_DELTA_COMPILER_SOURCE"
SCALAR_RECURSIVE_EXPANSION="$GATE_DIR/generalized_scalar_recursive.gamma"
SCALAR_SURFACE_SOURCE="$GATE_DIR/scalar_surface.delta"
SCALAR_SURFACE_EXPANSION="$GATE_DIR/scalar_surface.gamma"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null
materialize_gamma_compiler "$TMP/gamma-compiler" >/dev/null
compile_gamma_source_to_tape "$TMP/gamma-compiler" "$TMP/beta-compiler" \
    "$ELABORATOR" "$TMP/elaborator.tape"

ELABORATOR=$ELABORATOR SOURCE=$SOURCE EXPANSION=$EXPANSION \
    EVALUATOR="$TMP/evaluator" NATIVE_ELABORATOR_TAPE="$TMP/elaborator.tape" \
    GAMMA_COMPILER="$TMP/gamma-compiler" BETA_COMPILER="$TMP/beta-compiler" \
    TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

elaborator = Path(os.environ["ELABORATOR"]).read_bytes()
source = Path(os.environ["SOURCE"]).read_bytes()
expected = Path(os.environ["EXPANSION"]).read_bytes()
elaborator_tape = Path(os.environ["NATIVE_ELABORATOR_TAPE"]).read_bytes()
temporary = Path(os.environ["TMP"])

if len(elaborator_tape) != 9526:
    raise SystemExit(f"Delta-to-Gamma elaborator is {len(elaborator_tape)} bytes")
if hashlib.sha256(elaborator_tape).hexdigest() != "3691240de5454653634fdf5bc0872c910fe5049bd69f134f340d5da3005719cd":
    raise SystemExit("Delta-to-Gamma elaborator identity changed")

def interpreted(subject: bytes):
    request = struct.pack("<I", len(elaborator)) + elaborator + subject
    return subprocess.run(
        [os.environ["EVALUATOR"]], input=request, stdout=subprocess.PIPE
    )

temporary.joinpath("elaborator.tape").write_bytes(elaborator_tape)

def compile_gamma(expansion: bytes):
    lowered = subprocess.run(
        [os.environ["GAMMA_COMPILER"]], input=expansion, stdout=subprocess.PIPE
    )
    if lowered.returncode != 0:
        return lowered
    return subprocess.run(
        [os.environ["BETA_COMPILER"]], input=lowered.stdout,
        stdout=subprocess.PIPE,
    )

native_path = temporary / "elaborator"

left = interpreted(source)
if left.returncode != 0 or left.stdout != expected:
    raise SystemExit("interpreted elaboration disagrees with canonical Gamma")

malformed = {
    "callee mismatch": source.replace(b"(sum_to (- n 1)", b"(other (- n 1)"),
    "parameter mismatch": source.replace(b"(+ acc n)", b"(+ acc wrong)"),
    "main call mismatch": source.replace(b"(sum_to start 0)", b"(sum_to wrong 0)"),
    "start overflow": source.replace(b"(let start 5", b"(let start 65536"),
}
for name, subject in malformed.items():
    rejected = interpreted(subject)
    if rejected.returncode != 2 or rejected.stdout:
        raise SystemExit(f"interpreted {name} was not rejected before output")

program = compile_gamma(left.stdout)
if program.returncode != 0:
    raise SystemExit(f"Gamma compilation exited {program.returncode}")
if len(program.stdout) != 1366:
    raise SystemExit(f"expanded Gamma tape is {len(program.stdout)} bytes")
if hashlib.sha256(program.stdout).hexdigest() != "5b09d2e2a0bea873bbe8f0f0e44bb4a65b1f026c7637da21cb8fbc058d017842":
    raise SystemExit("expanded Gamma tape identity changed")
temporary.joinpath("program.tape").write_bytes(program.stdout)
'

stamp_seed "$TMP/elaborator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/elaborator" >/dev/null

ELABORATOR="$TMP/elaborator" SOURCE=$SOURCE EXPANSION=$EXPANSION \
    GAMMA_COMPILER="$TMP/gamma-compiler" BETA_COMPILER="$TMP/beta-compiler" \
    TMP=$TMP python3 -c '
import os
import subprocess
from pathlib import Path

source = Path(os.environ["SOURCE"]).read_text()
expected = Path(os.environ["EXPANSION"]).read_bytes()
temporary = Path(os.environ["TMP"])

def elaborate(subject: str):
    return subprocess.run(
        [os.environ["ELABORATOR"]], input=subject.encode("ascii"),
        stdout=subprocess.PIPE,
    )

result = elaborate(source)
if result.returncode != 0 or result.stdout != expected:
    raise SystemExit("native elaboration disagrees with canonical Gamma")

renamed = source.replace("sum_to", "fold")
renamed = renamed.replace("((n Int) (acc Int))", "((remaining Int) (total Int))")
renamed = renamed.replace("(eq n 0)", "(eq remaining 0)")
renamed = renamed.replace("    acc\n", "    total\n")
renamed = renamed.replace("(- n 1)", "(- remaining 1)")
renamed = renamed.replace("(+ acc n)", "(+ total remaining)")
renamed = renamed.replace("(let start 5", "(let seed 1000")
renamed = renamed.replace("(fold start 0)", "(fold seed 0)")
renamed_result = elaborate(renamed)
renamed_expected = expected.replace(b"0000000000000005", b"00000000000003e8")
if renamed_result.returncode != 0 or renamed_result.stdout != renamed_expected:
    raise SystemExit("renamed/long elaboration was not canonical")

long_program = subprocess.run(
    [os.environ["GAMMA_COMPILER"]], input=renamed_result.stdout,
    stdout=subprocess.PIPE,
)
if long_program.returncode != 0:
    raise SystemExit("long Gamma expansion did not lower")
long_tape = subprocess.run(
    [os.environ["BETA_COMPILER"]], input=long_program.stdout,
    stdout=subprocess.PIPE,
)
if long_tape.returncode != 0:
    raise SystemExit("long Beta expansion did not assemble")
temporary.joinpath("long-program.tape").write_bytes(long_tape.stdout)

malformed = {
    "callee mismatch": source.replace("(sum_to (- n 1)", "(other (- n 1)"),
    "parameter mismatch": source.replace("(+ acc n)", "(+ acc wrong)"),
    "main call mismatch": source.replace("(sum_to start 0)", "(sum_to wrong 0)"),
    "start overflow": source.replace("(let start 5", "(let start 65536"),
}
for name, subject in malformed.items():
    rejected = elaborate(subject)
    if rejected.returncode != 2 or rejected.stdout:
        raise SystemExit(f"{name} was not rejected before output")
'

stamp_seed "$TMP/program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/program" >/dev/null
OUTPUT=$("$TMP/program" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$OUTPUT" = "0f" ]

stamp_seed "$TMP/long-program.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/long-program" >/dev/null
LONG_OUTPUT=$("$TMP/long-program" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$LONG_OUTPUT" = "14" ]

materialize_delta_compiler "$TMP/scalar-elaborator" \
    "$TMP/gamma-compiler" "$TMP/beta-compiler" >/dev/null
compile_gamma_source_to_tape "$TMP/gamma-compiler" "$TMP/beta-compiler" \
    "$SCALAR_ELABORATOR" "$TMP/scalar-elaborator.tape"

SCALAR_ELABORATOR=$SCALAR_ELABORATOR \
    NATIVE_SCALAR_ELABORATOR="$TMP/scalar-elaborator" \
    EVALUATOR="$TMP/evaluator" GAMMA_COMPILER="$TMP/gamma-compiler" \
    BETA_COMPILER="$TMP/beta-compiler" \
    RECURSIVE_SOURCE=$SOURCE RECURSIVE_EXPANSION=$SCALAR_RECURSIVE_EXPANSION \
    SURFACE_SOURCE=$SCALAR_SURFACE_SOURCE SURFACE_EXPANSION=$SCALAR_SURFACE_EXPANSION \
    SCALAR_ELABORATOR_TAPE="$TMP/scalar-elaborator.tape" TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

elaborator = Path(os.environ["SCALAR_ELABORATOR"]).read_bytes()
elaborator_tape = Path(os.environ["SCALAR_ELABORATOR_TAPE"]).read_bytes()
temporary = Path(os.environ["TMP"])

if len(elaborator.splitlines()) != 550 or len(elaborator) != 21336:
    raise SystemExit(
        f"general scalar elaborator is {len(elaborator.splitlines())} lines / "
        f"{len(elaborator)} bytes"
    )
if len(elaborator_tape) != 19238:
    raise SystemExit(f"general scalar elaborator tape is {len(elaborator_tape)} bytes")
if hashlib.sha256(elaborator_tape).hexdigest() != "7992217795cddcabfec613b32c1a1541e0b529e2bc333d2ad3eeeaabe8bb5b1c":
    raise SystemExit("general scalar elaborator identity changed")

def interpreted(subject: bytes):
    request = struct.pack("<I", len(elaborator)) + elaborator + subject
    return subprocess.run(
        [os.environ["EVALUATOR"]], input=request, stdout=subprocess.PIPE
    )

def native(subject: bytes):
    return subprocess.run(
        [os.environ["NATIVE_SCALAR_ELABORATOR"]],
        input=subject,
        stdout=subprocess.PIPE,
    )

def compile_gamma(source: bytes):
    lowered = subprocess.run(
        [os.environ["GAMMA_COMPILER"]], input=source, stdout=subprocess.PIPE
    )
    if lowered.returncode != 0:
        return lowered
    return subprocess.run(
        [os.environ["BETA_COMPILER"]], input=lowered.stdout,
        stdout=subprocess.PIPE,
    )

fixtures = {
    "recursive": (
        Path(os.environ["RECURSIVE_SOURCE"]).read_bytes(),
        Path(os.environ["RECURSIVE_EXPANSION"]).read_bytes(),
        1267,
        2498,
        "2915365fb80951fdb5159b7980d9ff44857f32499e0a79a1f56655aa787754ec",
    ),
    "surface": (
        Path(os.environ["SURFACE_SOURCE"]).read_bytes(),
        Path(os.environ["SURFACE_EXPANSION"]).read_bytes(),
        4324,
        5884,
        "eaff42fea4d6a4316c43ad65764cbd70b6c6406fcc026ad4a9fe92c330bb11c3",
    ),
}
for name, (subject, expected, expansion_size, tape_size, tape_hash) in fixtures.items():
    left = interpreted(subject)
    right = native(subject)
    if left.returncode != 0 or right.returncode != 0:
        raise SystemExit(
            f"{name} elaboration statuses {left.returncode}/{right.returncode}"
        )
    if left.stdout != expected or right.stdout != expected:
        raise SystemExit(f"{name} elaboration disagrees with canonical Gamma")
    if len(expected) != expansion_size:
        raise SystemExit(f"{name} expansion is {len(expected)} bytes")
    program = compile_gamma(expected)
    if program.returncode != 0:
        raise SystemExit(f"{name} Gamma compilation exited {program.returncode}")
    if len(program.stdout) != tape_size:
        raise SystemExit(f"{name} final tape is {len(program.stdout)} bytes")
    if hashlib.sha256(program.stdout).hexdigest() != tape_hash:
        raise SystemExit(f"{name} final tape identity changed")
    temporary.joinpath(f"scalar-{name}.tape").write_bytes(program.stdout)

negative = b"(def main () Int -1)\n"
left = interpreted(negative)
right = native(negative)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("negative literal elaboration disagrees")
negative_program = compile_gamma(left.stdout)
if negative_program.returncode != 0:
    raise SystemExit("negative literal expansion did not compile")
temporary.joinpath("scalar-negative.tape").write_bytes(negative_program.stdout)

parameters = " ".join(f"(p{index} Int)" for index in range(13))
malformed = {
    "missing main": b"(def other () Int 0)\n",
    "duplicate function": b"(def main () Int 0)\n(def main () Int 1)\n",
    "unknown local": b"(def main () Int missing)\n",
    "unknown callee": b"(def main () Int (missing 1))\n",
    "arity mismatch": b"(def id ((x Int)) Int x)\n(def main () Int (id))\n",
    "non-Int type": b"(def main () Bytes 0)\n",
    "arity above thirteen": (
        f"(def too_many ({parameters} (extra Int)) Int extra)\n"
        "(def main () Int 0)\n"
    ).encode("ascii"),
    "truncated expression": b"(def main () Int (+ 1))\n",
}
for name, subject in malformed.items():
    left = interpreted(subject)
    right = native(subject)
    if left.returncode != 2 or right.returncode != 2:
        raise SystemExit(
            f"{name} rejection statuses {left.returncode}/{right.returncode}"
        )
    if left.stdout or right.stdout:
        raise SystemExit(f"{name} published output before rejection")
'

stamp_seed "$TMP/scalar-recursive.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-recursive" >/dev/null
compile_delta_source_to_tape "$TMP/scalar-elaborator" \
    "$TMP/gamma-compiler" "$TMP/beta-compiler" \
    "$SOURCE" "$TMP/helper-recursive.tape"
cmp "$TMP/scalar-recursive.tape" "$TMP/helper-recursive.tape"
SCALAR_RECURSIVE_OUTPUT=$("$TMP/scalar-recursive" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_RECURSIVE_OUTPUT" = "0f" ]

stamp_seed "$TMP/scalar-surface.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-surface" >/dev/null
SCALAR_SURFACE_OUTPUT=$("$TMP/scalar-surface" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_SURFACE_OUTPUT" = "15" ]

stamp_seed "$TMP/scalar-negative.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-negative" >/dev/null
SCALAR_NEGATIVE_OUTPUT=$("$TMP/scalar-negative" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_NEGATIVE_OUTPUT" = "ff" ]

echo "Delta compiler slice: selected 550-line compiler emitted exact 1,267/4,324-byte Gamma and composed programs returned 15/21/255; schema proof retained"