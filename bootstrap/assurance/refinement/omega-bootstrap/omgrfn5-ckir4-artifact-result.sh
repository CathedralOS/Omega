#!/usr/bin/env sh
# Complete OMGRFN5 responsibility-5 CKIR4/result checker gate.
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
  *) echo "OMGRFN5 responsibility 5 result: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN5 responsibility 5 result: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
ENVELOPE=$R/omgrfn5-component-envelope.beta
STRUCTURE=$R/ckir4-refinement-artifact.beta
RESULT=$R/ckir4-refinement-result.beta
CASES=$R/omgrfn5_r5_cases.py
PACKER=$R/omgrfn5_bundle.py
FIXTURE_TOOL=$G/delta-checked-ir-v4-fixture.py
BUILDER=$G/delta-resolved-to-ckir4-fixture.py
LOW_FRAME=$G/delta-resolved-to-ckir4-frame.py
SOURCE=$OMEGA_REPO_ROOT/compiler/psi/source/source.omg
HARNESS=$G/fixtures/ckir4-runtime-records/source-unit-harness.omg
V4_ENVELOPE=$R/omgrfn4-component-envelope.beta
V4_STRUCTURE=$R/ckir3-refinement-artifact.beta
V4_RESULT=$R/ckir3-refinement-result.beta
for FILE in "$ENVELOPE" "$STRUCTURE" "$RESULT" "$CASES" "$PACKER" "$FIXTURE_TOOL" "$BUILDER" "$LOW_FRAME" "$SOURCE" "$HARNESS" "$V4_ENVELOPE" "$V4_STRUCTURE" "$V4_RESULT"; do
  [ -f "$FILE" ] || { echo "OMGRFN5 responsibility 5 result: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
observe() { LIMIT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5; shift 5; python3 -B "$CASES" observe "$LIMIT" "$INPUT" "$OUTPUT" "$EXPECTED" "$T/timings.tsv" "$LABEL" -- "$@"; }
run_both() { RB_INPUT=$1 RB_EXPECTED=$2 RB_LABEL=$3; observe 45 "$RB_INPUT" "$T/$RB_LABEL.native.out" "$RB_EXPECTED" "$RB_LABEL-native" "$T/native"; observe 45 "$RB_INPUT" "$T/$RB_LABEL.self.out" "$RB_EXPECTED" "$RB_LABEL-self" "$T/self"; cmp "$T/$RB_LABEL.native.out" "$T/$RB_LABEL.self.out" >/dev/null; }

sed '/^proc main()/,$d' "$STRUCTURE" > "$T/structure-prefix.beta"
cp "$ENVELOPE" "$T/check.beta"
cat "$T/structure-prefix.beta" "$RESULT" >> "$T/check.beta"
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
[ "$PROCEDURES" -le 128 ] && [ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN5 responsibility 5 result: checker resource shape $PROCEDURES/$MAX_LOCALS" >&2; exit 1; }

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
TAPE_BYTES=$(wc -c < "$T/native.tape" | tr -d ' ')
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN5 responsibility 5 result: tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1
sed '/^proc main()/,$d' "$V4_STRUCTURE" > "$T/v4-prefix.beta"
cp "$V4_ENVELOPE" "$T/v4.beta"
cat "$T/v4-prefix.beta" "$V4_RESULT" >> "$T/v4.beta"
observe 90 "$T/v4.beta" "$T/v4.asm" 0 beta-build-frozen-v4 "$T/bc0"
observe 60 "$T/v4.asm" "$T/v4.tape" 0 beta-assemble-frozen-v4 "$ASM"
stamp_seed "$T/v4.tape" "$SEED" "$T/v4" >/dev/null 2>&1

observe 120 - - 0 cargo-build cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
observe 60 - - 0 compile-resolver env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver"
observe 90 - - 0 compile-lowerer env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer"
observe 30 - - 0 exact-builder python3 -B "$BUILDER" build "$T/exact.omgc" SourceUnit bootstrap_runtime_record_probe "$SOURCE" "$HARNESS"
observe 45 "$T/exact.omgc" "$T/exact.witness" 0 exact-resolver "$T/resolver"
observe 10 - "$T/exact.low4" 0 exact-frame python3 -B "$LOW_FRAME" pack "$T/exact.omgc" "$T/exact.witness"
observe 60 "$T/exact.low4" "$T/exact.ckir4" 0 exact-lowerer "$T/lowerer"
printf opaque-result-elf > "$T/opaque.elf"
observe 10 - "$T/exact.rfn" 0 exact-pack python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/opaque.elf" --result 70
run_both "$T/exact.rfn" 0 exact-source-result70

observe 20 - - 0 fixture-emit python3 -B "$FIXTURE_TOOL" emit "$T/fixtures"
python3 -B "$CASES" constructor-cases "$T/exact.rfn" "$T/fixtures" "$T/constructor-cases"
while IFS="$(printf '\t')" read -r NAME EXPECTED; do run_both "$T/constructor-cases/$NAME.rfn" "$EXPECTED" "$NAME"; done < "$T/fixtures/manifest.tsv"
run_both "$T/constructor-cases/canonical.rfn" 0 canonical-nested-call-copy-valid-source-mismatch
run_both "$T/constructor-cases/empty.rfn" 0 empty-object-anchor-valid-source-mismatch

python3 -B "$CASES" evaluator-cases "$T/evaluator"
for SPEC in frames-64:0 frames-65:252 entries-65536:0 entries-65537:252; do
  NAME=${SPEC%:*}; EXPECTED=${SPEC#*:}
  python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/evaluator/$NAME.ckir4" "$T/opaque.elf" --result 70 > "$T/$NAME.rfn"
  run_both "$T/$NAME.rfn" "$EXPECTED" "$NAME"
done

python3 - "$T/exact.rfn" "$T/version4.rfn" "$T/wrong-result.rfn" <<'PY'
from pathlib import Path
import struct,sys
raw=bytearray(Path(sys.argv[1]).read_bytes()); raw[6]=ord('4'); struct.pack_into('<I',raw,8,4); Path(sys.argv[2]).write_bytes(raw)
raw=bytearray(Path(sys.argv[1]).read_bytes()); struct.pack_into('<II',raw,32,71,71); Path(sys.argv[3]).write_bytes(raw)
PY
run_both "$T/version4.rfn" 251 frozen-v4-carrier-rejected
observe 45 "$T/exact.rfn" "$T/frozen-v4-rejects-v5.out" 251 frozen-v4-rejects-v5 "$T/v4"
run_both "$T/wrong-result.rfn" 251 wrong-claimed-result

python3 -B "$CASES" report "$T/timings.tsv"
echo "OMGRFN5 responsibility 5 result: exact source CKIR4/result70, immutable nested constructor objects, structural Call/Copy, direct-edge/opcode mutations, 64/65 frames, 65536/65537 entries, V4/V5 separation, native/self persisted Beta, and 0/251/252 passed ($PROCEDURES/128 procedures; $MAX_LOCALS/32 locals; $TAPE_BYTES/262140 tape bytes)"
