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
ELABORATOR_SOURCE="$OMEGA_PATH_GAMMA_COMPILER_SOURCE"
DIRECT_COMPILER_SOURCE="$GATE_DIR/direct_compiler.gamma"
SURFACE_SOURCE="$GATE_DIR/../fixtures/gamma_to_beta_surface.gamma"
SURFACE_RECEIPT="$GATE_DIR/../fixtures/gamma_to_beta_surface.beta"
DELTA_RECURSIVE="$OMEGA_REPO_ROOT/tests/delta/macro-extension-experiment/generalized_scalar_recursive.gamma"
DELTA_SURFACE="$OMEGA_REPO_ROOT/tests/delta/macro-extension-experiment/scalar_surface.gamma"
DELTA0_SOURCE="$GATE_DIR/../fixtures/delta0_compiler.gamma"
GAMMA_COMPILER_SOURCE="$DIRECT_COMPILER_SOURCE"

materialize_beta_compiler "$TMP/beta-compiler" >/dev/null
"$TMP/beta-compiler" < "$OMEGA_PATH_GAMMA_EVALUATOR_SOURCE" > "$TMP/evaluator.tape"
stamp_seed "$TMP/evaluator.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" \
    "$TMP/evaluator" >/dev/null
materialize_gamma_compiler "$TMP/elaborator" >/dev/null
cp "$OMEGA_PATH_GAMMA_COMPILER_TAPE" "$TMP/elaborator.tape"
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
    (725, 23133, "08eb29ffabc4a3b27d430cd1431f45245ec31b02390ed8c32632bb173e447afc"),
)
if len(elaborator_tape) != 26674:
    raise SystemExit(f"elaborator tape is {len(elaborator_tape)} bytes")
if hashlib.sha256(elaborator_tape).hexdigest() != (
    "4284b2dfe5496c59d42fab8f61a7924668847bf2ea25a790b02efffd012b3db1"
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
        (343, 7244, "b2164dc17068ee2cff864415125e2186337d2d8b927d82c5dc2166f05f0d37e9"),
        (2087, "a70113e09f6deaff8b89680a3656d92fbe8e527abb82cbb175dadf53bd8ca59f"),
    ),
    "delta-recursive": (
        os.environ["DELTA_RECURSIVE"],
        (25, 1267, "32e4b0b520e8b2363dfbd4c86f37238155dd4474284fe72027f8deea47a3b688"),
        (419, 8995, "f03d5f273775cc4449c11bb85628192cfe93cb9257a53e0a8702966aa4412285"),
        (2498, "2915365fb80951fdb5159b7980d9ff44857f32499e0a79a1f56655aa787754ec"),
    ),
    "delta-surface": (
        os.environ["DELTA_SURFACE"],
        (77, 4324, "9af21390cf43bd907e0dc29bc3f27e949d8b203b3df5a408d2ca1d2e70975895"),
        (886, 20076, "3420b7473a0f6ec734a717a5b75c5d564449897c8e8cf32d4784590e4a575b03"),
        (5884, "eaff42fea4d6a4316c43ad65764cbd70b6c6406fcc026ad4a9fe92c330bb11c3"),
    ),
    "delta0": (
        os.environ["DELTA0_SOURCE"],
        (82, 1913, "cef1311f4439da085db429f1334595a6d0bb73fcf856853c26e9fc76f6c6f2fd"),
        (520, 11477, "2034babd497e166ec45505078c30f4410a041673217a43bbf06b1c4486e0c3ed"),
        (3399, "5212d58fe654fdfc25461a4ccdb56214600f96bf66d350b045f3dce792346cf6"),
    ),
    "gamma-compiler": (
        os.environ["GAMMA_COMPILER_SOURCE"],
        (533, 18704, "6012d60377ddd2220a6afec27c8f247ff2d4bea16c44b43a4bd2f3c96c0ae696"),
        (2628, 63396, "1f9938f5bf88fca95e27b146f749e23d9bdc42a71bbb9babc251ecd0a5ce7bc5"),
        (19756, "ada3f6822c9e1123f82adb239deb828b437cdb4f0df5c34a5f66406a4111491e"),
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
    if len(right.stdout) > 0x1000000:
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

capacity_source = b": main " + (b"0x0 drop " * 37408) + b";\n"
if len(capacity_source) != 336681:
    raise SystemExit("capacity witness source identity changed")
direct = compile_gamma(capacity_source)
expanded = native(capacity_source)
if direct.returncode != 0 or len(direct.stdout) != 1048547:
    raise SystemExit(
        f"capacity witness direct result {direct.returncode}/{len(direct.stdout)}"
    )
if expanded.returncode != 0 or len(expanded.stdout) != 2772595:
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

oversized_source = b": main " + (b"0x0 drop " * 37409) + b";\n"
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

echo "Gamma-to-Beta experiment: 5 exact receipts and 2,772,595-byte near-limit Beta expansion passed; adjacent oversized output rejected before publication"
