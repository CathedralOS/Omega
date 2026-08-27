#!/usr/bin/env sh
# OMGRFN2 layer-5 exact CKIR relations and CKIR -> limited-ELF refinement.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN2 layer 5: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN2 layer 5: skipped ($TOOL absent)"
    exit 0
  }
done

ENVELOPE="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2-component-envelope.beta"
ARTIFACT="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-artifact.beta"
ELF="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-elf.beta"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn2_bundle.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir.alp"
BACKEND="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-to-elf.alp"
LOWER_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_omglow.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/two_unit_compilation_fixture.py"
for REQUIRED in "$ENVELOPE" "$ARTIFACT" "$ELF" "$PACKER" "$RESOLVER" \
  "$LOWERER" "$BACKEND" "$LOWER_FRAME" "$FIXTURE"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN2 layer 5: missing $REQUIRED" >&2
    exit 1
  }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
STARTED=$(date +%s)
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null

build_checker() { # name include_elf
  NAME=$1
  INCLUDE_ELF=$2
  cp "$ENVELOPE" "$T/$NAME.beta"
  sed '/^proc main()/,$d' "$ARTIFACT" >> "$T/$NAME.beta"
  if [ "$INCLUDE_ELF" -eq 1 ]; then
    sed '/^proc main()/,$d' "$ELF" >> "$T/$NAME.beta"
    printf '%s\n' \
      '' \
      'proc main() {' \
      '    let status = omgrfn2_component_read()' \
      '    state envelope { to done when (status != 0)  to artifact }' \
      '    state artifact { status = ckir_refinement_artifact_check()  to done when (status != 0)  to elf }' \
      '    state elf { status = ckir_refinement_elf_check()  to done }' \
      '    state done { return status }' \
      '}' >> "$T/$NAME.beta"
  else
    printf '%s\n' \
      '' \
      'proc main() {' \
      '    let status = omgrfn2_component_read()' \
      '    state envelope { to done when (status != 0)  to artifact }' \
      '    state artifact { status = ckir_refinement_artifact_check()  to done }' \
      '    state done { return status }' \
      '}' >> "$T/$NAME.beta"
  fi
  PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/$NAME.beta")
  [ "$PROCEDURES" -le 128 ] || {
    echo "OMGRFN2 layer 5: $NAME exceeds 128 procedures ($PROCEDURES)" >&2
    exit 1
  }
  "$BC" < "$T/$NAME.beta" > "$T/$NAME.asm"
  "$ASM" < "$T/$NAME.asm" > "$T/$NAME.tape"
  stamp_seed "$T/$NAME.tape" "$SEED" "$T/$NAME" >/dev/null 2>&1
}

# Separate acceptance executables preserve the CKIR-relation and ELF-relation
# responsibility boundary even though this focused gate exercises both.
build_checker artifact-check 0
build_checker elf-check 1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND" "$T/backend" >/dev/null

python3 "$FIXTURE" build "$T/canonical"
OMGCOMP="$T/canonical/compilation-envelope.bin"
"$T/resolver" < "$OMGCOMP" > "$T/canonical.omgrsw1"
python3 "$LOWER_FRAME" pack "$OMGCOMP" "$T/canonical.omgrsw1" > "$T/canonical.omglow"
"$T/lowerer" < "$T/canonical.omglow" > "$T/canonical.ckir"
"$T/backend" < "$T/canonical.ckir" > "$T/canonical.elf"
for PRODUCT in "$T/canonical.omgrsw1" "$T/canonical.ckir" "$T/canonical.elf"; do
  [ -s "$PRODUCT" ] || { echo "OMGRFN2 layer 5: empty producer output $PRODUCT" >&2; exit 1; }
done

pack_entry() { # ckir elf result output
  python3 "$PACKER" "$OMGCOMP" "$T/canonical.omgrsw1" "$1" "$2" \
    --result "$3" > "$4"
}

observe() { # checker expected input label
  CHECKER=$1
  EXPECTED=$2
  INPUT=$3
  LABEL=$4
  set +e
  "$CHECKER" < "$INPUT" > "$T/stdout" 2> "$T/stderr"
  ACTUAL=$?
  set -e
  [ "$ACTUAL" -eq "$EXPECTED" ] || {
    echo "OMGRFN2 layer 5: $LABEL returned $ACTUAL, expected $EXPECTED" >&2
    sed -n '1,12p' "$T/stderr" >&2
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "OMGRFN2 layer 5: $LABEL published stdout" >&2
    exit 1
  }
}

pack_entry "$T/canonical.ckir" "$T/canonical.elf" 70 "$T/canonical.rfn"
observe "$T/artifact-check" 0 "$T/canonical.rfn" "canonical two-package CKIR"
observe "$T/elf-check" 0 "$T/canonical.rfn" "canonical two-package CKIR/ELF"

# Produce a second valid CKIR and its exact ELF. The source/witness components
# are intentionally unchanged: layer 5 owns only CKIR validity, execution, and
# ELF emission, while layers 3 and 4 own their joins to the source witness.
python3 - "$T/canonical.ckir" "$T/result-71.ckir" <<'PY'
from pathlib import Path
import struct
import sys

contents = bytearray(Path(sys.argv[1]).read_bytes())
header = struct.unpack_from("<8sHHHH14I", contents)
counts = header[7:]
operations = 72 + sum(
    count * size
    for count, size in zip(counts[:7], (24, 20, 16, 36, 20, 32, 20))
)
found = 0
for index in range(counts[7]):
    row = operations + index * 40
    decoded = struct.unpack_from("<IIIBBHIIIIII", contents, row)
    if decoded[3] == 1 and decoded[10] == 70:
        struct.pack_into("<I", contents, row + 32, 71)
        found += 1
if found != 1:
    raise SystemExit(f"expected one constant 70, found {found}")
Path(sys.argv[2]).write_bytes(contents)
PY
"$T/backend" < "$T/result-71.ckir" > "$T/result-71.elf"
pack_entry "$T/result-71.ckir" "$T/result-71.elf" 71 "$T/result-71.rfn"
pack_entry "$T/result-71.ckir" "$T/canonical.elf" 71 "$T/result-71-canonical.rfn"
pack_entry "$T/canonical.ckir" "$T/result-71.elf" 70 "$T/canonical-result-71.rfn"
observe "$T/artifact-check" 0 "$T/result-71.rfn" "second valid CKIR"
observe "$T/elf-check" 0 "$T/result-71.rfn" "second matching CKIR/ELF pair"
observe "$T/artifact-check" 0 "$T/result-71-canonical.rfn" "CKIR-only checker ignores ELF join"
observe "$T/elf-check" 251 "$T/result-71-canonical.rfn" "result-71 CKIR/canonical ELF cross-pair"
observe "$T/elf-check" 251 "$T/canonical-result-71.rfn" "canonical CKIR/result-71 ELF cross-pair"

# Representative schema, relation, and resource mutations exercise the reused
# complete CKIR checker through the new v2 component offset.
python3 - "$T/canonical.ckir" "$T" <<'PY'
from pathlib import Path
import struct
import sys

source = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])

bad_magic = bytearray(source)
bad_magic[0] ^= 1
out.joinpath("bad-magic.ckir").write_bytes(bad_magic)

exhausted = bytearray(source)
struct.pack_into("<I", exhausted, 24, 8193)
out.joinpath("type-count-exhausted.ckir").write_bytes(exhausted)

header = struct.unpack_from("<8sHHHH14I", source)
counts = header[7:]
operations = 72 + sum(
    count * size
    for count, size in zip(counts[:7], (24, 20, 16, 36, 20, 32, 20))
)
bad_opcode = bytearray(source)
bad_opcode[operations + 12] = 10
out.joinpath("bad-opcode.ckir").write_bytes(bad_opcode)
out.joinpath("trailing.ckir").write_bytes(source + b"\0")
PY
for CASE in bad-magic bad-opcode trailing; do
  pack_entry "$T/$CASE.ckir" "$T/canonical.elf" 70 "$T/$CASE.rfn"
  observe "$T/artifact-check" 251 "$T/$CASE.rfn" "CKIR $CASE"
done
pack_entry "$T/type-count-exhausted.ckir" "$T/canonical.elf" 70 \
  "$T/type-count-exhausted.rfn"
observe "$T/artifact-check" 252 "$T/type-count-exhausted.rfn" \
  "CKIR type-count exhaustion"

# Exact ELF-byte controls cover a header field, an emitted instruction, the RX
# padding/extent, and exact component EOF.
python3 - "$T/canonical.elf" "$T" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
sites = {
    "elf-header": 24,
    "elf-instruction": source.find(b"\xb8\xe7\x00\x00\x00") + 1,
    "elf-padding": len(source) - 1,
}
if any(offset < 0 for offset in sites.values()):
    raise SystemExit(f"canonical ELF mutation site absent: {sites}")
for name, offset in sites.items():
    changed = bytearray(source)
    changed[offset] ^= 1
    out.joinpath(name + ".elf").write_bytes(changed)
out.joinpath("elf-truncated.elf").write_bytes(source[:-1])
out.joinpath("elf-trailing.elf").write_bytes(source + b"\0")
PY
for CASE in elf-header elf-instruction elf-padding elf-truncated elf-trailing; do
  pack_entry "$T/canonical.ckir" "$T/$CASE.elf" 70 "$T/$CASE.rfn"
  observe "$T/elf-check" 251 "$T/$CASE.rfn" "$CASE"
done

# The CKIR evaluator owns the full result; the ELF checker additionally binds
# its low byte. A malformed frame exit is rejected before either component is
# read, and a different full result with the same exit byte cannot substitute.
pack_entry "$T/canonical.ckir" "$T/canonical.elf" 71 "$T/wrong-result.rfn"
pack_entry "$T/canonical.ckir" "$T/canonical.elf" 326 "$T/same-exit.rfn"
observe "$T/artifact-check" 251 "$T/wrong-result.rfn" "wrong selected result"
observe "$T/artifact-check" 251 "$T/same-exit.rfn" "same exit, wrong full result"
python3 - "$T/canonical.rfn" "$T/wrong-exit.rfn" "$T/trailing-frame.rfn" \
  "$T/ckir-boundary-drift.rfn" "$T/ckir-frame-exhausted.rfn" <<'PY'
from pathlib import Path
import struct
import sys

canonical = Path(sys.argv[1]).read_bytes()
wrong_exit = bytearray(canonical)
struct.pack_into("<I", wrong_exit, 36, 71)
Path(sys.argv[2]).write_bytes(wrong_exit)
Path(sys.argv[3]).write_bytes(canonical + b"\0")

# Keep total frame length fixed while moving the witness/CKIR boundary by one;
# the checker must read CKIR from the exact declared v2 component offset.
boundary = bytearray(canonical)
witness_length, ckir_length = struct.unpack_from("<II", boundary, 20)
struct.pack_into("<II", boundary, 20, witness_length + 1, ckir_length - 1)
Path(sys.argv[4]).write_bytes(boundary)

exhausted = bytearray(canonical)
struct.pack_into("<I", exhausted, 24, 2_260_041)
Path(sys.argv[5]).write_bytes(exhausted)
PY
observe "$T/artifact-check" 251 "$T/wrong-exit.rfn" "incoherent claimed exit"
observe "$T/elf-check" 251 "$T/trailing-frame.rfn" "v2 exact frame EOF"
observe "$T/artifact-check" 251 "$T/ckir-boundary-drift.rfn" "exact v2 CKIR offset"
observe "$T/artifact-check" 252 "$T/ckir-frame-exhausted.rfn" "v2 CKIR component ceiling"

ELAPSED=$(($(date +%s) - STARTED))
echo "OMGRFN2 layer 5: exact CKIR relations/result and CKIR-to-ELF bytes/exit, matching controls, cross-pairs, and 0/251/252 teeth passed below Delta (${ELAPSED}s)"
