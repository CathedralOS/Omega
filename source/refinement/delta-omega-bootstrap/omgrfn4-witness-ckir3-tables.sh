#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN4 responsibility-3 declaration and intrinsic
# CKIR3 constant-table refinement. This is not a whole-CKIR meaning gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN4 responsibility 3: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN4 responsibility 3: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-witness-ckir3-tables.beta"
CASES="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_r3_cases.py"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_bundle.py"
OLD_PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn3_bundle.py"
BUILDER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-fixture.py"
LOW_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir3-frame.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir3.alp"
UNICODE="$OMEGA_REPO_ROOT/source/psi/generated/unicode_tables.omg"
FIXTURES="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir3-constant-aggregates"
HARNESS="$FIXTURES/unicode-harness.omg"
COMPACT="$FIXTURES/renamed-reordered-nested.omg"
for REQUIRED in "$CHECKER" "$CASES" "$PACKER" "$OLD_PACKER" "$BUILDER" \
  "$LOW_FRAME" "$RESOLVER" "$LOWERER" "$UNICODE" "$HARNESS" "$COMPACT"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN4 responsibility 3: missing $REQUIRED" >&2
    exit 1
  }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN4 responsibility 3: exceeds 128 procedures ($PROCEDURES)" >&2
  exit 1
}
MAX_LOCALS=$(python3 - "$CHECKER" <<'PY'
import re, sys
source = open(sys.argv[1], encoding="utf-8").read()
maximum = 0
for match in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{", source, re.M):
    end = source.find("\nproc ", match.end())
    body = source[match.end():end if end >= 0 else len(source)]
    params = sum(bool(item.strip()) for item in match.group(1).split(","))
    maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", body)))
print(maximum)
PY
)
[ "$MAX_LOCALS" -le 32 ] || {
  echo "OMGRFN4 responsibility 3: exceeds 32 local slots ($MAX_LOCALS)" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
STARTED=$(date +%s)

observe() { # timeout input output expected label command...
  TIMEOUT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5
  shift 5
  python3 -B "$CASES" observe "$TIMEOUT" "$INPUT" "$OUTPUT" "$EXPECTED" \
    "$T/timings.tsv" "$LABEL" -- "$@"
}

echo "OMGRFN4 responsibility 3: START bounded checker and CKIR3 producers"
BC="$T/bc"
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$BC" >/dev/null
observe 60 "$CHECKER" "$T/check.asm" 0 beta-build "$BC"
observe 60 "$T/check.asm" "$T/check.tape" 0 beta-assemble "$ASM"
TAPE_BYTES=$(wc -c < "$T/check.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || {
  echo "OMGRFN4 responsibility 3: tape exceeds 262140 bytes ($TAPE_BYTES)" >&2
  exit 1
}
stamp_seed "$T/check.tape" "$SEED" "$T/check" >/dev/null 2>&1

observe 120 - - 0 cargo-build cargo build -q --manifest-path \
  "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
export DELTA_ARCH=aarch64
observe 60 - - 0 compile-resolver "$DELTA" "$RESOLVER" "$T/resolver"
observe 60 - - 0 compile-lowerer "$DELTA" "$LOWERER" "$T/lowerer"

build_pair() {
  NAME=$1 OWNER=$2 MACHINE=$3
  shift 3
  observe 20 - - 0 "$NAME-builder" python3 -B "$BUILDER" build \
    "$T/$NAME.omgc" "$OWNER" "$MACHINE" "$@"
  observe 30 "$T/$NAME.omgc" "$T/$NAME.witness" 0 "$NAME-resolver" "$T/resolver"
  observe 10 - "$T/$NAME.low3" 0 "$NAME-frame" python3 -B "$LOW_FRAME" pack \
    "$T/$NAME.omgc" "$T/$NAME.witness"
  observe 30 "$T/$NAME.low3" "$T/$NAME.ckir3" 0 "$NAME-lowerer" "$T/lowerer"
  printf 'opaque-responsibility-3-elf' > "$T/$NAME.elf"
  observe 10 - "$T/$NAME.rfn" 0 "$NAME-pack" python3 -B "$PACKER" \
    "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir3" "$T/$NAME.elf" --result 70
}

build_pair unicode UnicodeTables bootstrap_constant_aggregate_probe "$UNICODE" "$HARNESS"
build_pair compact AggregateProbe run "$COMPACT"
python3 -B "$CASES" summary "$T/unicode.rfn" 2740 3537
python3 -B "$CASES" summary "$T/compact.rfn" 28 28

run_expect() {
  INPUT=$1 EXPECTED=$2 LABEL=$3
  observe 30 "$INPUT" "$T/$LABEL.out" "$EXPECTED" "$LABEL" "$T/check"
  [ ! -s "$T/$LABEL.out" ] || {
    echo "OMGRFN4 responsibility 3: $LABEL published stdout" >&2
    exit 1
  }
}

run_expect "$T/unicode.rfn" 0 unicode-check
run_expect "$T/compact.rfn" 0 compact-check

# Independently valid products cannot cross-pair at the declaration/type join.
observe 10 - "$T/cross-unicode-compact.rfn" 0 cross-a-pack python3 -B "$PACKER" \
  "$T/unicode.omgc" "$T/unicode.witness" "$T/compact.ckir3" "$T/unicode.elf" --result 70
observe 10 - "$T/cross-compact-unicode.rfn" 0 cross-b-pack python3 -B "$PACKER" \
  "$T/compact.omgc" "$T/compact.witness" "$T/unicode.ckir3" "$T/compact.elf" --result 70
run_expect "$T/cross-unicode-compact.rfn" 251 cross-unicode-compact
run_expect "$T/cross-compact-unicode.rfn" 251 cross-compact-unicode

python3 -B "$CASES" cases "$T/compact.rfn" "$T/cases"
for NAME in count-framing dense-id empty-span-offset reserved scalar-range \
  scalar-type-arity structural-arity child-back-edge child-type-layout \
  height-order key-order duplicate-key type-layout-join ckir2-inner-version; do
  run_expect "$T/cases/$NAME.rfn" 251 "$NAME"
done
for NAME in constant-count-resource child-count-resource declared-ckir-resource; do
  run_expect "$T/cases/$NAME.rfn" 252 "$NAME"
done

# These are deliberately outside responsibility 3. The scalar mutation stays
# intrinsically canonical but disagrees with source-body meaning; opcode-11
# root/reachability, result execution, and ELF/image bytes remain opaque.
for NAME in opaque-source-constant opaque-opcode11-root opaque-result opaque-elf; do
  run_expect "$T/cases/$NAME.rfn" 0 "$NAME"
done

observe 10 - "$T/old.rfn" 0 old-pack python3 -B "$OLD_PACKER" \
  "$T/compact.omgc" "$T/compact.witness" "$T/compact.ckir3" "$T/compact.elf" --result 70
run_expect "$T/old.rfn" 251 omgrfn3-cross-version

ELAPSED=$(($(date +%s) - STARTED))
python3 -B "$CASES" report "$T/timings.tsv"
echo "OMGRFN4 responsibility 3: Unicode 2740/3537 and compact 28/28 constant tables, declarations/layout/selected-entry-root joins, cross-pairs, intrinsic DAG controls, 0/251/252, and V3 rejection passed (${ELAPSED}s; ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape bytes)"
echo "OMGRFN4 responsibility 3 unowned: source-body constants, opcode-11 roots/reachability, execution/result, constant image, and ELF"
