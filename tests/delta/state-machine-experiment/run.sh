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
TRANSFORM="$GATE_DIR/ast_transform.delta"
ENCODER="$GATE_DIR/alpha_encoder.delta"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/evaluator" >/dev/null
stamp_seed "$OMEGA_PATH_GAMMA/compiler/gamma_compiler_bytecode.tape" \
    "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$TMP/gamma-compiler" >/dev/null

COMPILER=$COMPILER SAMPLE=$SAMPLE PARSER=$PARSER TRANSFORM=$TRANSFORM ENCODER=$ENCODER EVALUATOR="$TMP/evaluator" \
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
if len(native_delta.stdout) != 23403:
    raise SystemExit(f"native Delta compiler is {len(native_delta.stdout)} bytes")
if hashlib.sha256(native_delta.stdout).hexdigest() != "cd8fa38de56f9d019bfc72defbf18118419a07e376405316fc12aa7e54f18a2b":
    raise SystemExit("native Delta compiler identity changed")
(temporary / "delta-compiler.tape").write_bytes(native_delta.stdout)
'

stamp_seed "$TMP/delta-compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/delta-compiler" >/dev/null

COMPILER=$COMPILER SAMPLE=$SAMPLE PARSER=$PARSER TRANSFORM=$TRANSFORM ENCODER=$ENCODER EVALUATOR="$TMP/evaluator" \
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
    "multiword-array-element": sample.replace("array Slots word 4", "array Slots Pair 4"),
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

left = interpreted(transform)
right = native(transform)
if left.returncode != 0 or right.returncode != 0 or left.stdout != right.stdout:
    raise SystemExit("AST transform interpreted/native compilation disagrees")
if len(left.stdout) != 9563:
    raise SystemExit(f"AST transform tape is {len(left.stdout)} bytes")
if hashlib.sha256(left.stdout).hexdigest() != "20ee64218fb14140b1a88bd50191175beab76c4ffaff05e157f43c41f3b3ba27":
    raise SystemExit("AST transform tape identity changed")
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
    raise SystemExit("Alpha encoder interpreted/native compilation disagrees")
if len(left.stdout) != 11772:
    raise SystemExit(f"Alpha encoder tape is {len(left.stdout)} bytes")
if hashlib.sha256(left.stdout).hexdigest() != "c67d25e2a4a9d87062cf0dcf7f4eb6b59afb044fed95f11b69cc2c7af6635142":
    raise SystemExit("Alpha encoder tape identity changed")
(temporary / "encoder.tape").write_bytes(left.stdout)
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

echo "Delta state-machine experiment: 661-line Gamma compiler produced identical 23,403-byte native compiler; recursive AST and full-profile Alpha encoder customers pass"
