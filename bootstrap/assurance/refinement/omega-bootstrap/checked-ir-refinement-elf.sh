#!/usr/bin/env sh
# Persisted-Beta exact CKIR1 -> limited x86-64 ELF refinement gate.
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
  *) echo "checked-IR refinement ELF: skipped (fixture producer requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR refinement ELF: skipped ($TOOL absent)"
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
BACKEND_SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-to-elf.alp"
BUNDLER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega_bootstrap_bundle.py"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir_refinement_bundle.py"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/source-custody-artifact.omg"
PRODUCT_SOURCE="$OMEGA_REPO_ROOT/compiler/psi/source/source.omg"

# The acceptance executable contains only persisted Alpha/Beta artifacts plus
# the three Beta sources. Delta and Python below create untrusted test inputs.
stamp_beta_compiler "$BC" >/dev/null
sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" \
  > "$T/check.beta"
sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-artifact.beta" \
  >> "$T/check.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-elf.beta" \
  >> "$T/check.beta"
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER_SOURCE" "$T/producer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND_SOURCE" "$T/backend" >/dev/null

python3 "$BUNDLER" pack "main.omg=$FIXTURE" > "$T/fixture.bundle"
python3 "$BUNDLER" pack "source.omg=$PRODUCT_SOURCE" > "$T/library.bundle"
"$T/producer" < "$T/fixture.bundle" > "$T/fixture.ckir"
"$T/producer" < "$T/library.bundle" > "$T/library.ckir"
"$T/backend" < "$T/fixture.ckir" > "$T/fixture.elf"
: > "$T/empty"

pack_entry() { # ckir elf result output
  python3 "$PACKER" "$T/fixture.bundle" "$1" "$2" --result "$3" > "$4"
}

observe() { # expected input label
  expected=$1
  input=$2
  label=$3
  set +e
  "$T/check" < "$input" > "$T/stdout" 2> "$T/stderr"
  actual=$?
  set -e
  [ "$actual" = "$expected" ] || {
    echo "checked-IR refinement ELF: $label returned $actual, expected $expected" >&2
    tail -c 4096 "$T/stderr" >&2 || true
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "checked-IR refinement ELF: $label published stdout" >&2
    exit 1
  }
}

pack_entry "$T/fixture.ckir" "$T/fixture.elf" 70 "$T/fixture.rfn"
python3 "$PACKER" "$T/library.bundle" "$T/library.ckir" "$T/empty" \
  --library > "$T/library.rfn"
observe 0 "$T/fixture.rfn" canonical-field-branch-control
observe 0 "$T/library.rfn" canonical-library

# A valid CKIR mutation changes the selected execution result and exact image.
# The matching pair must pass; pairing that CKIR with the old still-valid ELF
# must fail, proving that neither artifact validity nor equal exit shape alone
# substitutes for the exact CKIR-to-ELF relation.
python3 - "$T/fixture.ckir" "$T/alias.ckir" <<'PY'
from pathlib import Path
import struct
import sys

contents = bytearray(Path(sys.argv[1]).read_bytes())
header = struct.unpack_from("<8sHHHH14I", contents)
counts = header[7:17]
operations = 72 + sum(
    count * size
    for count, size in zip(counts[:7], (24, 20, 16, 36, 20, 32, 20))
)
operands = operations + counts[7] * 40
for index in range(counts[7]):
    row = operations + index * 40
    if contents[row + 12] == 7 and struct.unpack_from("<I", contents, row + 32)[0] == 2:
        start, count = struct.unpack_from("<II", contents, row + 24)
        assert count == 2
        destination = struct.unpack_from("<I", contents, operands + start * 4)[0]
        struct.pack_into("<I", contents, operands + (start + 1) * 4, destination)
        break
else:
    raise SystemExit("fixture has no structural place-copy control")
Path(sys.argv[2]).write_bytes(contents)
PY
"$T/backend" < "$T/alias.ckir" > "$T/alias.elf"
pack_entry "$T/alias.ckir" "$T/alias.elf" 71 "$T/alias.rfn"
pack_entry "$T/alias.ckir" "$T/fixture.elf" 71 "$T/mismatched-pair.rfn"
observe 0 "$T/alias.rfn" valid-alias-result-71
observe 251 "$T/mismatched-pair.rfn" valid-but-mismatched-pair

# Locate representative canonical field-address and Branch rel32 templates,
# then perturb those exact bytes along with the ELF entry, syscall observation,
# padding/EOF, and coherent claimed-result observations.
python3 - "$T/fixture.elf" "$T" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_bytes()
out = Path(sys.argv[2])
sites = {
    "header-entry": 24,
    "field-offset": source.find(b"\x48\x05") + 2,
    "branch-rel32": source.find(b"\x85\xc0\x0f\x84") + 4,
    "exit-syscall": source.find(b"\xb8\xe7\x00\x00\x00") + 1,
    "rx-padding": len(source) - 1,
}
if any(offset < 0 for offset in sites.values()):
    raise SystemExit(f"canonical template site absent: {sites}")
for name, offset in sites.items():
    changed = bytearray(source)
    changed[offset] ^= 1
    out.joinpath(name + ".elf").write_bytes(changed)
out.joinpath("truncated.elf").write_bytes(source[:-1])
out.joinpath("trailing.elf").write_bytes(source + b"\0")
PY

for CASE in header-entry field-offset branch-rel32 exit-syscall rx-padding truncated trailing; do
  pack_entry "$T/fixture.ckir" "$T/$CASE.elf" 70 "$T/$CASE.rfn"
  observe 251 "$T/$CASE.rfn" "$CASE"
done

pack_entry "$T/fixture.ckir" "$T/fixture.elf" 71 "$T/wrong-exit.rfn"
pack_entry "$T/fixture.ckir" "$T/fixture.elf" 326 "$T/wrong-full-result.rfn"
observe 251 "$T/wrong-exit.rfn" wrong-selected-exit
observe 251 "$T/wrong-full-result.rfn" same-exit-wrong-full-result

ELAPSED=$(($(date +%s) - STARTED))
echo "checked-IR refinement ELF: exact headers, frame/templates, field/branch offsets, padding/EOF, CKIR pairing, and selected result/exit passed below Delta (${ELAPSED}s)"
