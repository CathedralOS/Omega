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
SCALAR_ELABORATOR="$OMEGA_PATH_CONCATENATIVE_DELTA_COMPILER_SOURCE"
SCALAR_RECURSIVE_EXPANSION="$GATE_DIR/generalized_scalar_recursive.gamma"
SCALAR_SURFACE_SOURCE="$GATE_DIR/scalar_surface.delta"
SCALAR_SURFACE_EXPANSION="$GATE_DIR/scalar_surface.gamma"
OPTION_MATCH_SOURCE="$GATE_DIR/option_match.delta"
OPTION_MATCH_EXPANSION="$GATE_DIR/option_match.gamma"
LIST_RECURSIVE_SOURCE="$GATE_DIR/list_recursive.delta"
LIST_RECURSIVE_EXPANSION="$GATE_DIR/list_recursive.gamma"
NESTED_MATCH_SOURCE="$GATE_DIR/nested_match.delta"
NESTED_MATCH_EXPANSION="$GATE_DIR/nested_match.gamma"
BYTES_SOURCE="$GATE_DIR/bytes_operations.delta"
BYTES_EXPANSION="$GATE_DIR/bytes_operations.gamma"
BYTES_RUNTIME="$GATE_DIR/bytes_runtime.gamma"
TAIL_SOURCE="$GATE_DIR/tail_recursive.delta"
TAIL_EXPANSION="$GATE_DIR/tail_recursive.gamma"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
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

if len(elaborator_tape) != 9535:
    raise SystemExit(f"Delta-to-Gamma elaborator is {len(elaborator_tape)} bytes")
if hashlib.sha256(elaborator_tape).hexdigest() != "90da1a3445eb154412917623eb7419c40e2353c81bb48104b8c5f5bd2eb0c585":
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
    "start overflow": source.replace(b"(let start Int 5", b"(let start Int 65536"),
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
if hashlib.sha256(program.stdout).hexdigest() != "55582cc611c5b72e3bad8f25f45b2d1e791a43c2ec0f9f08197437e060547619":
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
renamed = renamed.replace("(let start Int 5", "(let seed Int 1000")
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
    "start overflow": source.replace("(let start Int 5", "(let start Int 65536"),
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
    OPTION_SOURCE=$OPTION_MATCH_SOURCE OPTION_EXPANSION=$OPTION_MATCH_EXPANSION \
    LIST_SOURCE=$LIST_RECURSIVE_SOURCE LIST_EXPANSION=$LIST_RECURSIVE_EXPANSION \
    NESTED_SOURCE=$NESTED_MATCH_SOURCE NESTED_EXPANSION=$NESTED_MATCH_EXPANSION \
    BYTES_SOURCE=$BYTES_SOURCE BYTES_EXPANSION=$BYTES_EXPANSION \
    BYTES_RUNTIME=$BYTES_RUNTIME \
    TAIL_SOURCE=$TAIL_SOURCE TAIL_EXPANSION=$TAIL_EXPANSION \
    SCALAR_ELABORATOR_TAPE="$TMP/scalar-elaborator.tape" TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

elaborator = Path(os.environ["SCALAR_ELABORATOR"]).read_bytes()
elaborator_tape = Path(os.environ["SCALAR_ELABORATOR_TAPE"]).read_bytes()
temporary = Path(os.environ["TMP"])

runtime_block = elaborator.split(b": emit_bytes_runtime\n", 1)[1].split(
    b"\n;\n", 1
)[0]
packed_runtime = bytearray()
for value, width in __import__("re").findall(
    rb"0x([0-9a-f]+) output-(word|byte)", runtime_block
):
    number = int(value, 16)
    packed_runtime.extend(
        number.to_bytes(8, "little") if width == b"word" else bytes([number])
    )
if bytes(packed_runtime) != Path(os.environ["BYTES_RUNTIME"]).read_bytes():
    raise SystemExit("packed Bytes runtime disagrees with readable source")

if len(elaborator.splitlines()) != 1690 or len(elaborator) != 66682:
    raise SystemExit(
        f"general scalar elaborator is {len(elaborator.splitlines())} lines / "
        f"{len(elaborator)} bytes"
    )
if len(elaborator_tape) != 51239:
    raise SystemExit(f"general scalar elaborator tape is {len(elaborator_tape)} bytes")
if hashlib.sha256(elaborator_tape).hexdigest() != "e089a504f6d7f92f5127af07929b131c4c9d6079dedb80bfe306423ee980b2ec":
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
        1977,
        3660,
        "6ac7cf77a11344e159c9166ed641a64c5b0d72a4b76807c4c64a86c6b5b8cff6",
    ),
    "surface": (
        Path(os.environ["SURFACE_SOURCE"]).read_bytes(),
        Path(os.environ["SURFACE_EXPANSION"]).read_bytes(),
        5232,
        7214,
        "41f55e8cb4999a9ad40d49e23e2d4e39f9368ec3dd875b3b444ecf50548c14da",
    ),
    "option": (
        Path(os.environ["OPTION_SOURCE"]).read_bytes(),
        Path(os.environ["OPTION_EXPANSION"]).read_bytes(),
        1614,
        2905,
        "a7874033a6fa39bd1945884ef555a6e62327eec1c5e88f428bdcefedb4254a32",
    ),
    "list": (
        Path(os.environ["LIST_SOURCE"]).read_bytes(),
        Path(os.environ["LIST_EXPANSION"]).read_bytes(),
        3367,
        5332,
        "0f776bde132c6dedf8e22f3072497fb17856444d3956546cc5f661b86f2e4249",
    ),
    "nested": (
        Path(os.environ["NESTED_SOURCE"]).read_bytes(),
        Path(os.environ["NESTED_EXPANSION"]).read_bytes(),
        2603,
        4381,
        "c0bdc77beb19395085198965a355e167bbfe1877b43a0e59aa54abb7d6fb163c",
    ),
    "bytes": (
        Path(os.environ["BYTES_SOURCE"]).read_bytes(),
        Path(os.environ["BYTES_EXPANSION"]).read_bytes(),
        2510,
        5305,
        "3151971cb085ba62c8ff7b78167e2624f569c7b09dd1d97ed8a197495dcfa33f",
    ),
    "tail": (
        Path(os.environ["TAIL_SOURCE"]).read_bytes(),
        Path(os.environ["TAIL_EXPANSION"]).read_bytes(),
        2171,
        3858,
        "bbd36fee1fec5041636b4a168c4d34be50166fe59980b4ca1534c00269bde423",
    ),
}
differential_fixtures = {"recursive", "option", "bytes", "tail"}
for name, (subject, expected, expansion_size, tape_size, tape_hash) in fixtures.items():
    result = native(subject)
    if result.returncode != 0:
        raise SystemExit(f"{name} native elaboration status {result.returncode}")
    if result.stdout != expected:
        raise SystemExit(f"{name} elaboration disagrees with canonical Gamma")
    if name in differential_fixtures:
        seeded = interpreted(subject)
        if seeded.returncode != 0 or seeded.stdout != result.stdout:
            raise SystemExit(f"{name} interpreted/native elaboration disagrees")
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
    "truncated expression": b"(def main () Int (+ 1))\n",
    "constructor arity mismatch": (
        b"(data Option (None) (Some Int))\n"
        b"(def main () Int (match (Some 1) (None 0) ((Some x y) x)))\n"
    ),
    "constructor expression arity mismatch": (
        b"(data Option (None) (Some Int))\n"
        b"(def main () Int (match (Some 1 2) (None 0) ((Some x) x)))\n"
    ),
    "match cross-type constructor arm": (
        b"(data Option (None) (Some Int))\n"
        b"(data Flag (NoneF) (SomeF Int))\n"
        b"(def main () Int (match None (NoneF 0) (None 1) ((Some x) x)))\n"
    ),
    "match duplicate constructor arm": (
        b"(data Option (None) (Some Int))\n"
        b"(def main () Int (match None (None 0) (None 1) ((Some x) x)))\n"
    ),
    "match non-exhaustive": (
        b"(data Option (None) (Some Int))\n"
        b"(def main () Int (match None (None 0)))\n"
    ),
    "match binder scope misuse": (
        b"(data Option (None) (Some Int))\n"
        b"(def main () Int (match None ((Some x) x) (None x)))\n"
    ),
    "let initializer self-reference": b"(def main () Int (let value Int value value))\n",
    "active let shadowing": b"(def main () Int (let value Int 1 (let value Int 2 value)))\n",
    "let declared type mismatch": b"(def main () Int (let value Bytes 1 0))\n",
    "bytes-single argument type": b"(def main () Bytes (bytes_single (bytes_empty)))\n",
    "bytes-length argument type": b"(def main () Int (bytes_length 0))\n",
    "bytes-get index type": b"(def main () Int (bytes_get (bytes_empty) (bytes_empty)))\n",
    "bytes-concat argument type": b"(def main () Bytes (bytes_concat (bytes_empty) 0))\n",
}
for name, subject in malformed.items():
    result = native(subject)
    if result.returncode != 2 or result.stdout:
        raise SystemExit(f"{name} native rejection status {result.returncode}")
    if name in {"missing main", "match non-exhaustive", "bytes-get index type"}:
        seeded = interpreted(subject)
        if seeded.returncode != 2 or seeded.stdout != result.stdout:
            raise SystemExit(f"{name} interpreted/native rejection disagrees")
    if result.stdout:
        raise SystemExit(f"{name} published output before rejection")

runtime_traps = {
    "bytes-single-high": b"(def main () Int (bytes_length (bytes_single 256)))\n",
    "bytes-get-empty": b"(def main () Int (bytes_get (bytes_empty) 0))\n",
    "int-add-positive-overflow": b"(def main () Int (+ 9223372036854775807 1))\n",
    "int-add-negative-overflow": b"(def main () Int (+ -9223372036854775808 -1))\n",
    "int-sub-positive-overflow": b"(def main () Int (- 9223372036854775807 -1))\n",
    "int-sub-negative-overflow": b"(def main () Int (- -9223372036854775808 1))\n",
    "int-mul-positive-overflow": b"(def main () Int (* 9223372036854775807 2))\n",
    "int-mul-min-overflow": b"(def main () Int (* -9223372036854775808 -1))\n",
    "int-div-zero": b"(def main () Int (/ 1 0))\n",
    "int-div-min-overflow": b"(def main () Int (/ -9223372036854775808 -1))\n",
    "int-mod-zero": b"(def main () Int (% 1 0))\n",
    "int-mod-min-overflow": b"(def main () Int (% -9223372036854775808 -1))\n",
}
for name, subject in runtime_traps.items():
    result = native(subject)
    if result.returncode != 0:
        raise SystemExit(f"{name} native compilation exited {result.returncode}")
    program = compile_gamma(result.stdout)
    if program.returncode != 0:
        raise SystemExit(f"{name} Gamma compilation exited {program.returncode}")
    temporary.joinpath(f"{name}.tape").write_bytes(program.stdout)

arity_parameters = " ".join(f"(p{index} Int)" for index in range(14))
arity_arguments = " ".join(str(index + 1) for index in range(14))
arity_source = (
    f"(def select ({arity_parameters}) Int p13)\n"
    f"(def main () Int (select {arity_arguments}))\n"
).encode("ascii")
arity_result = native(arity_source)
if arity_result.returncode != 0:
    raise SystemExit(f"arity-14 native compilation exited {arity_result.returncode}")
arity_program = compile_gamma(arity_result.stdout)
if arity_program.returncode != 0:
    raise SystemExit(f"arity-14 Gamma compilation exited {arity_program.returncode}")
temporary.joinpath("scalar-arity-14.tape").write_bytes(arity_program.stdout)

scope_source = b"(def main () Int (+ (let value Int 1 value) (let value Int 2 value)))\n"
scope_result = native(scope_source)
scope_interpreted = interpreted(scope_source)
if scope_result.returncode != 0 or scope_interpreted.returncode != 0:
    raise SystemExit("sibling let-scope reuse was rejected")
if scope_result.stdout != scope_interpreted.stdout:
    raise SystemExit("sibling let-scope interpreted/native elaboration disagrees")
scope_program = compile_gamma(scope_result.stdout)
if scope_program.returncode != 0:
    raise SystemExit("sibling let-scope Gamma compilation failed")
temporary.joinpath("scalar-sibling-scope.tape").write_bytes(scope_program.stdout)

typed_let_source = (
    b"(def make () Bytes (bytes_single 1))\n"
    b"(def main () Int (let first Bytes (if 1 (make) (bytes_empty)) "
    b"(let second Bytes (bytes_concat first first) (bytes_length second))))\n"
)
typed_let_result = native(typed_let_source)
if typed_let_result.returncode != 0:
    raise SystemExit("typed Bytes let was rejected")
typed_let_program = compile_gamma(typed_let_result.stdout)
if typed_let_program.returncode != 0:
    raise SystemExit("typed Bytes let Gamma compilation failed")
temporary.joinpath("scalar-typed-let.tape").write_bytes(typed_let_program.stdout)

high_match_source = (
    b"(data Option (None) (Some Int))\n"
    + b"".join(
        f"(def filler{index} () Int 0)\n".encode("ascii")
        for index in range(130)
    )
    + b"(def main () Int (match None (None 1) ((Some value) value)))\n"
)
high_match_result = native(high_match_source)
if high_match_result.returncode != 0:
    raise SystemExit("high-row match label compilation was rejected")
if not any(
    len(word) == 9 and word.startswith(b"x")
    for word in high_match_result.stdout.split()
):
    raise SystemExit("high-row match label did not widen")
high_match_program = compile_gamma(high_match_result.stdout)
if high_match_program.returncode != 0:
    raise SystemExit("high-row match Gamma compilation failed")
temporary.joinpath("scalar-high-match.tape").write_bytes(high_match_program.stdout)
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

stamp_seed "$TMP/scalar-option.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-option" >/dev/null
SCALAR_OPTION_OUTPUT=$("$TMP/scalar-option" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_OPTION_OUTPUT" = "09" ]

stamp_seed "$TMP/scalar-list.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-list" >/dev/null
SCALAR_LIST_OUTPUT=$("$TMP/scalar-list" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_LIST_OUTPUT" = "09" ]

stamp_seed "$TMP/scalar-nested.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-nested" >/dev/null
SCALAR_NESTED_OUTPUT=$("$TMP/scalar-nested" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_NESTED_OUTPUT" = "09" ]

stamp_seed "$TMP/scalar-bytes.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-bytes" >/dev/null
SCALAR_BYTES_OUTPUT=$("$TMP/scalar-bytes" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_BYTES_OUTPUT" = "42" ]

stamp_seed "$TMP/scalar-tail.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-tail" >/dev/null
SCALAR_TAIL_OUTPUT=$("$TMP/scalar-tail" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_TAIL_OUTPUT" = "01" ]

stamp_seed "$TMP/scalar-arity-14.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-arity-14" >/dev/null
SCALAR_ARITY_OUTPUT=$("$TMP/scalar-arity-14" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_ARITY_OUTPUT" = "0e" ]

stamp_seed "$TMP/scalar-sibling-scope.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-sibling-scope" >/dev/null
SCALAR_SCOPE_OUTPUT=$("$TMP/scalar-sibling-scope" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_SCOPE_OUTPUT" = "03" ]

stamp_seed "$TMP/scalar-typed-let.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-typed-let" >/dev/null
SCALAR_TYPED_LET_OUTPUT=$("$TMP/scalar-typed-let" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_TYPED_LET_OUTPUT" = "02" ]

stamp_seed "$TMP/scalar-high-match.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-high-match" >/dev/null
SCALAR_HIGH_MATCH_OUTPUT=$("$TMP/scalar-high-match" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$SCALAR_HIGH_MATCH_OUTPUT" = "01" ]

for NAME in \
    bytes-single-high bytes-get-empty \
    int-add-positive-overflow int-add-negative-overflow \
    int-sub-positive-overflow int-sub-negative-overflow \
    int-mul-positive-overflow int-mul-min-overflow \
    int-div-zero int-div-min-overflow int-mod-zero int-mod-min-overflow
do
    stamp_seed "$TMP/$NAME.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
        "$TMP/$NAME" >/dev/null
    set +e
    "$TMP/$NAME" > "$TMP/$NAME.out"
    STATUS=$?
    set -e
    [ "$STATUS" -eq 2 ]
    [ ! -s "$TMP/$NAME.out" ]
done

echo "Downgraded Delta compiler slice: exact scalar/ADT/Bytes/tail receipts and checks passed"