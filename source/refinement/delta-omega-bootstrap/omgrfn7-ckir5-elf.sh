#!/usr/bin/env sh
# Fresh-frame OMGRFN7 responsibility-5 exact CKIR5 -> ELF gate.
set -eu

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
ROOT=$HERE
while [ ! -f "$ROOT/tools/bootstrap/paths.sh" ]; do ROOT=$(dirname "$ROOT"); done
export OMEGA_REPO_ROOT=$ROOT
. "$ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$ROOT"
case "$(uname -sm)" in
  "Darwin arm64") ;;
  *) echo "OMGRFN7 R5 ELF: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN7 R5 ELF: skipped ($TOOL absent)"; exit 0; }; done

R=$HERE
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
CASES=$R/omgrfn5_r5_cases.py
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
observe() { LIMIT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5; shift 5; python3 -B "$CASES" observe "$LIMIT" "$INPUT" "$OUTPUT" "$EXPECTED" "$T/timings.tsv" "$LABEL" -- "$@"; }
run_case() { RC_INPUT=$1 RC_EXPECTED=$2 RC_LABEL=$3; observe 45 "$RC_INPUT" "$T/$RC_LABEL.native.out" "$RC_EXPECTED" "$RC_LABEL-native" "$T/native"; observe 45 "$RC_INPUT" "$T/$RC_LABEL.self.out" "$RC_EXPECTED" "$RC_LABEL-self" "$T/self"; cmp "$T/$RC_LABEL.native.out" "$T/$RC_LABEL.self.out" >/dev/null; [ ! -s "$T/$RC_LABEL.native.out" ]; }

# The structure conjunct owns whole-CKIR validity. This fresh-frame checker
# independently rechecks the declaration/layout and operation facts consumed
# while reconstructing every selected executable byte.
sed '/^proc ckir_constant_key_after/,$d' "$R/ckir5-refinement-artifact.beta" > "$T/core"
sed -n '/^proc ckir_value_type/,/^proc ckir_initialize_call_graph/{ /^proc ckir_initialize_call_graph/,$d; p; }' "$R/ckir5-refinement-artifact.beta" > "$T/value-types"
sed -n '/^proc ckir5_preserve_tables()/,/^}/p' "$R/ckir5-refinement-artifact.beta" > "$T/preserve"
sed '/^proc main()/,$d' "$R/ckir5-refinement-elf.beta" > "$T/elf"
cp "$R/omgrfn7-component-envelope-r5.beta" "$T/check.beta"
cat "$T/core" "$T/value-types" "$T/preserve" "$T/elf" >> "$T/check.beta"
cat >> "$T/check.beta" <<'EOF'
proc main(){let s=omgrfn5_component_read() state a {to z when(s!=0) s=ckir_decode_header() to z when(s!=0) to bad when(ckir_count(7)!=0) s=ckir_validate_types_records() to z when(s!=0) s=ckir_validate_machines_blocks() to z when(s!=0) s=elf_assign_operation_types() to z when(s!=0) s=ckir5_preserve_tables() to z when(s!=0) s=ckir5_refinement_elf_check() to z} state bad{return 251} state z{return s}}
EOF

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
[ "$PROCEDURES" -le 128 ] && [ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN7 R5 ELF: checker resource shape $PROCEDURES/$MAX_LOCALS" >&2; exit 1; }

SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED
ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
observe 90 "$OMEGA_PATH_BETA/bc.beta" "$T/bc1.asm" 0 beta-self-source "$T/bc0"
observe 60 "$T/bc1.asm" "$T/bc1.tape" 0 beta-self-assemble "$ASM"
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
observe 90 "$T/check.beta" "$T/native.asm" 0 beta-build-native "$T/bc0"
observe 90 "$T/check.beta" "$T/self.asm" 0 beta-build-self "$T/bc1"
cmp "$T/native.asm" "$T/self.asm" >/dev/null
observe 60 "$T/native.asm" "$T/native.tape" 0 beta-assemble-native "$ASM"
observe 60 "$T/self.asm" "$T/self.tape" 0 beta-assemble-self "$ASM"
cmp "$T/native.tape" "$T/self.tape" >/dev/null
N=$(wc -c < "$T/native.tape" | tr -d ' ')
[ "$N" -le 262140 ] || { echo "OMGRFN7 R5 ELF: tape $N" >&2; exit 1; }
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1

observe 120 - - 0 cargo-build cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
observe 90 - - 0 compile-exact-backend env DELTA_ARCH=aarch64 "$DELTA" "$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-checked-ir-v5-to-elf.alp" "$T/backend"
observe 20 - - 0 fixture-emit python3 -B "$G/delta-checked-ir-v5-backend-fixture.py" emit "$T/cases"
observe 30 "$T/cases/canonical.ckir5" "$T/canonical.elf" 0 exact-backend "$T/backend"
printf x > "$T/omg"
printf 'OMGRSW3\000\003\000\000\000' > "$T/wit"
observe 10 - "$T/frame" 0 exact-pack python3 -B "$R/omgrfn7_bundle.py" "$T/omg" "$T/wit" "$T/cases/canonical.ckir5" "$T/canonical.elf" --result 70

for M in source-opaque witness-opaque result-opaque version6 elf-byte truncated trailing cases4097 ckir-constant; do observe 10 - - 0 mutate-$M python3 -B "$R/omgrfn7_r5_cases.py" "$M" "$T/frame" "$T/$M"; done
observe 10 - - 0 extract-cross-ckir python3 -B "$R/omgrfn7_r5_cases.py" extract-ckir "$T/ckir-constant" "$T/cross.ckir5"
observe 30 "$T/cross.ckir5" "$T/cross.elf" 0 cross-backend "$T/backend"
observe 10 - "$T/cross-paired" 0 cross-pack python3 -B "$R/omgrfn7_bundle.py" "$T/omg" "$T/wit" "$T/cross.ckir5" "$T/cross.elf" --result 70

run_case "$T/frame" 0 exact-canonical-elf
run_case "$T/source-opaque" 0 source-body-opacity
run_case "$T/witness-opaque" 0 witness-identity-opacity
run_case "$T/result-opaque" 0 result-claim-opacity
run_case "$T/cross-paired" 0 distinct-ckir-elf-pair
run_case "$T/ckir-constant" 251 ckir-elf-cross-pair
run_case "$T/version6" 251 v7-envelope-only
run_case "$T/elf-byte" 251 wrong-elf-byte
run_case "$T/truncated" 251 truncated-frame
run_case "$T/trailing" 251 trailing-frame
run_case "$T/cases4097" 252 aggregate-case-resource

if [ -n "${OMEGA_OMGRFN7_R5_ELF_EXPORT:-}" ]; then cp "$T/check.beta" "$OMEGA_OMGRFN7_R5_ELF_EXPORT"; fi
awk -F '\t' '{ total += $1; if ($1 > max) { max=$1; slow=$2 } } END { printf "OMGRFN7 R5 ELF timings: command-sum=%.3fs commands=%d slowest=%s:%.3fs\n",total,NR,slow,max }' "$T/timings.tsv"
echo "OMGRFN7 R5 ELF: native/self exact CKIR5 ELF, opacity, cross-pair, mutation/resource passed; tape=${N}B procedures=$PROCEDURES max-locals=$MAX_LOCALS"
