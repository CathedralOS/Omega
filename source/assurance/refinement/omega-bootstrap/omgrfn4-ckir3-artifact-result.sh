#!/usr/bin/env sh
# Complete OMGRFN4 responsibility-5 CKIR3/result checker gate.
set -eu

GATE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$GATE_DIR
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do
  PARENT=$(dirname -- "$OMEGA_REPO_ROOT")
  [ "$PARENT" != "$OMEGA_REPO_ROOT" ] || exit 2
  OMEGA_REPO_ROOT=$PARENT
done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"

case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN4 responsibility 5 result: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN4 responsibility 5 result: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
ENVELOPE=$R/omgrfn4-component-envelope.beta
STRUCTURE=$R/ckir3-refinement-artifact.beta
RESULT=$R/ckir3-refinement-result.beta
CASES=$R/omgrfn4_r5_cases.py
R3_CASES=$R/omgrfn4_r3_cases.py
PACKER=$R/omgrfn4_bundle.py
BUILDER=$G/delta-resolved-to-ckir3-fixture.py
LOW_FRAME=$G/delta-resolved-to-ckir3-frame.py
FIXTURE=$G/fixtures/ckir3-constant-aggregates/renamed-reordered-nested.omg
for FILE in "$ENVELOPE" "$STRUCTURE" "$RESULT" "$CASES" "$R3_CASES" "$PACKER" "$BUILDER" "$LOW_FRAME" "$FIXTURE"; do
  [ -f "$FILE" ] || { echo "OMGRFN4 responsibility 5 result: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
observe() { # timeout input output expected label command...
  LIMIT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5; shift 5
  python3 -B "$CASES" observe "$LIMIT" "$INPUT" "$OUTPUT" "$EXPECTED" "$T/timings.tsv" "$LABEL" -- "$@"
}
run_case() { INPUT=$1 EXPECTED=$2 LABEL=$3; observe 30 "$INPUT" "$T/$LABEL.out" "$EXPECTED" "$LABEL" "$T/check"; }

stamp_beta_compiler "$T/bc" >/dev/null
cat "$ENVELOPE" > "$T/check.beta"
sed '/^proc main()/,$d' "$STRUCTURE" >> "$T/check.beta"
cat "$RESULT" >> "$T/check.beta"
PROCEDURES=$(awk '/^proc / { n += 1 } END { print n + 0 }' "$T/check.beta")
MAX_LOCALS=$(python3 - "$T/check.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="ascii").read(); m=0
for p in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
 e=s.find("\nproc ",p.end()); b=s[p.end():e if e>=0 else len(s)]
 m=max(m,sum(bool(x.strip()) for x in p.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",b)))
print(m)
PY
)
[ "$PROCEDURES" -le 128 ] && [ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN4 responsibility 5 result: compiler resource shape $PROCEDURES/$MAX_LOCALS" >&2; exit 1; }
observe 90 "$T/check.beta" "$T/check.asm" 0 beta-build "$T/bc"
observe 90 "$T/check.asm" "$T/check.tape" 0 alpha-assemble "$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
TAPE_BYTES=$(wc -c < "$T/check.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN4 responsibility 5 result: tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/check.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$T/check" >/dev/null 2>&1

observe 120 - - 0 cargo-build cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
observe 60 - - 0 compile-resolver env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver"
observe 60 - - 0 compile-lowerer env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir3.alp" "$T/lowerer"
observe 20 - - 0 compact-builder python3 -B "$BUILDER" build "$T/compact.omgc" AggregateProbe run "$FIXTURE"
observe 30 "$T/compact.omgc" "$T/compact.witness" 0 compact-resolver "$T/resolver"
observe 10 - "$T/compact.low3" 0 compact-frame python3 -B "$LOW_FRAME" pack "$T/compact.omgc" "$T/compact.witness"
observe 30 "$T/compact.low3" "$T/compact.ckir3" 0 compact-lowerer "$T/lowerer"
printf opaque-result-layer-elf > "$T/opaque.elf"
observe 10 - "$T/compact.rfn" 0 compact-pack python3 -B "$PACKER" "$T/compact.omgc" "$T/compact.witness" "$T/compact.ckir3" "$T/opaque.elf" --result 70
run_case "$T/compact.rfn" 0 compact-result

python3 -B "$R3_CASES" cases "$T/compact.rfn" "$T/r3-cases"
for NAME in count-framing dense-id empty-span-offset reserved scalar-range scalar-type-arity structural-arity child-back-edge child-type-layout height-order key-order duplicate-key type-layout-join ckir2-inner-version opaque-opcode11-root opaque-result; do
  run_case "$T/r3-cases/$NAME.rfn" 251 "$NAME"
done
for NAME in constant-count-resource child-count-resource declared-ckir-resource; do run_case "$T/r3-cases/$NAME.rfn" 252 "$NAME"; done
run_case "$T/r3-cases/opaque-source-constant.rfn" 0 source-opaque-valid-ckir
run_case "$T/r3-cases/opaque-elf.rfn" 0 elf-opacity

python3 -B "$CASES" phase-cases "$T/compact.rfn" "$T/r5-cases"
for NAME in unreachable-node root-id root-imm1 root-operand root-result-shape setbe-opcode wrong-result; do run_case "$T/r5-cases/$NAME.rfn" 251 "$NAME"; done

python3 -B "$CASES" evaluator-cases "$T/evaluator"
printf opaque > "$T/opaque.omgc"; printf opaque > "$T/opaque.witness"
for SPEC in frames-64:0 frames-65:252 entries-65536:0 entries-65537:252; do
  NAME=${SPEC%:*}; EXPECTED=${SPEC#*:}
  python3 -B "$PACKER" "$T/opaque.omgc" "$T/opaque.witness" "$T/evaluator/$NAME.ckir3" "$T/opaque.elf" --result 70 > "$T/$NAME.rfn"
  run_case "$T/$NAME.rfn" "$EXPECTED" "$NAME"
done

python3 -B "$CASES" report "$T/timings.tsv"
echo "OMGRFN4 responsibility 5 result: complete CKIR3, typed roots/reachability, result 70, 64/65 frames, 65536/65537 entries, phase mutations, opacity, and 0/251/252 passed ($PROCEDURES/128 procedures; $MAX_LOCALS/32 locals; $TAPE_BYTES/262140 tape bytes)"
