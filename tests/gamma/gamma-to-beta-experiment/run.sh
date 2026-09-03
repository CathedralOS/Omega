#!/usr/bin/env sh
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$(CDPATH= cd -- "$GATE_DIR/../../.." && pwd -P)
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/beta/artifact_env.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/gamma/artifact_env.sh"

command -v python3 >/dev/null 2>&1 || {
    echo "Gamma-to-Beta experiment: skipped (python3 absent)"
    exit 0
}

TMP=$(mktemp -d)
trap 'rm -rf -- "$TMP"' EXIT HUP INT TERM
ELABORATOR_SOURCE="$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_SOURCE"
DIRECT_COMPILER_SOURCE="$GATE_DIR/direct_compiler.gamma"
SURFACE_SOURCE="$GATE_DIR/../fixtures/gamma_to_beta_surface.gamma"
SURFACE_RECEIPT="$GATE_DIR/../fixtures/gamma_to_beta_surface.beta"
DELTA_RECURSIVE="$OMEGA_REPO_ROOT/tests/delta/compiler-slice/generalized_scalar_recursive.gamma"
DELTA_SURFACE="$OMEGA_REPO_ROOT/tests/delta/compiler-slice/scalar_surface.gamma"
DELTA0_SOURCE="$GATE_DIR/../fixtures/delta0_compiler.gamma"
GAMMA_COMPILER_SOURCE="$DIRECT_COMPILER_SOURCE"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_CONCATENATIVE_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null
materialize_gamma_compiler "$TMP/elaborator" >/dev/null
cp "$OMEGA_PATH_CONCATENATIVE_GAMMA_COMPILER_TAPE" "$TMP/elaborator.tape"
compile_gamma_source_to_tape "$TMP/elaborator" "$TMP/beta-compiler" \
    "$DIRECT_COMPILER_SOURCE" "$TMP/direct-compiler.tape"
stamp_seed "$TMP/direct-compiler.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/direct-compiler" >/dev/null

ELABORATOR_SOURCE=$ELABORATOR_SOURCE ELABORATOR_TAPE="$TMP/elaborator.tape" \
    ELABORATOR="$TMP/elaborator" EVALUATOR="$TMP/evaluator" \
    BETA_COMPILER="$TMP/beta-compiler" GAMMA_COMPILER="$TMP/direct-compiler" \
    SURFACE_SOURCE=$SURFACE_SOURCE SURFACE_RECEIPT=$SURFACE_RECEIPT \
    DELTA_RECURSIVE=$DELTA_RECURSIVE DELTA_SURFACE=$DELTA_SURFACE \
    DELTA0_SOURCE=$DELTA0_SOURCE GAMMA_COMPILER_SOURCE=$GAMMA_COMPILER_SOURCE \
    TMP=$TMP python3 - <<'PY'
import hashlib
import os
import struct
import subprocess
from pathlib import Path


def identity(data: bytes):
    return len(data.splitlines()), len(data), hashlib.sha256(data).hexdigest()


def require_identity(name: str, data: bytes, expected):
    actual = identity(data)
    if actual != expected:
        raise SystemExit(f"{name} identity {actual}, expected {expected}")


elaborator_source = Path(os.environ["ELABORATOR_SOURCE"]).read_bytes()
elaborator_tape = Path(os.environ["ELABORATOR_TAPE"]).read_bytes()
require_identity(
    "elaborator source",
    elaborator_source,
    (725, 23135, "60cf3a75c30cbd8731f8e2b53f58d082dc813ce8ad01031a503de4471b59020e"),
)
if len(elaborator_tape) != 26674:
    raise SystemExit(f"elaborator tape is {len(elaborator_tape)} bytes")
if hashlib.sha256(elaborator_tape).hexdigest() != (
    "96563e9422a4298fa117fad6613765d7d8d90ea81b55e9528ddf6a40d635bab4"
):
    raise SystemExit("elaborator tape identity changed")


def interpreted(subject: bytes):
    request = struct.pack("<I", len(elaborator_source)) + elaborator_source + subject
    return subprocess.run(
        [os.environ["EVALUATOR"]], input=request, stdout=subprocess.PIPE
    )


def native(subject: bytes):
    return subprocess.run(
        [os.environ["ELABORATOR"]], input=subject, stdout=subprocess.PIPE
    )


def compile_beta(source: bytes):
    return subprocess.run(
        [os.environ["BETA_COMPILER"]], input=source, stdout=subprocess.PIPE
    )


def compile_gamma(source: bytes):
    return subprocess.run(
        [os.environ["GAMMA_COMPILER"]], input=source, stdout=subprocess.PIPE
    )


subjects = {
    "surface": (
        os.environ["SURFACE_SOURCE"],
        (23, 702, "4d91b0e26eb941b80a4516f5fa5a063a3c8f905f5bc19f001c22bc910a50b8f4"),
        (343, 7244, "a2483b9e0743a117f2309364514aa5d8f458fb5e0aa03140344bec0afcf34974"),
        (2087, "776dd3e2561446072c99ee88f81fb3ac77b13988c7cb8a3b51b654e08d5ff237"),
    ),
    "delta-recursive": (
        os.environ["DELTA_RECURSIVE"],
        (42, 1977, "79b809b2f90fde47af1d73d461802e07f6b4e6aafe6ff2742b9dd6b3a9875aef"),
        (577, 12718, "eee7af8e67234266fe04b53c3d07908c2bf6bfe382b24c790e0902c379409dc3"),
        (3660, "6ac7cf77a11344e159c9166ed641a64c5b0d72a4b76807c4c64a86c6b5b8cff6"),
    ),
    "delta-surface": (
        os.environ["DELTA_SURFACE"],
        (100, 5232, "30d17b2521d94eec84ff4282486981e81f5ed2a6699ab61b31a9d30743269a74"),
        (1074, 24483, "2186c4e8c7fee774aa76b15790d058a91121792f6ff123327c0ba3ea99f955e3"),
        (7214, "41f55e8cb4999a9ad40d49e23e2d4e39f9368ec3dd875b3b444ecf50548c14da"),
    ),
    "delta0": (
        os.environ["DELTA0_SOURCE"],
        (82, 1913, "cef1311f4439da085db429f1334595a6d0bb73fcf856853c26e9fc76f6c6f2fd"),
        (520, 11477, "3d45fc54b5ac04a4bd66776e80a1768bf3bccb4ded68501bae6cd1a82f29af2b"),
        (3399, "d17a36f5fa1960f173cf4153fd5fdd9e6816e7d556403e2245db08e15a5af54a"),
    ),
    "gamma-compiler": (
        os.environ["GAMMA_COMPILER_SOURCE"],
        (533, 18707, "6929624469a2ed690b501ab92fd6994960800aab7a26f946561fae323b5c5bd3"),
        (2628, 63396, "1793ca2b2db9ac85c218aa4356749b0b208fa5761d4f0dcbbeeb3671130e00a8"),
        (19756, "d25f94d90addf682b7556bed67021c27bf027f69cfc9ca6ae29560c9b93bf528"),
    ),
}

temporary = Path(os.environ["TMP"])
for name, (source_path, source_id, expansion_id, tape_id) in subjects.items():
    source = Path(source_path).read_bytes()
    require_identity(f"{name} source", source, source_id)

    left = interpreted(source)
    right = native(source)
    if left.returncode != 0 or right.returncode != 0:
        raise SystemExit(
            f"{name} elaboration statuses {left.returncode}/{right.returncode}"
        )
    if left.stdout != right.stdout:
        raise SystemExit(f"{name} interpreted/native elaboration disagrees")
    require_identity(f"{name} expansion", right.stdout, expansion_id)
    if len(right.stdout) > 0x4000000:
        raise SystemExit(f"{name} expansion exceeds Beta source capacity")
    if any(line.lstrip().startswith(b"dw ") for line in right.stdout.splitlines()):
        raise SystemExit(f"{name} expansion contains dw")

    assembled = compile_beta(right.stdout)
    direct = compile_gamma(source)
    if assembled.returncode != 0 or direct.returncode != 0:
        raise SystemExit(
            f"{name} compilation statuses {assembled.returncode}/{direct.returncode}"
        )
    if assembled.stdout != direct.stdout:
        raise SystemExit(f"{name} final tape disagrees with direct Gamma compiler")
    expected_size, expected_hash = tape_id
    if len(assembled.stdout) != expected_size:
        raise SystemExit(f"{name} tape is {len(assembled.stdout)} bytes")
    if hashlib.sha256(assembled.stdout).hexdigest() != expected_hash:
        raise SystemExit(f"{name} tape identity changed")
    temporary.joinpath(f"{name}.tape").write_bytes(assembled.stdout)

receipt = Path(os.environ["SURFACE_RECEIPT"]).read_bytes()
surface_expansion = native(Path(os.environ["SURFACE_SOURCE"]).read_bytes()).stdout
if receipt != surface_expansion:
    raise SystemExit("retained surface Beta receipt changed")

malformed = {
    "unknown word": b": ok 0x41 output-byte ; : main ok missing ;\n",
    "missing jump target": b": main jump ;\n",
    "duplicate definition": b": main ; : main ;\n",
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

capacity_source = b": main " + (b"0x0 drop " * 599146) + b";\n"
if len(capacity_source) != 5392323:
    raise SystemExit("capacity witness source identity changed")
direct = compile_gamma(capacity_source)
expanded = native(capacity_source)
if direct.returncode != 0 or len(direct.stdout) != 16777211:
    raise SystemExit(
        f"capacity witness direct result {direct.returncode}/{len(direct.stdout)}"
    )
if expanded.returncode != 0 or len(expanded.stdout) != 44341207:
    raise SystemExit(
        f"capacity witness elaboration result "
        f"{expanded.returncode}/{len(expanded.stdout)}"
    )
assembled = compile_beta(expanded.stdout)
if assembled.returncode != 0 or assembled.stdout != direct.stdout:
    raise SystemExit(
        f"capacity witness assembly result "
        f"{assembled.returncode}/{len(assembled.stdout)}"
    )

oversized_source = b": main " + (b"0x0 drop " * 599147) + b";\n"
direct = compile_gamma(oversized_source)
expanded = native(oversized_source)
if direct.returncode != 2 or direct.stdout:
    raise SystemExit("adjacent direct oversized program was not rejected first")
if expanded.returncode != 2 or expanded.stdout:
    raise SystemExit("adjacent elaborated oversized program was not rejected first")
PY

stamp_seed "$TMP/surface.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/surface" >/dev/null
SURFACE_OUTPUT=$(printf A | "$TMP/surface" | od -An -tx1 | tr -d ' \n')
[ "$SURFACE_OUTPUT" = "41080706050403020101010305414159" ]

stamp_seed "$TMP/delta-recursive.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/delta-recursive" >/dev/null
RECURSIVE_OUTPUT=$("$TMP/delta-recursive" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$RECURSIVE_OUTPUT" = "0f" ]

stamp_seed "$TMP/delta-surface.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/delta-surface" >/dev/null
DELTA_SURFACE_OUTPUT=$("$TMP/delta-surface" < /dev/null | od -An -tx1 | tr -d ' \n')
[ "$DELTA_SURFACE_OUTPUT" = "15" ]

echo "Gamma-to-Beta experiment: 5 exact receipts and 44,341,207-byte near-limit Beta expansion passed; adjacent oversized output rejected before publication"
