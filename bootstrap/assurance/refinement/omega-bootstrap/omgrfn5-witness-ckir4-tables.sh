#!/usr/bin/env sh
# Focused persisted-Beta OMGRFN5 responsibility-3 declaration, intrinsic
# constant-DAG, and opcode-13 nominal-envelope refinement. This is not a
# source-lowering, whole-CKIR meaning, object-layout, or ELF gate.
set -eu

STARTED=$(date +%s)
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
  *) echo "OMGRFN5 responsibility 3: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do
  command -v "$TOOL" >/dev/null 2>&1 || {
    echo "OMGRFN5 responsibility 3: skipped ($TOOL absent)"
    exit 0
  }
done

CHECKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn5-witness-ckir4-tables.beta"
CHECKER4="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4-witness-ckir3-tables.beta"
CASES="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn5_r3_cases.py"
PACKER="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn5_bundle.py"
PACKER4="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT/omgrfn4_bundle.py"
BUILDER="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir4-fixture.py"
LOW_FRAME="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir4-frame.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir4.alp"
SOURCE="$OMEGA_REPO_ROOT/compiler/psi/source/source.omg"
HARNESS="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir4-runtime-records/source-unit-harness.omg"
DIRECT="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir4-runtime-records/direct-call.omg"
COMPACT="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir3-constant-aggregates/renamed-reordered-nested.omg"
for REQUIRED in "$CHECKER" "$CHECKER4" "$CASES" "$PACKER" "$PACKER4" \
    "$BUILDER" "$LOW_FRAME" "$RESOLVER" "$LOWERER" "$SOURCE" "$HARNESS" \
    "$COMPACT" "$DIRECT" "$OMEGA_PATH_BETA/bc.beta"; do
  [ -f "$REQUIRED" ] || {
    echo "OMGRFN5 responsibility 3: missing $REQUIRED" >&2
    exit 1
  }
done

PROCEDURES=$(awk '/^proc / { count += 1 } END { print count + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || {
  echo "OMGRFN5 responsibility 3: checker procedures $PROCEDURES exceed 128" >&2
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
  echo "OMGRFN5 responsibility 3: checker locals $MAX_LOCALS exceed 32" >&2
  exit 1
}

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
observe() { # timeout input output expected label command...
  TIMEOUT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5
  shift 5
  python3 -B "$CASES" observe "$TIMEOUT" "$INPUT" "$OUTPUT" "$EXPECTED" \
    "$T/timings.tsv" "$LABEL" -- "$@"
}

SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"
ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
stamp_beta_compiler "$T/bc0" >/dev/null
observe 90 "$OMEGA_PATH_BETA/bc.beta" "$T/bc1.asm" 0 beta-self-source "$T/bc0"
observe 60 "$T/bc1.asm" "$T/bc1.tape" 0 beta-self-assemble "$ASM"
BC1_TAPE=$(wc -c < "$T/bc1.tape" | tr -d ' ')
[ $((BC1_TAPE + 4)) -le "$HOLE_SIZE" ] || {
  echo "OMGRFN5 responsibility 3: self-built Beta exceeds seed hole" >&2
  exit 1
}
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1

observe 90 "$CHECKER" "$T/native.asm" 0 beta-build-native "$T/bc0"
observe 90 "$CHECKER" "$T/self.asm" 0 beta-build-self "$T/bc1"
cmp "$T/native.asm" "$T/self.asm" >/dev/null
observe 60 "$T/native.asm" "$T/native.tape" 0 beta-assemble-native "$ASM"
observe 60 "$T/self.asm" "$T/self.tape" 0 beta-assemble-self "$ASM"
cmp "$T/native.tape" "$T/self.tape" >/dev/null
TAPE_BYTES=$(wc -c < "$T/native.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || {
  echo "OMGRFN5 responsibility 3: checker tape $TAPE_BYTES exceeds 262140" >&2
  exit 1
}
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1

observe 90 "$CHECKER4" "$T/v4.asm" 0 beta-build-v4 "$T/bc0"
observe 60 "$T/v4.asm" "$T/v4.tape" 0 beta-assemble-v4 "$ASM"
stamp_seed "$T/v4.tape" "$SEED" "$T/v4" >/dev/null 2>&1

observe 120 - - 0 cargo-build cargo build -q --manifest-path \
  "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
export DELTA_ARCH=aarch64
observe 60 - - 0 compile-resolver "$DELTA" "$RESOLVER" "$T/resolver"
observe 90 - - 0 compile-lowerer "$DELTA" "$LOWERER" "$T/lowerer"

build_pair() { # name owner machine source...
  NAME=$1 OWNER=$2 MACHINE=$3
  shift 3
  observe 20 - - 0 "$NAME-builder" python3 -B "$BUILDER" build \
    "$T/$NAME.omgc" "$OWNER" "$MACHINE" "$@"
  observe 30 "$T/$NAME.omgc" "$T/$NAME.witness" 0 "$NAME-resolver" "$T/resolver"
  observe 10 - "$T/$NAME.low4" 0 "$NAME-frame" python3 -B "$LOW_FRAME" pack \
    "$T/$NAME.omgc" "$T/$NAME.witness"
  observe 45 "$T/$NAME.low4" "$T/$NAME.ckir4" 0 "$NAME-lowerer" "$T/lowerer"
  printf 'opaque-responsibility-3-elf' > "$T/$NAME.elf"
  observe 10 - "$T/$NAME.rfn" 0 "$NAME-pack" python3 -B "$PACKER" \
    "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir4" "$T/$NAME.elf" --result 70
}

build_pair constructor DirectCallProbe run "$DIRECT"
build_pair compact AggregateProbe run "$COMPACT"
cat > "$T/five.omg" <<'OMEGA'
data FiveValue [copy] {
    f0: u8;
    f1: u8;
    f2: u8;
    f3: u8;
    f4: u8;
}
data FiveProbe { scalar: u8; }
machine FiveProbe::run(&mut self) -> u8 {
    self.scalar = 70;
    self.scalar
}
OMEGA
build_pair five FiveProbe run "$T/five.omg"

run_one() { # executable input expected label
  EXE=$1 INPUT=$2 EXPECTED=$3 LABEL=$4
  observe 30 "$INPUT" "$T/$LABEL.out" "$EXPECTED" "$LABEL" "$EXE"
  [ ! -s "$T/$LABEL.out" ] || {
    echo "OMGRFN5 responsibility 3: $LABEL published stdout" >&2
    exit 1
  }
}
run_expect() { # input expected label
  run_one "$T/native" "$1" "$2" "$3-native"
  run_one "$T/self" "$1" "$2" "$3-self"
}

build_pair exact SourceUnit bootstrap_runtime_record_probe "$SOURCE" "$HARNESS"
run_expect "$T/exact.rfn" 0 exact-check
run_expect "$T/constructor.rfn" 0 constructor-check
run_expect "$T/compact.rfn" 0 compact-check
run_expect "$T/five.rfn" 0 five-declaration-check

# Independently valid products reject at the declaration/type join.
observe 10 - "$T/cross-exact-compact.rfn" 0 cross-a-pack python3 -B "$PACKER" \
  "$T/exact.omgc" "$T/exact.witness" "$T/compact.ckir4" "$T/exact.elf" --result 70
observe 10 - "$T/cross-compact-exact.rfn" 0 cross-b-pack python3 -B "$PACKER" \
  "$T/compact.omgc" "$T/compact.witness" "$T/exact.ckir4" "$T/compact.elf" --result 70
run_expect "$T/cross-exact-compact.rfn" 251 cross-exact-compact
run_expect "$T/cross-compact-exact.rfn" 251 cross-compact-exact

python3 -B "$CASES" cases "$T/compact.rfn" "$T/cases"
for NAME in count-framing dense-id empty-span-offset reserved scalar-range \
    scalar-type-arity structural-arity child-back-edge child-type-layout \
    height-order key-order duplicate-key type-layout-join ckir3-inner-version; do
  run_expect "$T/cases/$NAME.rfn" 251 "$NAME"
done
for NAME in constant-count-resource child-count-resource declared-ckir-resource; do
  run_expect "$T/cases/$NAME.rfn" 252 "$NAME"
done
for NAME in opaque-source-constant opaque-opcode11-root opaque-result opaque-elf; do
  run_expect "$T/cases/$NAME.rfn" 0 "$NAME"
done

python3 -B "$CASES" constructor-cases "$T/exact.rfn" "$T/constructors"
for NAME in constructor-result-kind constructor-flags constructor-scalar-result constructor-noncopyable \
    constructor-arity constructor-immediate-zero constructor-immediate-one; do
  run_expect "$T/constructors/$NAME.rfn" 251 "$NAME"
done
for NAME in opaque-constructor-operand opaque-constructor-result-id; do
  run_expect "$T/constructors/$NAME.rfn" 0 "$NAME"
done

python3 -B "$CASES" constructor-five-cases "$T/five.rfn" "$T/five-cases"
run_expect "$T/five-cases/constructor-five-malformed.rfn" 251 constructor-five-malformed
run_expect "$T/five-cases/constructor-five-valid.rfn" 252 constructor-five-valid

observe 10 - "$T/version4.rfn" 0 version4-pack python3 -B "$PACKER4" \
  "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/exact.elf" --result 70
run_expect "$T/version4.rfn" 251 omgrfn4-frame-rejected-by-v5
run_one "$T/v4" "$T/exact.rfn" 251 omgrfn5-frame-rejected-by-v4

ELAPSED=$(($(date +%s) - STARTED))
python3 -B "$CASES" report "$T/timings.tsv"
echo "OMGRFN5 responsibility 3: OMGRSW1->CKIR4 declarations/layout/types/selected entry, dense tables, copyability, intrinsic constant DAG, and opcode-13 nominal envelope passed native/self"
echo "OMGRFN5 responsibility 3: cross-pairs, phase opacity, V4/V5 separation, malformed-five=251, valid-five=252 passed (${ELAPSED}s; ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape bytes)"
echo "OMGRFN5 responsibility 3 unowned: source-body lowering, constructor operand identity/order/visibility/typing, result/execution, objects/frame extents, image, and ELF"
