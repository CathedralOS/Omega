#!/usr/bin/env sh
# Complete OMGRFN5 responsibility-5 CKIR4 -> exact Linux x86-64 ELF gate.
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
  *) echo "OMGRFN5 responsibility 5 ELF: skipped (requires Darwin arm64)"; exit 0 ;;
esac
for TOOL in cargo python3 clang codesign; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN5 responsibility 5 ELF: skipped ($TOOL absent)"; exit 0; }; done

R=$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT
G=$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES
C=$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER
ENVELOPE=$R/omgrfn5-component-envelope.beta
STRUCTURE=$R/ckir4-refinement-artifact.beta
ELF_CHECKER=$R/ckir4-refinement-elf.beta
CASES=$R/omgrfn5_r5_cases.py
PACKER=$R/omgrfn5_bundle.py
PACKER6=$R/omgrfn6_bundle.py
FIXTURE_TOOL=$G/delta-checked-ir-v4-fixture.py
BUILDER=$G/delta-resolved-to-ckir4-fixture.py
LOW_FRAME=$G/delta-resolved-to-ckir4-frame.py
IR_REFERENCE=$G/checked_ir_v4_reference.py
ELF_REFERENCE=$G/checked_elf_v4_reference.py
V3_RESOURCES=$G/checked_ir_v3_resources.py
SOURCE=$OMEGA_REPO_ROOT/source/compiler/omega/psi/source/source.omg
HARNESS=$G/fixtures/ckir4-runtime-records/source-unit-harness.omg
for FILE in "$ENVELOPE" "$STRUCTURE" "$ELF_CHECKER" "$CASES" "$PACKER" "$PACKER6" "$FIXTURE_TOOL" "$BUILDER" "$LOW_FRAME" "$IR_REFERENCE" "$ELF_REFERENCE" "$V3_RESOURCES" "$SOURCE" "$HARNESS"; do
  [ -f "$FILE" ] || { echo "OMGRFN5 responsibility 5 ELF: missing $FILE" >&2; exit 1; }
done

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
: > "$T/timings.tsv"
observe() { LIMIT=$1 INPUT=$2 OUTPUT=$3 EXPECTED=$4 LABEL=$5; shift 5; python3 -B "$CASES" observe "$LIMIT" "$INPUT" "$OUTPUT" "$EXPECTED" "$T/timings.tsv" "$LABEL" -- "$@"; }
run_both() { RB_INPUT=$1 RB_EXPECTED=$2 RB_LABEL=$3; observe 90 "$RB_INPUT" "$T/$RB_LABEL.native.out" "$RB_EXPECTED" "$RB_LABEL-native" "$T/native"; observe 90 "$RB_INPUT" "$T/$RB_LABEL.self.out" "$RB_EXPECTED" "$RB_LABEL-self" "$T/self"; cmp "$T/$RB_LABEL.native.out" "$T/$RB_LABEL.self.out" >/dev/null; }

sed '/^proc main()/,$d' "$STRUCTURE" > "$T/structure-prefix.beta"
cp "$ENVELOPE" "$T/check.beta"
cat "$T/structure-prefix.beta" "$ELF_CHECKER" >> "$T/check.beta"
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
[ "$PROCEDURES" -le 128 ] && [ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN5 responsibility 5 ELF: checker resource shape $PROCEDURES/$MAX_LOCALS" >&2; exit 1; }

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
[ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN5 responsibility 5 ELF: tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1
stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1

observe 120 - - 0 cargo-build cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA=$OMEGA_PATH_DELTA_RUST/target/debug/delta
observe 60 - - 0 compile-resolver env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolve.alp" "$T/resolver"
observe 90 - - 0 compile-lowerer env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-resolved-to-ckir4.alp" "$T/lowerer"
observe 90 - - 0 compile-backend env DELTA_ARCH=aarch64 "$DELTA" "$C/omega-bootstrap-checked-ir-v4-to-elf.alp" "$T/backend"
observe 30 - - 0 exact-builder python3 -B "$BUILDER" build "$T/exact.omgc" SourceUnit bootstrap_runtime_record_probe "$SOURCE" "$HARNESS"
observe 45 "$T/exact.omgc" "$T/exact.witness" 0 exact-resolver "$T/resolver"
observe 10 - "$T/exact.low4" 0 exact-frame python3 -B "$LOW_FRAME" pack "$T/exact.omgc" "$T/exact.witness"
observe 60 "$T/exact.low4" "$T/exact.ckir4" 0 exact-lowerer "$T/lowerer"
observe 90 "$T/exact.ckir4" "$T/exact.elf" 0 exact-backend "$T/backend"
observe 30 - - 0 exact-reference python3 -B "$ELF_REFERENCE" check "$T/exact.ckir4" "$T/exact.elf"
observe 10 - "$T/exact.rfn" 0 exact-pack python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/exact.elf" --result 70
run_both "$T/exact.rfn" 0 exact-source-elf

# R5 owns the CKIR4/result/ELF claim under either exact outer carrier, while
# witness schema identity remains opaque here and is paired only by R3.
python3 - "$T/exact.witness" "$T/exact.witness2" <<'PY'
from pathlib import Path
import struct, sys
raw = bytearray(Path(sys.argv[1]).read_bytes())
raw[6] = ord("2"); struct.pack_into("<I", raw, 8, 2)
Path(sys.argv[2]).write_bytes(raw)
PY
observe 10 - "$T/exact6.rfn" 0 exact6-pack python3 -B "$PACKER6" \
  "$T/exact.omgc" "$T/exact.witness2" "$T/exact.ckir4" "$T/exact.elf" --result 70
python3 - "$T/exact6.rfn" "$T/v6-witness-identity-opaque-to-r5.rfn" \
  "$T/v6-magic5-version6.rfn" "$T/v6-magic6-version5.rfn" <<'PY'
from pathlib import Path
import struct, sys
canonical = Path(sys.argv[1]).read_bytes()
omgcomp_length = struct.unpack_from("<I", canonical, 16)[0]
witness_at = 40 + omgcomp_length
raw = bytearray(canonical); raw[witness_at + 6] = ord("X")
Path(sys.argv[2]).write_bytes(raw)
raw = bytearray(canonical); raw[6] = ord("5")
Path(sys.argv[3]).write_bytes(raw)
raw = bytearray(canonical); struct.pack_into("<I", raw, 8, 5)
Path(sys.argv[4]).write_bytes(raw)
PY
run_both "$T/exact6.rfn" 0 exact-omgrfn6-source-elf
run_both "$T/v6-witness-identity-opaque-to-r5.rfn" 0 \
  omgrfn6-witness-identity-opaque-to-r5-elf
run_both "$T/v6-magic5-version6.rfn" 251 omgrfn5-magic-version6-elf
run_both "$T/v6-magic6-version5.rfn" 251 omgrfn6-magic-version5-elf

observe 20 - - 0 fixture-emit python3 -B "$FIXTURE_TOOL" emit "$T/fixtures"
for NAME in canonical empty; do
  observe 90 "$T/fixtures/$NAME.ckir4" "$T/$NAME.elf" 0 "$NAME-backend" "$T/backend"
  observe 30 - - 0 "$NAME-reference" python3 -B "$ELF_REFERENCE" check "$T/fixtures/$NAME.ckir4" "$T/$NAME.elf"
  python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/fixtures/$NAME.ckir4" "$T/$NAME.elf" --result 70 > "$T/$NAME.rfn"
  run_both "$T/$NAME.rfn" 0 "$NAME-valid-mismatched-source"
done
observe 20 - - 0 canonical-byte-contract python3 -B "$FIXTURE_TOOL" check-artifact "$T/fixtures/canonical.ckir4" "$T/canonical.elf"
observe 20 - - 0 empty-byte-contract python3 -B "$FIXTURE_TOOL" check-empty-artifact "$T/fixtures/empty.ckir4" "$T/empty.elf"

python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir4" "$T/canonical.elf" --result 70 > "$T/exact-canonical.rfn"
python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/fixtures/canonical.ckir4" "$T/exact.elf" --result 70 > "$T/canonical-exact.rfn"
run_both "$T/exact-canonical.rfn" 251 exact-ckir-canonical-elf
run_both "$T/canonical-exact.rfn" 251 canonical-ckir-exact-elf
python3 -B "$CASES" elf-cases "$T/canonical.rfn" "$T/elf-cases"
for FILE in "$T/elf-cases"/*.rfn; do NAME=$(basename "$FILE" .rfn); run_both "$FILE" 251 "$NAME"; done

python3 -B "$CASES" constructor-resources "$T/object-resources"
python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/object-resources/constructor-frame-next.ckir4" "$T/canonical.elf" --result 70 > "$T/constructor-frame-next.rfn"
run_both "$T/constructor-frame-next.rfn" 252 constructor-object-frame-next

observe 120 - - 0 inherited-resources python3 -B "$V3_RESOURCES" "$T/v3-resources"
python3 - "$T/v3-resources" "$T/v4-resources" <<'PY'
from pathlib import Path
import struct,sys
source=Path(sys.argv[1]); target=Path(sys.argv[2]); target.mkdir()
for path in source.glob('*.ckir3'):
 raw=bytearray(path.read_bytes()); struct.pack_into('<H',raw,8,4)
 target.joinpath(path.stem+'.ckir4').write_bytes(raw)
PY
for NAME in image-exact frame-greatest elf-exact; do
  observe 120 "$T/v4-resources/$NAME.ckir4" "$T/$NAME.elf" 0 "$NAME-backend" "$T/backend"
  observe 120 - - 0 "$NAME-reference" python3 -B "$ELF_REFERENCE" check "$T/v4-resources/$NAME.ckir4" "$T/$NAME.elf"
  RESULT_VALUE=$(python3 -B "$IR_REFERENCE" run "$T/v4-resources/$NAME.ckir4" | tail -1)
  python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/v4-resources/$NAME.ckir4" "$T/$NAME.elf" --result "$RESULT_VALUE" > "$T/$NAME.rfn"
  run_both "$T/$NAME.rfn" 0 "$NAME-exact-check"
done
for NAME in image-over frame-next text-over; do
  python3 -B "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/v4-resources/$NAME.ckir4" "$T/canonical.elf" --result 70 > "$T/$NAME.rfn"
  run_both "$T/$NAME.rfn" 252 "$NAME"
done

python3 - "$T/exact.rfn" "$T/version4.rfn" <<'PY'
from pathlib import Path
import struct,sys
raw=bytearray(Path(sys.argv[1]).read_bytes()); raw[6]=ord('4'); struct.pack_into('<I',raw,8,4); Path(sys.argv[2]).write_bytes(raw)
PY
run_both "$T/version4.rfn" 251 frozen-v4-carrier-rejected

python3 -B "$CASES" report "$T/timings.tsv"
echo "OMGRFN5/6 responsibility 5 ELF: exact v5/v6 outer dispatch with witness identity opaque to R5, producer CKIR4, distinct aligned objects, empty/nested templates, selected frame/live stack/text/ELF exact-adjacent resources, valid source-mismatched pairs, CKIR/ELF cross-pair rejection, V4/V5/V6 separation, native/self persisted Beta, and 0/251/252 passed ($PROCEDURES/128 procedures; $MAX_LOCALS/32 locals; $TAPE_BYTES/262140 tape bytes)"
