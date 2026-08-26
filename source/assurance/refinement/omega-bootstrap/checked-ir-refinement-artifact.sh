#!/usr/bin/env sh
# Lower-rooted CKIR1 relation checking and selected-result reconstruction.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "checked-IR refinement artifact: skipped (fixture producer requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR refinement artifact: skipped ($TOOL absent)"
    exit 0
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
PRODUCER_SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-source-custody-check.alp"
BUNDLER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_bundle.py"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir_refinement_bundle.py"
MUTATION_GENERATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_mutations.py"
RESOURCE_GENERATOR="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/checked_ir_resources.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/source-custody-artifact.omg"
PRODUCT_SOURCE="$OMEGA_REPO_ROOT/source/compiler/omega/psi/source/source.omg"

stamp_beta_compiler "$BC" >/dev/null
sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" \
  > "$T/check.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-artifact.beta" \
  >> "$T/check.beta"
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER_SOURCE" "$T/producer" >/dev/null

python3 "$BUNDLER" pack "main.omg=$FIXTURE" > "$T/fixture.bundle"
python3 "$BUNDLER" pack "source.omg=$PRODUCT_SOURCE" > "$T/library.bundle"
"$T/producer" < "$T/fixture.bundle" > "$T/fixture.ckir"
"$T/producer" < "$T/library.bundle" > "$T/library.ckir"
[ -s "$T/fixture.ckir" ] && [ -s "$T/library.ckir" ] || {
  echo "checked-IR refinement artifact: fixture producer published no CKIR" >&2
  exit 1
}

printf 'ELF deferred to the artifact-template checker\n' > "$T/placeholder.elf"
: > "$T/empty"

pack_entry() {
  python3 "$PACKER" "$T/fixture.bundle" "$1" "$T/placeholder.elf" \
    --result "$2" > "$3"
}

python3 "$PACKER" "$T/library.bundle" "$T/library.ckir" "$T/empty" \
  --library > "$T/library.rfn"
pack_entry "$T/fixture.ckir" 70 "$T/fixture.rfn"
pack_entry "$T/fixture.ckir" 71 "$T/wrong-result.rfn"

observe() {
  expected=$1
  input=$2
  set +e
  "$T/check" < "$input" > "$T/stdout" 2> "$T/stderr"
  actual=$?
  set -e
  [ "$actual" = "$expected" ] || {
    echo "checked-IR refinement artifact: $input returned $actual, expected $expected" >&2
    tail -c 4096 "$T/stderr" >&2 || true
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "checked-IR refinement artifact: $input published stdout" >&2
    exit 1
  }
}

observe 0 "$T/fixture.rfn"
observe 0 "$T/library.rfn"
observe 251 "$T/wrong-result.rfn"

# Two valid controls cover relations not selected by the fixture's ordinary
# result: library Jump/ReturnUnit with a structural edge and value Copy, plus a
# selected self-aliasing place Copy whose scalar leaves must be snapshotted.
python3 - "$RESOURCE_GENERATOR" "$T/structural-control.ckir" <<'PY'
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import sys

spec = spec_from_file_location("checked_ir_resources", sys.argv[1])
module = module_from_spec(spec)
assert spec.loader is not None
sys.modules[spec.name] = module
spec.loader.exec_module(module)
Path(sys.argv[2]).write_bytes(module.build_structural_jump_control().encode())
PY
python3 "$PACKER" "$T/library.bundle" "$T/structural-control.ckir" "$T/empty" \
  --library > "$T/structural-control.rfn"
observe 0 "$T/structural-control.rfn"

python3 - "$T/fixture.ckir" "$T/copy-self-alias.ckir" <<'PY'
from pathlib import Path
import struct
import sys

source = bytearray(Path(sys.argv[1]).read_bytes())
header = struct.unpack_from("<8sHHHH14I", source)
counts = header[7:]
row_sizes = (24, 20, 16, 36, 20, 32, 20, 40)
operation_offset = 72 + sum(count * size for count, size in zip(counts[:7], row_sizes[:7]))
operand_offset = operation_offset + counts[7] * 40
found = 0
for operation in range(counts[7]):
    row = struct.unpack_from("<IIIBBHIIIIII", source, operation_offset + operation * 40)
    if row[3] == 7 and row[10] == 2:
        start = row[8]
        destination = struct.unpack_from("<I", source, operand_offset + start * 4)[0]
        struct.pack_into("<I", source, operand_offset + (start + 1) * 4, destination)
        found += 1
if found != 1:
    raise SystemExit(f"expected one place-source Copy, found {found}")
Path(sys.argv[2]).write_bytes(source)
PY
pack_entry "$T/copy-self-alias.ckir" 71 "$T/copy-self-alias.rfn"
observe 0 "$T/copy-self-alias.rfn"

python3 - "$T/fixture.ckir" "$T" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])

bad_magic = bytearray(source)
bad_magic[0] ^= 1
out.joinpath("bad-magic.ckir").write_bytes(bad_magic)

table_exhausted = bytearray(source)
struct.pack_into("<I", table_exhausted, 24, 8_193)
out.joinpath("table-exhausted.ckir").write_bytes(table_exhausted)

trailing = source + b"\0"
out.joinpath("trailing.ckir").write_bytes(trailing)

header = struct.unpack_from("<8sHHHH14I", source)
counts = header[7:]
row_sizes = (24, 20, 16, 36, 20, 32, 20, 40)
operation_offset = 72 + sum(count * size for count, size in zip(counts[:7], row_sizes[:7]))
bad_opcode = bytearray(source)
bad_opcode[operation_offset + 12] = 10
out.joinpath("bad-opcode.ckir").write_bytes(bad_opcode)
PY

for CASE in bad-magic trailing bad-opcode; do
  pack_entry "$T/$CASE.ckir" 70 "$T/$CASE.rfn"
  observe 251 "$T/$CASE.rfn"
done
pack_entry "$T/table-exhausted.ckir" 70 "$T/table-exhausted.rfn"
observe 252 "$T/table-exhausted.rfn"

# Reuse the artifact tranche's schema-aware mutation inventory, but route every
# row through this lower-rooted checker. The generator supplies bytes and an
# expected status only; it is neither part of the accepted checker nor an
# authority over the verdict.
mkdir "$T/mutations"
python3 "$MUTATION_GENERATOR" "$T/fixture.ckir" "$T/mutations" >/dev/null
TAB=$(printf '\t')
MUTATIONS=0
while IFS="$TAB" read -r NAME EXPECTED_STATUS MUTATION_CLASS REPRESENTATIVE; do
  [ "$NAME" != path ] || continue
  MUTATIONS=$((MUTATIONS + 1))
  pack_entry "$T/mutations/$NAME" 70 "$T/$NAME.rfn"
  observe "$EXPECTED_STATUS" "$T/$NAME.rfn"
done < "$T/mutations/manifest.tsv"
[ "$MUTATIONS" -gt 0 ] || {
  echo "checked-IR refinement artifact: mutation inventory was empty" >&2
  exit 1
}

ELAPSED=$(($(date +%s) - STARTED))
echo "checked-IR refinement artifact: exact CKIR relations, library/root selection, result reconstruction, and $MUTATIONS schema negatives passed below Delta (${ELAPSED}s)"
