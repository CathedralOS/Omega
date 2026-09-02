#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Delta state-machine experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
COMPILER="$GATE_DIR/compiler.gamma"
SAMPLE="$GATE_DIR/sample.delta"
PARSER="$GATE_DIR/nested_parser.delta"
TRANSFORM="$GATE_DIR/ast_transform.delta"
ENCODER="$GATE_DIR/alpha_encoder.delta"
EPSILON_SLICE="$GATE_DIR/epsilon_parser_slice.delta"
CALL_OVERFLOW="$GATE_DIR/call_overflow.delta"
SCALAR_RECURSIVE="$GATE_DIR/scalar_recursive.delta"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null
materialize_gamma_compiler "$TMP/gamma-compiler" >/dev/null
compile_gamma_source_to_tape "$TMP/gamma-compiler" "$TMP/beta-compiler" \
    "$COMPILER" "$TMP/delta-compiler.tape"

DELTA_COMPILER_TAPE="$TMP/delta-compiler.tape" python3 -c '
import hashlib
import os
from pathlib import Path

tape = Path(os.environ["DELTA_COMPILER_TAPE"]).read_bytes()
if len(tape) != 29105:
    raise SystemExit(f"native Delta compiler is {len(tape)} bytes")
if hashlib.sha256(tape).hexdigest() != "bd63b8f628fac9e1fbe874342a44108336d7940f75ece8bebcfeee6ff8680dd4":
    raise SystemExit("native Delta compiler identity changed")
'

stamp_seed "$TMP/delta-compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/delta-compiler" >/dev/null

COMPILER=$COMPILER SAMPLE=$SAMPLE PARSER=$PARSER TRANSFORM=$TRANSFORM ENCODER=$ENCODER \
    EPSILON_SLICE=$EPSILON_SLICE CALL_OVERFLOW=$CALL_OVERFLOW \
    SCALAR_RECURSIVE=$SCALAR_RECURSIVE EVALUATOR="$TMP/evaluator" \
    NATIVE_DELTA="$TMP/delta-compiler" TMP=$TMP python3 -c '
import hashlib
import os
import struct
import subprocess
from pathlib import Path

compiler = Path(os.environ["COMPILER"]).read_bytes()
sample = Path(os.environ["SAMPLE"]).read_text()
parser = Path(os.environ["PARSER"]).read_text()
transform = Path(os.environ["TRANSFORM"]).read_text()
encoder = Path(os.environ["ENCODER"]).read_text()
epsilon_slice = Path(os.environ["EPSILON_SLICE"]).read_text()
call_overflow = Path(os.environ["CALL_OVERFLOW"]).read_text()
scalar_recursive = Path(os.environ["SCALAR_RECURSIVE"]).read_text()
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

expected_hash = "13c46ed23793ee614abb012a836433f648b8972c02d83660f5915490719402dd"
left = interpreted(sample)
right = native(sample)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit(
        "interpreted/native successful compilation disagrees: "
        f"statuses {left.returncode}/{right.returncode}, "
        f"sizes {len(left.stdout)}/{len(right.stdout)}, "
        f"hashes {hashlib.sha256(left.stdout).hexdigest()}/"
        f"{hashlib.sha256(right.stdout).hexdigest()}"
    )
if len(left.stdout) != 1357 or hashlib.sha256(left.stdout).hexdigest() != expected_hash:
    raise SystemExit(
        f"representative output identity changed: {len(left.stdout)} bytes, "
        f"{hashlib.sha256(left.stdout).hexdigest()}"
    )
(temporary / "sample.tape").write_bytes(left.stdout)

cases = {
    "duplicate-name": sample.replace("local count word", "local one word"),
    "duplicate-owned-field": sample.replace(
        "record Pair left word right word end",
        "record Pair left word left word end",
    ),
    "duplicate-parameter-local": sample.replace(
        "local next word", "local value word", 1
    ),
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
    "multiword-array-element": sample.replace("array Slots word 4", "array Slots Pair 4"),
    "nonexhaustive-switch": sample.replace(
        "switch mode Mode stop done go loop endswitch",
        "switch mode Mode stop done endswitch",
    ),
    "cross-machine-state": sample.replace(
        "brzero count done loop", "brzero count decrement_entry loop"
    ),
    "call-argument-type": sample.replace(
        "call result sum_down 3 next step next_accumulator",
        "call result sum_down 3 mode step next_accumulator",
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
if len(left.stdout) != 2278 or hashlib.sha256(left.stdout).hexdigest() != "d523d1f0b2e812cffda5c2a87f13367aebdad888579ba2c3d91a23420ff725b7":
    raise SystemExit(
        f"nested parser tape identity changed: {len(left.stdout)} bytes, "
        f"{hashlib.sha256(left.stdout).hexdigest()}"
    )
(temporary / "parser.tape").write_bytes(left.stdout)

left = interpreted(transform)
right = native(transform)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("AST transform interpreted/native compilation disagrees")
if len(left.stdout) != 11038 or hashlib.sha256(left.stdout).hexdigest() != "c4e9ec9547f1471012564d12edcead7d318a4d179d8bd07423fb82da7da8299c":
    raise SystemExit(
        f"AST transform tape identity changed: {len(left.stdout)} bytes, "
        f"{hashlib.sha256(left.stdout).hexdigest()}"
    )
(temporary / "transform.tape").write_bytes(left.stdout)

ill_typed_transform = transform.replace(
    "index-set-dyn tags new_node pending_tag",
    "index-set-dyn tags new_node pending_value",
    1,
)
left = interpreted(ill_typed_transform)
right = native(ill_typed_transform)
if left.returncode != 2 or right.returncode != 2 or left.stdout != right.stdout:
    raise SystemExit("nominal array mismatch was not rejected identically")

left = interpreted(encoder)
right = native(encoder)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit(
        "Alpha encoder interpreted/native compilation disagrees: "
        f"statuses {left.returncode}/{right.returncode}, "
        f"sizes {len(left.stdout)}/{len(right.stdout)}, "
        f"hashes {hashlib.sha256(left.stdout).hexdigest()}/"
        f"{hashlib.sha256(right.stdout).hexdigest()}"
    )
if len(left.stdout) != 14505 or hashlib.sha256(left.stdout).hexdigest() != "317c23ebfe65d85c1a0a42201e45c54a30e62f0ed287b18bb99e94990c80e939":
    raise SystemExit(
        f"Alpha encoder tape identity changed: {len(left.stdout)} bytes, "
        f"{hashlib.sha256(left.stdout).hexdigest()}"
    )
(temporary / "encoder.tape").write_bytes(left.stdout)

ill_typed_encoder = encoder.replace(
    "index-field-set items item_count item_kind pending_kind",
    "index-field-set items item_count item_kind a",
    1,
)
left = interpreted(ill_typed_encoder)
right = native(ill_typed_encoder)
if left.returncode != 2 or right.returncode != 2 or left.stdout != right.stdout:
    raise SystemExit("row field mismatch was not rejected identically")

left = interpreted(epsilon_slice)
right = native(epsilon_slice)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("Epsilon parser slice interpreted/native compilation disagrees")
if len(left.stdout) != 1802 or hashlib.sha256(left.stdout).hexdigest() != "3cb92cba02e73ce0776c769531d63a17bbd4afd1d20281282d3c51e1f6bf8702":
    raise SystemExit("Epsilon parser slice tape identity changed")
(temporary / "epsilon-slice.tape").write_bytes(left.stdout)

left = interpreted(call_overflow)
right = native(call_overflow)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("call-overflow interpreted/native compilation disagrees")
if len(left.stdout) != 288 or hashlib.sha256(left.stdout).hexdigest() != "9b9e8be59daf2891da1c30ff91d513e4faf8715e14e83e058807cb799728a3d2":
    raise SystemExit("call-overflow tape identity changed")
(temporary / "call-overflow.tape").write_bytes(left.stdout)

left = interpreted(scalar_recursive)
right = native(scalar_recursive)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("scalar recursion interpreted/native compilation disagrees")
if len(left.stdout) != 771 or hashlib.sha256(left.stdout).hexdigest() != "a9f47331b19c5f17b0a01637e685bf6d24dd575cbea1da83dbc7a60746892f24":
    raise SystemExit("scalar recursion tape identity changed")
(temporary / "scalar-recursive.tape").write_bytes(left.stdout)

parameters = " ".join(f"p{index} word" for index in range(13))
arguments = " ".join("thirteen" for _ in range(13))
max_arity = (
    "machine zero 0 result word\nstate zero_start\n"
    "const result 0\nreturn result\nend\n"
    f"machine select 13 {parameters} result word\n"
    "state select_start\ncopy result p12\nreturn result\nend\n"
    "machine main 1 ignored word result word\nlocal thirteen word\n"
    "local zero_result word\nstate start\nconst thirteen 13\n"
    "call zero_result zero 0\n"
    f"call result select 13 {arguments}\nhalt result\nend\n"
    "entry main start\n"
)
left = interpreted(max_arity)
right = native(max_arity)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("maximum-arity interpreted/native compilation disagrees")
if len(left.stdout) != 896 or hashlib.sha256(left.stdout).hexdigest() != "07af65013a62d3fb80ac39849ca07fcd40171ce97ff1b4e8eaef17782cc83a9f":
    raise SystemExit("maximum-arity tape identity changed")
(temporary / "max-arity.tape").write_bytes(left.stdout)

too_many_parameters = max_arity.replace(
    f"machine select 13 {parameters}",
    f"machine select 14 {parameters} extra word",
)
wrong_call_arity = max_arity.replace(
    f"call result select 13 {arguments}",
    "call result select 12 " + " ".join("thirteen" for _ in range(12)),
)
wide_parameter = call_overflow.replace("value word", "value Huge", 1)
wide_result = call_overflow.replace("result word", "result Huge", 1)
oversized_frame = call_overflow.replace("10000000", "40000000", 1)
for name, source in {
    "arity-above-thirteen": too_many_parameters,
    "call-arity-mismatch": wrong_call_arity,
    "multiword-parameter": wide_parameter,
    "multiword-result": wide_result,
    "oversized-frame": oversized_frame,
}.items():
    left = interpreted(source)
    right = native(source)
    if left.returncode != 2 or right.returncode != 2 or left.stdout != right.stdout:
        raise SystemExit(f"{name} was not rejected identically")
'

stamp_seed "$TMP/sample.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/sample" >/dev/null
SAMPLE="$TMP/sample" python3 -c '
import os
import subprocess

result = subprocess.run([os.environ["SAMPLE"]], stdout=subprocess.PIPE)
if result.returncode != 0 or result.stdout != b"\x07":
    raise SystemExit(f"recursive sample: {result.returncode}/{result.stdout.hex()}")
'

stamp_seed "$TMP/epsilon-slice.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/epsilon-slice" >/dev/null
EPSILON_SLICE="$TMP/epsilon-slice" python3 -c '
import os
import subprocess

for name, data, expected in [
    ("decimal", b"123", b"\x7b"),
    ("invalid digit", b"12x", b"\xff"),
]:
    result = subprocess.run(
        [os.environ["EPSILON_SLICE"]], input=data, stdout=subprocess.PIPE
    )
    if result.returncode != 0 or result.stdout != expected:
        raise SystemExit(f"{name}: {result.returncode}/{result.stdout.hex()}")
'

stamp_seed "$TMP/max-arity.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/max-arity" >/dev/null
MAX_ARITY="$TMP/max-arity" python3 -c '
import os
import subprocess

result = subprocess.run([os.environ["MAX_ARITY"]], stdout=subprocess.PIPE)
if result.returncode != 13 or result.stdout:
    raise SystemExit(f"maximum arity: {result.returncode}/{result.stdout.hex()}")
'

stamp_seed "$TMP/call-overflow.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/call-overflow" >/dev/null
CALL_OVERFLOW="$TMP/call-overflow" python3 -c '
import os
import subprocess

result = subprocess.run([os.environ["CALL_OVERFLOW"]], stdout=subprocess.PIPE)
if result.returncode != 2 or result.stdout:
    raise SystemExit(f"call overflow: {result.returncode}/{result.stdout.hex()}")
'

stamp_seed "$TMP/scalar-recursive.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/scalar-recursive" >/dev/null
set +e
"$TMP/scalar-recursive"
SCALAR_RECURSIVE_STATUS=$?
set -e
[ "$SCALAR_RECURSIVE_STATUS" -eq 15 ]

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

stamp_seed "$TMP/transform.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/transform" >/dev/null
TRANSFORM="$TMP/transform" python3 -c '
import os
import subprocess

cases = [
    ("mixed", b"(+ 1 (- 2) (? 0 (+ 3 x) 1))", 0, "0303010101fe0302010302"),
    ("fully-folded", b"(+ 1 (- 2) (? 1 3 2))", 0, "0101"),
    ("surviving-choice", b"(? x (+ 1 2) (- 3))", 0, "050302010301fd"),
    ("bad-negate-arity", b"(- 1 2)", 1, "0106"),
    ("bad-choice-arity", b"(? 0 1)", 1, "0106"),
    ("unclosed", b"(+ 1", 1, "0104"),
    ("unmatched-close", b")", 1, "0100"),
    ("unknown-token", b"z", 1, "0100"),
    ("multiple-roots", b"1 2", 1, "0103"),
    ("node-overflow", b"(+" + b" 1" * 49 + b")", 2, ""),
    ("depth-overflow", b"(-" * 17, 2, ""),
]
for name, data, expected_status, expected_hex in cases:
    result = subprocess.run([os.environ["TRANSFORM"]], input=data, stdout=subprocess.PIPE)
    if result.returncode != expected_status or result.stdout.hex() != expected_hex:
        raise SystemExit(
            f"{name}: {result.returncode}/{result.stdout.hex()}, "
            f"expected {expected_status}/{expected_hex}"
        )
'

stamp_seed "$TMP/encoder.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/encoder" >/dev/null
ENCODER="$TMP/encoder" python3 -c '
import os
import subprocess

def item(kind, a=0, b=0, c=0, immediate=b"\0" * 8):
    return bytes((kind, a, b, c)) + immediate

def target_item(kind, target, a=0, b=0):
    return item(kind, a, b, immediate=target.to_bytes(4, "little") + b"\0" * 4)

def program(label_count, *items):
    return label_count.to_bytes(4, "little") + b"".join(items) + b"\xff"

items = [target_item(21, 0), item(0, 1), item(1, 2, immediate=bytes.fromhex("8877665544332211"))]
items += [item(opcode, 3, 4) for opcode in range(2, 12)]
items += [target_item(12, 1), target_item(13, 1, 5), target_item(14, 1, 6)]
items += [target_item(15, 1, 7, 8), target_item(16, 1, 9, 10)]
items += [item(17, 11), item(18, 12), target_item(19, 2), item(20), target_item(21, 1), item(0, 0), target_item(21, 2), item(20)]

prefix = bytes((0, 1, 1, 2)) + bytes.fromhex("8877665544332211")
prefix += b"".join(bytes((opcode, 3, 4)) for opcode in range(2, 12))
target_one = 107
target_two = 109
expected = prefix
expected += bytes((12,)) + target_one.to_bytes(8, "little")
expected += bytes((13, 5)) + target_one.to_bytes(8, "little")
expected += bytes((14, 6)) + target_one.to_bytes(8, "little")
expected += bytes((15, 7, 8)) + target_one.to_bytes(8, "little")
expected += bytes((16, 9, 10)) + target_one.to_bytes(8, "little")
expected += bytes((17, 11, 18, 12, 19)) + target_two.to_bytes(8, "little")
expected += bytes((20, 0, 0, 20))

high_labels = [target_item(21, label) for label in range(301)]
high_label_output = bytes((12,)) + bytes(8) + bytes((0, 0))
exact_items = [target_item(21, 0)] + [target_item(16, 0)] * 95324 + [item(20)] * 8
exact_output = (bytes((16, 0, 0)) + bytes(8)) * 95324 + bytes((20,)) * 8
oversized_items = [target_item(21, 0)] + [target_item(16, 0)] * 95325 + [item(20)] * 8

cases = [
    ("all-opcodes", program(3, *items), 0, expected),
    ("high-label", program(301, *high_labels, target_item(12, 300), item(0)), 0, high_label_output),
    ("exact-payload", program(1, *exact_items), 0, exact_output),
    ("oversized-payload", program(1, *oversized_items), 1, b""),
    ("duplicate-label", program(1, target_item(21, 0), target_item(21, 0), item(0)), 1, b""),
    ("missing-label", program(2, target_item(21, 0), item(0)), 1, b""),
    ("extra-label", program(1, target_item(21, 0), target_item(21, 1), item(0)), 1, b""),
    ("undefined-target", program(1, target_item(21, 0), target_item(12, 1)), 1, b""),
    ("unknown-item", program(0, item(22), item(0)), 1, b""),
    ("truncated-item", bytes(4) + bytes((0, 0)), 1, b""),
    ("trailing-byte", program(0, item(0)) + b"x", 1, b""),
    ("empty-payload", program(0), 1, b""),
]
for name, data, expected_status, expected_output in cases:
    result = subprocess.run([os.environ["ENCODER"]], input=data, stdout=subprocess.PIPE)
    if result.returncode != expected_status or result.stdout != expected_output:
        raise SystemExit(
            f"{name}: {result.returncode}/{result.stdout.hex()}, "
            f"expected {expected_status}/{expected_output.hex()}"
        )
'

echo "Delta state-machine experiment: 815-line scoped/call-frame Gamma compiler produced identical 29,105-byte native compiler; recursive Epsilon parser slice, AST, and full-profile Alpha encoder customers pass"
