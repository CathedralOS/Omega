#!/usr/bin/env sh
# Persisted-Beta source declaration/signature/layout -> canonical CKIR1 join.
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
  *) echo "checked-IR refinement source tables: skipped (fixture producer requires Darwin arm64)"; exit 0 ;;
esac

for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "checked-IR refinement source tables: skipped ($TOOL absent)"
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
  > "$T/check.beta"
sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-input.beta" \
  >> "$T/check.beta"
sed '/^proc main()/,$d' \
  "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-artifact.beta" \
  >> "$T/check.beta"
cat "$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/ckir-refinement-source-tables.beta" \
  >> "$T/check.beta"
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
peek_comment_at = fixture.index("// Shared-receiver")
reordered = (
    fixture[pair_at:run_at]
    + fixture[:pair_at]
    + fixture[peek_comment_at:]
    + fixture[run_at:peek_comment_at]
)
out.joinpath("renamed-reordered.omg").write_text(rename(reordered), encoding="utf-8")

copy_owner = fixture.replace("data Probe {", "data Probe [copy] {", 1)
if copy_owner == fixture:
    raise SystemExit("copy-owner mutation did not apply")
out.joinpath("copy-owner.omg").write_text(copy_owner, encoding="utf-8")
PY

observe() { # expected input label
  expected=$1
  input=$2
  label=$3
  set +e
  "$T/check" < "$input" > "$T/stdout" 2> "$T/stderr"
  actual=$?
  set -e
  [ "$actual" = "$expected" ] || {
    echo "checked-IR refinement source tables: $label returned $actual, expected $expected" >&2
    tail -c 4096 "$T/stderr" >&2 || true
    exit 1
  }
  [ ! -s "$T/stdout" ] || {
    echo "checked-IR refinement source tables: $label published stdout" >&2
    exit 1
  }
}

build_entry() { # label source
  label=$1
  source=$2
  python3 "$BUNDLER" pack "$label.omg=$source" > "$T/$label.bundle"
  "$T/producer" < "$T/$label.bundle" > "$T/$label.ckir"
  "$T/backend" < "$T/$label.ckir" > "$T/$label.elf"
  python3 "$PACKER" "$T/$label.bundle" "$T/$label.ckir" "$T/$label.elf" \
    --result 70 > "$T/$label.rfn"
  observe 0 "$T/$label.rfn" "$label"
}

build_entry fixture "$T/sources/fixture.omg"
build_entry renamed "$T/sources/renamed.omg"
build_entry renamed-reordered "$T/sources/renamed-reordered.omg"
build_entry copy-owner "$T/sources/copy-owner.omg"

# Names erase from CKIR when declaration and semantic order is unchanged.
cmp "$T/fixture.ckir" "$T/renamed.ckir"
python3 "$PACKER" "$T/renamed.bundle" "$T/fixture.ckir" "$T/fixture.elf" \
  --result 70 > "$T/renamed-original-artifacts.rfn"
observe 0 "$T/renamed-original-artifacts.rfn" renamed-name-erasure

: > "$T/empty"
python3 "$BUNDLER" pack "source.omg=$T/sources/product.omg" > "$T/product.bundle"
"$T/producer" < "$T/product.bundle" > "$T/product.ckir"
python3 "$PACKER" "$T/product.bundle" "$T/product.ckir" "$T/empty" \
  --library > "$T/product.rfn"
observe 0 "$T/product.rfn" product-library

# Both triples are independently valid. Cross-pairing them must fail only at
# the source declaration/signature/layout -> canonical CKIR relation.
python3 "$PACKER" "$T/fixture.bundle" "$T/copy-owner.ckir" "$T/copy-owner.elf" \
  --result 70 > "$T/original-source-copy-ckir.rfn"
python3 "$PACKER" "$T/copy-owner.bundle" "$T/fixture.ckir" "$T/fixture.elf" \
  --result 70 > "$T/copy-source-original-ckir.rfn"
observe 251 "$T/original-source-copy-ckir.rfn" valid-ckir-source-mismatch
observe 251 "$T/copy-source-original-ckir.rfn" valid-source-ckir-mismatch

# Declaration order participates in canonical IDs even though the reordered
# program remains accepted and has the same selected observation.
python3 "$PACKER" "$T/fixture.bundle" "$T/renamed-reordered.ckir" \
  "$T/renamed-reordered.elf" --result 70 > "$T/reordered-cross-pair.rfn"
observe 251 "$T/reordered-cross-pair.rfn" declaration-order-mismatch

ELAPSED=$(($(date +%s) - STARTED))
echo "checked-IR refinement source tables: declarations, nominal/copy/layout/type interning, machine/state signatures, name erasure, and valid cross-pair negatives passed below Delta (${ELAPSED}s)"
