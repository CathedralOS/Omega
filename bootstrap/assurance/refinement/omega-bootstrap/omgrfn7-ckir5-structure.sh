#!/usr/bin/env sh
# Fresh-frame OMGRFN7 responsibility-5 complete CKIR5 structure gate.
set -eu
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P); ROOT=$HERE
while [ ! -f "$ROOT/bootstrap/paths.sh" ]; do ROOT=$(dirname "$ROOT"); done
export OMEGA_REPO_ROOT=$ROOT
. "$ROOT/bootstrap/paths.sh"; . "$OMEGA_PATH_BETA/artifact_env.sh"; . "$OMEGA_PATH_ALPHA/seed_env.sh"; cd "$ROOT"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGRFN7 R5 structure: skipped (requires Darwin arm64)"; exit 0;; esac
R=$HERE; G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES; CASES=$R/omgrfn5_r5_cases.py; T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
observe(){ L=$1 I=$2 O=$3 E=$4 N=$5; shift 5; python3 -B "$CASES" observe "$L" "$I" "$O" "$E" "$T/timings.tsv" "$N" -- "$@"; }
run_case(){ RC_I=$1 RC_E=$2 RC_N=$3; observe 45 "$RC_I" "$T/$RC_N.native.out" "$RC_E" "$RC_N-native" "$T/native"; observe 45 "$RC_I" "$T/$RC_N.self.out" "$RC_E" "$RC_N-self" "$T/self"; cmp "$T/$RC_N.native.out" "$T/$RC_N.self.out" >/dev/null; [ ! -s "$T/$RC_N.native.out" ]; }
cp "$R/omgrfn7-component-envelope-r5.beta" "$T/check.beta"; cat "$R/ckir5-refinement-artifact.beta" >> "$T/check.beta"
PROCEDURES=$(awk '/^proc /{n++}END{print n+0}' "$T/check.beta")
MAX_LOCALS=$(python3 - "$T/check.beta" <<'PY'
import re,sys
s=open(sys.argv[1],encoding='ascii').read();m=0
for p in re.finditer(r'^proc\s+\w+\(([^)]*)\)\s*\{',s,re.M):
 e=s.find('\nproc ',p.end());b=s[p.end():e if e>=0 else len(s)];m=max(m,sum(bool(x.strip()) for x in p.group(1).split(','))+len(re.findall(r'\blet\s+[A-Za-z_]\w*',b)))
print(m)
PY
)
[ "$PROCEDURES" -le 128 ] && [ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN7 R5 structure: resource shape $PROCEDURES/$MAX_LOCALS" >&2; exit 1; }
SEED=$OMEGA_PATH_ALPHA/$ALPHA_SEED; ASM=$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED
stamp_beta_compiler "$T/bc0" >/dev/null
observe 90 "$OMEGA_PATH_BETA/bc.beta" "$T/bc1.asm" 0 beta-self-source "$T/bc0"; observe 60 "$T/bc1.asm" "$T/bc1.tape" 0 beta-self-assemble "$ASM"; stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
observe 90 "$T/check.beta" "$T/native.asm" 0 beta-build-native "$T/bc0"; observe 90 "$T/check.beta" "$T/self.asm" 0 beta-build-self "$T/bc1"; cmp "$T/native.asm" "$T/self.asm"
observe 60 "$T/native.asm" "$T/native.tape" 0 beta-assemble-native "$ASM"; observe 60 "$T/self.asm" "$T/self.tape" 0 beta-assemble-self "$ASM"; cmp "$T/native.tape" "$T/self.tape"
TAPE_BYTES=$(wc -c < "$T/native.tape"|tr -d ' '); [ "$TAPE_BYTES" -le 262140 ]; stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1; stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1
observe 20 - - 0 fixture-emit python3 -B "$G/delta-checked-ir-v5-backend-fixture.py" emit "$T/cases"
printf x > "$T/omg"; printf 'OMGRSW3\000\003\000\000\000' > "$T/wit"; printf x > "$T/elf"
observe 10 - "$T/frame" 0 exact-pack python3 -B "$R/omgrfn7_bundle.py" "$T/omg" "$T/wit" "$T/cases/canonical.ckir5" "$T/elf" --result 70
for M in source-opaque witness-opaque result-opaque elf-byte version6 bad-tag cases4097; do observe 10 - - 0 mutate-$M python3 -B "$R/omgrfn7_r5_cases.py" "$M" "$T/frame" "$T/$M"; done
run_case "$T/frame" 0 canonical-structure; run_case "$T/source-opaque" 0 source-body-opacity; run_case "$T/witness-opaque" 0 witness-identity-opacity; run_case "$T/result-opaque" 0 result-claim-opacity; run_case "$T/elf-byte" 0 elf-opacity
run_case "$T/version6" 251 v7-envelope-only; run_case "$T/bad-tag" 251 owned-ckir-mutation; run_case "$T/cases4097" 252 aggregate-case-resource
if [ -n "${OMEGA_OMGRFN7_R5_STRUCTURE_EXPORT:-}" ]; then cp "$T/check.beta" "$OMEGA_OMGRFN7_R5_STRUCTURE_EXPORT"; fi
awk -F '\t' '{t+=$1;if($1>m){m=$1;s=$2}}END{printf "OMGRFN7 R5 structure timings: command-sum=%.3fs commands=%d slowest=%s:%.3fs\n",t,NR,s,m}' "$T/timings.tsv"
echo "OMGRFN7 R5 structure: native/self complete CKIR5, opacity, mutation/resource passed; tape=${TAPE_BYTES}B procedures=$PROCEDURES max-locals=$MAX_LOCALS"
