#!/usr/bin/env sh
# Complete OMGRFN4 responsibility-5 CKIR3 -> exact Linux x86-64 ELF gate.
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
  *) echo "OMGRFN4 responsibility 5 ELF: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN4 responsibility 5 ELF: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
ENVELOPE=$R/omgrfn4-component-envelope.beta
STRUCTURE=$R/ckir3-refinement-artifact.beta
ELF_CHECKER=$R/ckir3-refinement-elf.beta
CASES=$R/omgrfn4_r5_cases.py
R3_CASES=$R/omgrfn4_r3_cases.py
PACKER=$R/omgrfn4_bundle.py
BUILDER=$G/delta-resolved-to-ckir3-fixture.py
LOW_FRAME=$G/delta-resolved-to-ckir3-frame.py
IR_REFERENCE=$G/checked_ir_v3_reference.py
ELF_REFERENCE=$G/checked_elf_v3_reference.py
RESOURCES=$G/checked_ir_v3_resources.py
FIXTURES=$G/fixtures/ckir3-constant-aggregates
UNICODE=$OMEGA_REPO_ROOT/source/psi/generated/unicode_tables.omg
for FILE in "$ENVELOPE" "$STRUCTURE" "$ELF_CHECKER" "$CASES" "$R3_CASES" "$PACKER" "$BUILDER" "$LOW_FRAME" "$IR_REFERENCE" "$ELF_REFERENCE" "$RESOURCES" "$FIXTURES/renamed-reordered-nested.omg" "$FIXTURES/unicode-harness.omg" "$UNICODE"; do
  [ -f "$FILE" ] || { echo "OMGRFN4 responsibility 5 ELF: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
observe() { LIMIT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5; shift 5; python3 -B "$CASES" observe "$LIMIT" "$INPUT" "$OUTPUT" "$EXPECTED" "$T/timings.tsv" "$LABEL" -- "$@"; }
run_case() { INPUT=$1 EXPECTED=$2 LABEL=$3; observe 90 "$INPUT" "$T/$LABEL.out" "$EXPECTED" "$LABEL" "$T/check"; }

stamp_beta_compiler "$T/bc" >/dev/null
cat "$ENVELOPE" > "$T/check.beta"
sed '/^proc main()/,$d' "$STRUCTURE" >> "$T/check.beta"
cat "$ELF_CHECKER" >> "$T/check.beta"
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
[ "$PROCEDURES" -le 128 ] && [ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN4 responsibility 5 ELF: compiler resource shape $PROCEDURES/$MAX_LOCALS" >&2; exit 1; }
observe 90 "$T/check.beta" "$T/check.asm" 0 beta-build "$T/bc"
observe 90 "$T/check.asm" "$T/check.tape" 0 alpha-assemble "$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
TAPE_BYTES=$(wc -c < "$T/check.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN4 responsibility 5 ELF: tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/check.tape" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$T/check" >/dev/null 2>&1

# Every private region used by the two independently repeated R5 executables is
# explicit, non-overlapping, and below Alpha's 64-MiB memory ceiling.
python3 - <<'PY'
regions=[(1048576,5546120,"frame"),(6000000,6328192,"layouts"),(6400000,8794912,"scope"),
 (8800000,9800000,"evaluator"),(10500000,10630000,"calls"),(10800000,11662144,"ELF"),
 (13000000,13065536,"const-type"),(13070000,13135536,"const-start"),
 (13140000,13205536,"const-count"),(13210000,13275536,"const-scalar"),
 (13280000,13345536,"const-height"),(13350000,13358192,"roots"),
 (13360000,13368192,"reachable"),(13400000,13465536,"image-offset"),
 (13500000,13631072,"children"),(13700000,13831072,"image")]
for left,right in zip(sorted(regions),sorted(regions)[1:]): assert left[1] <= right[0],(left,right)
assert max(end for _,end,_ in regions) <= 0x04000000
PY

observe 120 - - 0 cargo-build cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
observe 60 - - 0 compile-resolver env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver"
observe 60 - - 0 compile-lowerer env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir3.alp" "$T/lowerer"
observe 60 - - 0 compile-backend env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v3-to-elf.alp" "$T/backend"

build_source() { # name owner machine source...
  NAME=$1 OWNER=$2 MACHINE=$3; shift 3
  observe 30 - - 0 "$NAME-builder" python3 -B "$BUILDER" build "$T/$NAME.omgc" "$OWNER" "$MACHINE" "$@"
  observe 30 "$T/$NAME.omgc" "$T/$NAME.witness" 0 "$NAME-resolver" "$T/resolver"
  observe 10 - "$T/$NAME.low3" 0 "$NAME-frame" python3 -B "$LOW_FRAME" pack "$T/$NAME.omgc" "$T/$NAME.witness"
  observe 30 "$T/$NAME.low3" "$T/$NAME.ckir3" 0 "$NAME-lowerer" "$T/lowerer"
  observe 60 "$T/$NAME.ckir3" "$T/$NAME.elf" 0 "$NAME-backend" "$T/backend"
  observe 60 - - 0 "$NAME-reference" python3 -B "$ELF_REFERENCE" check "$T/$NAME.ckir3" "$T/$NAME.elf"
  observe 10 - "$T/$NAME.rfn" 0 "$NAME-pack" python3 -B "$PACKER" "$T/$NAME.omgc" "$T/$NAME.witness" "$T/$NAME.ckir3" "$T/$NAME.elf" --result 70
  run_case "$T/$NAME.rfn" 0 "$NAME-exact"
}
build_source compact AggregateProbe run "$FIXTURES/renamed-reordered-nested.omg"
build_source unicode UnicodeTables bootstrap_constant_aggregate_probe "$UNICODE" "$FIXTURES/unicode-harness.omg"

python3 -B "$PACKER" "$T/compact.omgc" "$T/compact.witness" "$T/compact.ckir3" "$T/unicode.elf" --result 70 > "$T/compact-unicode.rfn"
python3 -B "$PACKER" "$T/unicode.omgc" "$T/unicode.witness" "$T/unicode.ckir3" "$T/compact.elf" --result 70 > "$T/unicode-compact.rfn"
run_case "$T/compact-unicode.rfn" 251 compact-ckir-unicode-elf
run_case "$T/unicode-compact.rfn" 251 unicode-ckir-compact-elf

python3 -B "$CASES" elf-cases "$T/compact.rfn" "$T/elf-cases"
for NAME in elf-header segment-field image-byte rip-constant-displacement setbe-byte truncated trailing; do run_case "$T/elf-cases/$NAME.rfn" 251 "$NAME"; done
python3 -B "$R3_CASES" cases "$T/compact.rfn" "$T/r3-cases"
run_case "$T/r3-cases/opaque-result.rfn" 0 result-opacity
run_case "$T/r3-cases/constant-count-resource.rfn" 252 constant-count-resource

# The same exact checker covers the inherited two-segment no-constant path.
python3 -B "$CASES" evaluator-cases "$T/evaluator"
printf opaque > "$T/opaque.omgc"; printf opaque > "$T/opaque.witness"
observe 60 "$T/evaluator/frames-64.ckir3" "$T/two.elf" 0 two-segment-backend "$T/backend"
python3 -B "$PACKER" "$T/opaque.omgc" "$T/opaque.witness" "$T/evaluator/frames-64.ckir3" "$T/two.elf" --result 70 > "$T/two.rfn"
run_case "$T/two.rfn" 0 two-segment-exact

# Genuine canonical backend boundaries pin image, selected frame, text, and
# simultaneous maximal ELF sizing in the lower-rooted checker itself.
observe 120 - - 0 generate-resources python3 -B "$RESOURCES" "$T/resources"
for NAME in image-exact frame-greatest elf-exact; do
  observe 120 "$T/resources/$NAME.ckir3" "$T/$NAME.elf" 0 "$NAME-backend" "$T/backend"
  observe 120 - - 0 "$NAME-reference" python3 -B "$ELF_REFERENCE" check "$T/resources/$NAME.ckir3" "$T/$NAME.elf"
  RESULT_VALUE=$(python3 -B "$IR_REFERENCE" run "$T/resources/$NAME.ckir3" | tail -1)
  python3 -B "$PACKER" "$T/opaque.omgc" "$T/opaque.witness" "$T/resources/$NAME.ckir3" "$T/$NAME.elf" --result "$RESULT_VALUE" > "$T/$NAME.rfn"
  run_case "$T/$NAME.rfn" 0 "$NAME-exact-check"
done
for NAME in image-over frame-next text-over; do
  python3 -B "$PACKER" "$T/opaque.omgc" "$T/opaque.witness" "$T/resources/$NAME.ckir3" "$T/compact.elf" --result 70 > "$T/$NAME.rfn"
  run_case "$T/$NAME.rfn" 252 "$NAME"
done

python3 -B "$CASES" report "$T/timings.tsv"
echo "OMGRFN4 responsibility 5 ELF: exact two/three-segment reconstruction, constant image/root offsets, closure/ABI/displacements/stack, cross-pairs, byte mutations, exact/adjacent resources, and 0/251/252 passed ($PROCEDURES/128 procedures; $MAX_LOCALS/32 locals; $TAPE_BYTES/262140 tape bytes)"
