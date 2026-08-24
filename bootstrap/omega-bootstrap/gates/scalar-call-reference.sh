#!/usr/bin/env sh
# Differential-only product reference for the first general scalar Call tranche.
# The product-owned test constructs, verifies, interprets, and lowers the module;
# bootstrap authority must still come from the later Delta/lower-rung seam.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] \
    || { echo "scalar call reference: repository root not found" >&2; exit 2; }
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
cd "$OMEGA_REPO_ROOT"

command -v cargo >/dev/null 2>&1 \
  || { echo "scalar call reference: skipped (cargo absent)"; exit 0; }
command -v python3 >/dev/null 2>&1 \
  || { echo "scalar call reference: python3 required" >&2; exit 2; }

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP/gates/fixtures/omega-bootstrap-scalar-call-v28.hex"

export_reference() {
  OUTPUT=$1
  OMEGA_BOOTSTRAP_SCALAR_CALL_TERMINAL="$OUTPUT" cargo test -q \
    -p omega-native-differential-test --test terminal_psi_calls \
    scalar_i32_call_has_exact_exportable_vocabulary_28_bytes -- --exact
  [ -s "$OUTPUT" ] \
    || { echo "scalar call reference FAIL — exporter published no bytes" >&2; exit 1; }
}

export_reference "$T/one.terminal"
export_reference "$T/two.terminal"
cmp "$T/one.terminal" "$T/two.terminal" >/dev/null \
  || { echo "scalar call reference FAIL — repeated export changed bytes" >&2; exit 1; }

python3 - "$FIXTURE" "$T/one.terminal" <<'PY'
import pathlib
import struct
import sys

fixture = bytes.fromhex(pathlib.Path(sys.argv[1]).read_text(encoding="ascii"))
exported = pathlib.Path(sys.argv[2]).read_bytes()
if fixture != exported:
    raise SystemExit("scalar call reference FAIL — committed fixture differs from exporter")
if len(exported) < 12 or exported[:8] != b"PSITERM\0":
    raise SystemExit("scalar call reference FAIL — malformed terminal envelope")
if struct.unpack_from("<HH", exported, 8) != (26, 28):
    raise SystemExit("scalar call reference FAIL — not codec 26 / vocabulary 28")
PY

echo "scalar call reference: exact deterministic vocabulary-28 i32 Call fixture, meaning, lowering, and mutation teeth passed"
