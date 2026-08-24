#!/usr/bin/env sh
# Source-derived selected result composed with independent CKIR and ELF checks.
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
  *) echo "checked-IR refinement source result: skipped (fixture producer requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR refinement source result: skipped ($TOOL absent)"
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

stamp_beta_compiler "$BC" >/dev/null

sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" \
  > "$T/artifact.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-artifact.beta" \
  >> "$T/artifact.beta"
"$BC" < "$T/artifact.beta" > "$T/artifact.asm"
"$ASM" < "$T/artifact.asm" > "$T/artifact.tape"
stamp_seed "$T/artifact.tape" "$SEED" "$T/artifact-check" >/dev/null 2>&1

sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" \
  > "$T/elf.beta"
sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-artifact.beta" \
  >> "$T/elf.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-elf.beta" \
  >> "$T/elf.beta"
"$BC" < "$T/elf.beta" > "$T/elf.asm"
"$ASM" < "$T/elf.asm" > "$T/elf.tape"
stamp_seed "$T/elf.tape" "$SEED" "$T/elf-check" >/dev/null 2>&1

sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" \
  > "$T/source.beta"
sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-input.beta" \
  >> "$T/source.beta"

# This executable reconstructs and evaluates source only. The other two
# persisted-Beta conjuncts own the CKIR and ELF joins over the same envelope.
# Omit their unreachable join procedures both to make the nondependence
# mechanical and to stay below Alpha's fixed assembly/code carrier.
python3 - \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-tables.beta" \
  src_compare_types_records src_compare_machines_blocks \
  src_refinement_tables_check main >> "$T/source.beta" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
removed = set(sys.argv[2:])
skip = False
for line in path.read_text(encoding="utf-8").splitlines(keepends=True):
    match = re.match(r"proc ([A-Za-z0-9_]+)\(", line)
    if match:
        skip = match.group(1) in removed
    if not skip:
        sys.stdout.write(line)
PY
python3 - \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-lowering.beta" \
  ckir_u32 ckir_row_word ckir_row_byte ckir_bparam_word ckir_operand \
  src_low_decode_validated_ckir src_lower_compare_final \
  src_refinement_lowering_check main >> "$T/source.beta" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
removed = set(sys.argv[2:])
skip = False
for line in path.read_text(encoding="utf-8").splitlines(keepends=True):
    match = re.match(r"proc ([A-Za-z0-9_]+)\(", line)
    if match:
        skip = match.group(1) in removed
    if not skip:
        sys.stdout.write(line)
PY
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-result.beta" \
  >> "$T/source.beta"
[ "$(grep -c '^proc ' "$T/source.beta")" -le 128 ] || {
  echo "checked-IR refinement source result: source checker exceeds 128 procedures" >&2
  exit 1
}
"$BC" < "$T/source.beta" > "$T/source.asm"
"$ASM" < "$T/source.asm" > "$T/source.tape"
stamp_seed "$T/source.tape" "$SEED" "$T/source-check" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER_SOURCE" "$T/producer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND_SOURCE" "$T/backend" >/dev/null

mkdir "$T/sources"
cp "$FIXTURE" "$T/sources/result70.omg"
cp "$PRODUCT_SOURCE" "$T/sources/product.omg"
python3 - "$FIXTURE" "$T/sources/result69.omg" <<'PY'
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_text(encoding="utf-8")
changed = source.replace("true -> store_3(70)", "true -> store_3(69)", 1)
if changed == source:
    raise SystemExit("reachable-result mutation did not apply")
Path(sys.argv[2]).write_text(changed, encoding="utf-8")
PY

build_entry() { # label result
  label=$1 result=$2
  python3 "$BUNDLER" pack "$label.omg=$T/sources/$label.omg" > "$T/$label.bundle"
  "$T/producer" < "$T/$label.bundle" > "$T/$label.ckir"
  "$T/backend" < "$T/$label.ckir" > "$T/$label.elf"
  python3 "$PACKER" "$T/$label.bundle" "$T/$label.ckir" "$T/$label.elf" \
    --result "$result" > "$T/$label.rfn"
}

run_one() { # checker expected input label
  checker=$1 expected=$2 input=$3 label=$4
  set +e
  "$checker" < "$input" > "$T/stdout" 2> "$T/stderr"
  actual=$?
  set -e
  [ "$actual" = "$expected" ] || {
    echo "checked-IR refinement source result: $label returned $actual, expected $expected" >&2
    tail -c 4096 "$T/stderr" >&2 || true
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "checked-IR refinement source result: $label published stdout" >&2
    exit 1
  }
}

run_all() { # artifact_expected source_expected elf_expected input label
  run_one "$T/artifact-check" "$1" "$4" "$5-artifact"
  run_one "$T/source-check" "$2" "$4" "$5-source"
  run_one "$T/elf-check" "$3" "$4" "$5-elf"
}

build_entry result70 70
build_entry result69 69
run_all 0 0 0 "$T/result70.rfn" result70
run_all 0 0 0 "$T/result69.rfn" result69

: > "$T/empty"
python3 "$BUNDLER" pack "source.omg=$T/sources/product.omg" > "$T/product.bundle"
"$T/producer" < "$T/product.bundle" > "$T/product.ckir"
python3 "$PACKER" "$T/product.bundle" "$T/product.ckir" "$T/empty" \
  --library > "$T/product.rfn"
run_all 0 0 0 "$T/product.rfn" product-library

# Each side is a valid complete compilation. Cross-pair the source with the
# other CKIR/ELF/result so artifact and ELF checks pass while only independent
# source interpretation rejects.
python3 "$PACKER" "$T/result70.bundle" "$T/result69.ckir" "$T/result69.elf" \
  --result 69 > "$T/source70-artifacts69.rfn"
python3 "$PACKER" "$T/result69.bundle" "$T/result70.ckir" "$T/result70.elf" \
  --result 70 > "$T/source69-artifacts70.rfn"
run_all 0 251 0 "$T/source70-artifacts69.rfn" source70-artifacts69
run_all 0 251 0 "$T/source69-artifacts70.rfn" source69-artifacts70

# Hold a valid source/CKIR/result pair fixed and cross only the other valid ELF.
# The source and CKIR meanings agree; exact CKIR->ELF reconstruction alone must
# reject the pair.
python3 "$PACKER" "$T/result70.bundle" "$T/result70.ckir" "$T/result69.elf" \
  --result 70 > "$T/result70-elf69.rfn"
python3 "$PACKER" "$T/result69.bundle" "$T/result69.ckir" "$T/result70.elf" \
  --result 69 > "$T/result69-elf70.rfn"
run_all 0 0 251 "$T/result70-elf69.rfn" result70-elf69
run_all 0 0 251 "$T/result69-elf70.rfn" result69-elf70

# The low byte of 326 is still 70. Full source and CKIR meaning must reject it;
# an exit-byte-only join is insufficient.
python3 "$PACKER" "$T/result70.bundle" "$T/result70.ckir" "$T/result70.elf" \
  --result 326 > "$T/same-exit-wrong-full-result.rfn"
run_all 251 251 251 "$T/same-exit-wrong-full-result.rfn" same-exit-wrong-full-result

# The source evaluator is mechanically isolated from CKIR bytes/accessors and
# artifact runtime/layout caches. Only its driver may invoke source derivation.
python3 - "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-result.beta" <<'PY'
from pathlib import Path
import sys

text = Path(sys.argv[1]).read_text(encoding="utf-8")
body = text[text.index("proc src_refinement_source_result_check()") : text.index("proc main()")]
for forbidden in ("ckir_", "refinement_ckir_byte", "525000", "525232", "6000000", "6200000", "8800000", "9400000"):
    if forbidden in body:
        raise SystemExit(f"source evaluator reads forbidden artifact symbol/range: {forbidden}")
for anchor in ("word[10000232]", "src_lower_op", "src_lower_term", "src_lower_value_type", "src_lower_place_type", "value%256"):
    if anchor not in text:
        raise SystemExit(f"missing source-result anchor: {anchor}")
PY

ELAPSED=$(($(date +%s) - STARTED))
echo "checked-IR refinement source result: independent finite source result, full-width claim, valid source/artifact and CKIR/ELF cross-pairs passed below Delta (${ELAPSED}s)"
