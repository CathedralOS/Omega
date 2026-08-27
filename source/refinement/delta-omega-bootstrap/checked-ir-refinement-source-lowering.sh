#!/usr/bin/env sh
# Persisted-Beta source body -> canonical CKIR1 lowering join.
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
  *) echo "checked-IR refinement source lowering: skipped (fixture producer requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 perl; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR refinement source lowering: skipped ($TOOL absent)"
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
PRODUCT_SOURCE="$OMEGA_REPO_ROOT/source/psi/source/source.omg"

stamp_beta_compiler "$BC" >/dev/null
sed '/^proc main()/,$d' "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-envelope.beta" > "$T/artifact.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-artifact.beta" >> "$T/artifact.beta"
"$BC" < "$T/artifact.beta" > "$T/artifact.asm"
"$ASM" < "$T/artifact.asm" > "$T/artifact.tape"
stamp_seed "$T/artifact.tape" "$SEED" "$T/artifact-check" >/dev/null 2>&1

for FRAGMENT in ckir-refinement-envelope.beta ckir-refinement-source-input.beta ckir-refinement-source-tables.beta; do
  sed '/^proc main()/,$d' "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/$FRAGMENT" >> "$T/check.beta"
done
# Artifact layout is checked by the first persisted-Beta conjunct.  Remove only
# its three reconstructed-layout cache joins from this second executable; the
# source-derived layout still determines and compares all CKIR type/field rows.
perl -pi -e 's/ckir_count\((\d+)\)/word[525016+$1*8]/g; s/  to bad when \(word\[6000000\+id\*8\] != src_record\(i,5\)\)//; s/  to bad when \(word\[6100000\+id\*8\] != src_record\(i,6\)\)//; s/  to bad when \(word\[6200000\+i\*8\] != src_field\(i,4\)\)//' "$T/check.beta"
perl -pi -e '
  s/ckir_type_word\(([^,()]+),([^()]+)\)/ckir_row_word(0,$1,24,$2)/g;
  s/ckir_type_byte\(([^,()]+),([^()]+)\)/ckir_row_byte(0,$1,24,$2)/g;
  s/ckir_record_word\(([^,()]+),([^()]+)\)/ckir_row_word(1,$1,20,$2)/g;
  s/ckir_record_byte\(([^,()]+),([^()]+)\)/ckir_row_byte(1,$1,20,$2)/g;
  s/ckir_field_word\(([^,()]+),([^()]+)\)/ckir_row_word(2,$1,16,$2)/g;
  s/ckir_machine_word\(([^,()]+),([^()]+)\)/ckir_row_word(3,$1,36,$2)/g;
  s/ckir_machine_byte\(([^,()]+),([^()]+)\)/ckir_row_byte(3,$1,36,$2)/g;
  s/ckir_mparam_word\(([^,()]+),([^()]+)\)/ckir_row_word(4,$1,20,$2)/g;
  s/ckir_block_word\(([^,()]+),([^()]+)\)/ckir_row_word(5,$1,32,$2)/g;
  s/ckir_block_byte\(([^,()]+),([^()]+)\)/ckir_row_byte(5,$1,32,$2)/g;
  s/ckir_bparam_word\(([^,()]+),([^()]+)\)/ckir_row_word(6,$1,20,$2)/g;
' "$T/check.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-lowering.beta" >> "$T/check.beta"
PROC_COUNT=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$T/check.beta")
[ "$PROC_COUNT" -le 128 ] || {
  echo "checked-IR refinement source lowering: composed source checker has $PROC_COUNT procedures (Beta ceiling 128)" >&2
  exit 1
}
"$BC" < "$T/check.beta" > "$T/check.asm"
"$ASM" < "$T/check.asm" > "$T/check.tape"
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$PRODUCER_SOURCE" "$T/producer" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$BACKEND_SOURCE" "$T/backend" >/dev/null

mkdir "$T/sources"
cp "$FIXTURE" "$T/sources/fixture.omg"
cp "$PRODUCT_SOURCE" "$T/sources/product.omg"
python3 - "$FIXTURE" "$T/sources" <<'PY'
from pathlib import Path
import re
import sys

fixture = Path(sys.argv[1]).read_text(encoding="utf-8")
out = Path(sys.argv[2])

renames = {
    "Probe": "Vault", "Pair": "Duo", "run": "execute", "peek": "inspect",
    "before": "prefix", "source": "origin", "copy": "replica",
    "bytes": "cells", "length": "used", "index": "cursor",
    "after": "suffix", "retained": "kept", "first": "left", "second": "right",
    "present": "available", "fail": "rejected",
}
def rename(source: str) -> str:
    source = source.replace("[copy]", "[__copy_capability__]")
    for old, new in renames.items():
        source = re.sub(rf"\b{re.escape(old)}\b", new, source)
    return source.replace("[__copy_capability__]", "[copy]")

out.joinpath("renamed.omg").write_text(rename(fixture), encoding="utf-8")
pair_at = fixture.index("data Pair [copy]")
run_at = fixture.index("machine Probe::run")
peek_at = fixture.index("// Shared-receiver")
reordered = fixture[pair_at:run_at] + fixture[:pair_at] + fixture[peek_at:] + fixture[run_at:peek_at]
out.joinpath("renamed-reordered.omg").write_text(rename(reordered), encoding="utf-8")

mutations = {
    # Addition is commutative at runtime, but its canonical operand/load order is not.
    "operand-swap": ("self.length = self.length + 1;", "self.length = 1 + self.length;"),
    # The selected execution never reaches this constant.
    "nonselected-const": ("state fail(&mut self) {\n        71", "state fail(&mut self) {\n        72"),
    # This machine is not emitted at all by the selected-entry backend.
    "unselected-machine-const": ("state absent(&self) {\n        0", "state absent(&self) {\n        1"),
    # Both fields contain 23 after the earlier checked structural copy.
    "field": ("transition self.copy.second < 24", "transition self.source.second < 24"),
    # This first false edge is not selected for the closed fixture execution.
    "branch": ("_ -> fail()\n    }\n\n    state store_0", "_ -> before_high()\n    }\n\n    state store_0"),
}
for label, (old, new) in mutations.items():
    changed = fixture.replace(old, new, 1)
    if changed == fixture:
        raise SystemExit(f"{label} mutation did not apply")
    out.joinpath(f"{label}.omg").write_text(changed, encoding="utf-8")

fail_result = "state fail(&mut self) {\n        71"
statements = "state fail(&mut self) {\n" + "        self.before = 0;\n" * 32 + "        71"
out.joinpath("resource-statements.omg").write_text(
    fixture.replace(fail_result, statements, 1), encoding="utf-8"
)
tree = "state fail(&mut self) {\n        " + " + ".join(["1"] * 9)
out.joinpath("resource-tree.omg").write_text(
    fixture.replace(fail_result, tree, 1), encoding="utf-8"
)
PY

observe() { # expected input label
  expected=$1 input=$2 label=$3
  set +e
  "$T/artifact-check" < "$input" > "$T/artifact-stdout" 2> "$T/artifact-stderr"
  artifact_status=$?
  set -e
  [ "$artifact_status" = 0 ] || {
    echo "checked-IR refinement source lowering: artifact conjunct for $label returned $artifact_status, expected 0" >&2
    tail -c 4096 "$T/artifact-stderr" >&2 || true
    exit 1
  }
  [ ! -s "$T/artifact-stdout" ] || {
    echo "checked-IR refinement source lowering: artifact conjunct for $label published stdout" >&2
    exit 1
  }
  set +e
  "$T/check" < "$input" > "$T/stdout" 2> "$T/stderr"
  actual=$?
  set -e
  [ "$actual" = "$expected" ] || {
    echo "checked-IR refinement source lowering: $label returned $actual, expected $expected" >&2
    tail -c 4096 "$T/stderr" >&2 || true
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "checked-IR refinement source lowering: $label published stdout" >&2
    exit 1
  }
}

build_entry() { # label source
  label=$1 source=$2
  python3 "$BUNDLER" pack "$label.omg=$source" > "$T/$label.bundle"
  "$T/producer" < "$T/$label.bundle" > "$T/$label.ckir"
  "$T/backend" < "$T/$label.ckir" > "$T/$label.elf"
  python3 "$PACKER" "$T/$label.bundle" "$T/$label.ckir" "$T/$label.elf" --result 70 > "$T/$label.rfn"
  observe 0 "$T/$label.rfn" "$label"
}

build_entry fixture "$T/sources/fixture.omg"
build_entry renamed "$T/sources/renamed.omg"
build_entry renamed-reordered "$T/sources/renamed-reordered.omg"

cmp "$T/fixture.ckir" "$T/renamed.ckir"
python3 "$PACKER" "$T/renamed.bundle" "$T/fixture.ckir" "$T/fixture.elf" --result 70 > "$T/renamed-original-artifacts.rfn"
observe 0 "$T/renamed-original-artifacts.rfn" renamed-name-erasure

: > "$T/empty"
python3 "$BUNDLER" pack "source.omg=$T/sources/product.omg" > "$T/product.bundle"
"$T/producer" < "$T/product.bundle" > "$T/product.ckir"
python3 "$PACKER" "$T/product.bundle" "$T/product.ckir" "$T/empty" --library > "$T/product.rfn"
observe 0 "$T/product.rfn" product-library

for label in operand-swap nonselected-const unselected-machine-const field branch; do
  build_entry "$label" "$T/sources/$label.omg"
  # Both sides are valid and retain the selected result.  Only the independent
  # source-body -> CKIR join can reject the cross-pair.
  python3 "$PACKER" "$T/fixture.bundle" "$T/$label.ckir" "$T/$label.elf" --result 70 > "$T/$label-cross.rfn"
  observe 251 "$T/$label-cross.rfn" "$label-valid-cross-pair"
done

# The backend emits only the selected machine, so this source change has
# identical ELF and result bytes. Rejection proves every body is related.
cmp "$T/fixture.elf" "$T/unselected-machine-const.elf"

for label in resource-statements resource-tree; do
  python3 "$BUNDLER" pack "$label.omg=$T/sources/$label.omg" > "$T/$label.bundle"
  python3 "$PACKER" "$T/$label.bundle" "$T/fixture.ckir" "$T/fixture.elf" --result 70 > "$T/$label.rfn"
  observe 252 "$T/$label.rfn" "$label"
done

ELAPSED=$(($(date +%s) - STARTED))
echo "checked-IR refinement source lowering: separate persisted-Beta artifact conjunct plus exact source-derived operations, operands, terminators, value/place IDs, positives, five valid semantic cross-pairs, and 251/252 separation passed below Delta (${ELAPSED}s; ${PROC_COUNT}/128 source-checker procedures)"
