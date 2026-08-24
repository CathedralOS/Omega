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

OMEGA_BOOTSTRAP_SCALAR_CALL_TERMINAL="$T/reference.terminal" \
OMEGA_BOOTSTRAP_SCALAR_CALL_X64_IMAGE="$T/reference.x86_64.elf" cargo test -q \
  -p omega-native-differential-test --test terminal_psi_calls \
  scalar_i32_call_has_exact_exportable_vocabulary_28_bytes -- --exact
[ -s "$T/reference.terminal" ] \
  || { echo "scalar call reference FAIL — terminal exporter published no bytes" >&2; exit 1; }
[ -s "$T/reference.x86_64.elf" ] \
  || { echo "scalar call reference FAIL — image exporter published no bytes" >&2; exit 1; }

python3 - "$FIXTURE" "$T/reference.terminal" "$T/reference.x86_64.elf" <<'PY'
import pathlib
import struct
import sys

fixture = bytes.fromhex(pathlib.Path(sys.argv[1]).read_text(encoding="ascii"))
exported = pathlib.Path(sys.argv[2]).read_bytes()
image = pathlib.Path(sys.argv[3]).read_bytes()
if fixture != exported:
    raise SystemExit("scalar call reference FAIL — committed fixture differs from exporter")
if len(exported) < 12 or exported[:8] != b"PSITERM\0":
    raise SystemExit("scalar call reference FAIL — malformed terminal envelope")
if struct.unpack_from("<HH", exported, 8) != (26, 28):
    raise SystemExit("scalar call reference FAIL — not codec 26 / vocabulary 28")
if len(image) != 8192 or image[:4] != b"\x7fELF":
    raise SystemExit("scalar call reference FAIL — malformed Linux x86-64 ELF")
if struct.unpack_from("<H", image, 18)[0] != 62:
    raise SystemExit("scalar call reference FAIL — image is not x86-64")
if struct.unpack_from("<Q", image, 24)[0] != 0x401033:
    raise SystemExit("scalar call reference FAIL — ELF entry does not name the owned shim")
if image[4096 + 51:4096 + 67] != bytes.fromhex(
    "e8c8ffffff89c7b8e70000000f050f0b"
):
    raise SystemExit("scalar call reference FAIL — final entry shim drifted")
PY

if [ "$(uname -sm)" = "Linux x86_64" ]; then
  chmod +x "$T/reference.x86_64.elf"
  set +e
  "$T/reference.x86_64.elf" > "$T/stdout" 2> "$T/stderr"
  scalar_rc=$?
  set -e
  [ "$scalar_rc" -eq 73 ] && [ ! -s "$T/stdout" ] && [ ! -s "$T/stderr" ] \
    || { echo "scalar call reference FAIL — runnable image observation drifted" >&2; exit 1; }
fi

echo "scalar call reference: exact deterministic vocabulary-28 i32 Call fixture, meaning, runnable lowering, and mutation teeth passed"
