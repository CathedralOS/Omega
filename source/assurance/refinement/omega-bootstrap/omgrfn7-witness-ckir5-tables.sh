#!/usr/bin/env sh
# Focused lower-rooted OMGRFN7 responsibility-3 OMGRSW3 -> CKIR5 gate.
set -eu

STARTED=$(date +%s)
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
OMEGA_REPO_ROOT=$HERE
while [ ! -f "$OMEGA_REPO_ROOT/bootstrap/paths.sh" ]; do OMEGA_REPO_ROOT=$(dirname -- "$OMEGA_REPO_ROOT"); done
export OMEGA_REPO_ROOT
. "$OMEGA_REPO_ROOT/bootstrap/paths.sh"
. "$OMEGA_PATH_BETA/artifact_env.sh"
. "$OMEGA_PATH_ALPHA/seed_env.sh"
cd "$OMEGA_REPO_ROOT"
case "$(uname -sm)" in "Darwin arm64") ;; *) echo "OMGRFN7 responsibility 3: skipped (requires Darwin arm64)"; exit 0;; esac
for TOOL in cargo python3; do command -v "$TOOL" >/dev/null 2>&1 || { echo "OMGRFN7 responsibility 3: skipped ($TOOL absent)"; exit 0; }; done

R="$OMEGA_PATH_OMEGA_BOOTSTRAP_REFINEMENT"
CHECKER="$R/omgrfn7-witness-ckir5-tables.beta"
PACKER="$R/omgrfn7_bundle.py"
RESOLVER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolve.alp"
LOWERER="$OMEGA_PATH_OMEGA_BOOTSTRAP_COMPILER/omega-bootstrap-resolved-to-ckir4.alp"
FIXTURE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/delta-resolved-to-ckir5-fixture.py"
SOURCE="$OMEGA_PATH_OMEGA_BOOTSTRAP_GATES/fixtures/ckir5-payload-sums/general.omg"
for FILE in "$CHECKER" "$PACKER" "$RESOLVER" "$LOWERER" "$FIXTURE" "$SOURCE" "$OMEGA_PATH_BETA/bc.beta"; do
  [ -f "$FILE" ] || { echo "OMGRFN7 responsibility 3: missing $FILE" >&2; exit 1; }
done
T=$(mktemp -d)
if [ "${OMEGA_KEEP_OMGRFN7_R3_TEMP:-0}" = 1 ]; then echo "OMGRFN7 responsibility 3: retained $T" >&2; else trap 'rm -rf "$T"' EXIT; fi
SEED="$OMEGA_PATH_ALPHA/$ALPHA_SEED"; ASM="$OMEGA_PATH_ALPHA_ASSEMBLER/$BETA_SEED"
PROCEDURES=$(awk '/^proc / { n += 1 } END { print n + 0 }' "$CHECKER")
[ "$PROCEDURES" -le 128 ] || { echo "OMGRFN7 R3 procedures $PROCEDURES" >&2; exit 1; }
MAX_LOCALS=$(python3 - "$CHECKER" <<'PY'
import re,sys
s=open(sys.argv[1],encoding="utf-8").read(); mmax=0
for m in re.finditer(r"^proc\s+\w+\(([^)]*)\)\s*\{",s,re.M):
 e=s.find("\nproc ",m.end()); b=s[m.end():e if e>=0 else len(s)]
 mmax=max(mmax,sum(bool(x.strip()) for x in m.group(1).split(","))+len(re.findall(r"\blet\s+[A-Za-z_]\w*",b)))
print(mmax)
PY
)
[ "$MAX_LOCALS" -le 32 ] || { echo "OMGRFN7 R3 locals $MAX_LOCALS" >&2; exit 1; }

stamp_beta_compiler "$T/bc0" >/dev/null
"$T/bc0" < "$OMEGA_PATH_BETA/bc.beta" > "$T/bc1.asm"; "$ASM" < "$T/bc1.asm" > "$T/bc1.tape"
BC1_TAPE=$(wc -c < "$T/bc1.tape" | tr -d ' '); [ $((BC1_TAPE+4)) -le "$HOLE_SIZE" ] || exit 1
stamp_seed "$T/bc1.tape" "$SEED" "$T/bc1" >/dev/null 2>&1
"$T/bc0" < "$CHECKER" > "$T/native.asm"; "$T/bc1" < "$CHECKER" > "$T/self.asm"; cmp "$T/native.asm" "$T/self.asm" >/dev/null
"$ASM" < "$T/native.asm" > "$T/native.tape"; "$ASM" < "$T/self.asm" > "$T/self.tape"; cmp "$T/native.tape" "$T/self.tape" >/dev/null
TAPE_BYTES=$(wc -c < "$T/native.tape" | tr -d ' '); [ "$TAPE_BYTES" -le 262140 ] || { echo "OMGRFN7 R3 tape $TAPE_BYTES" >&2; exit 1; }
stamp_seed "$T/native.tape" "$SEED" "$T/native" >/dev/null 2>&1; stamp_seed "$T/self.tape" "$SEED" "$T/self" >/dev/null 2>&1

cargo build -q --manifest-path "$OMEGA_PATH_DELTA_RUST/Cargo.toml"
DELTA="$OMEGA_PATH_DELTA_RUST/target/debug/delta"
DELTA_ARCH=aarch64 "$DELTA" "$RESOLVER" "$T/resolver" >/dev/null
DELTA_ARCH=aarch64 "$DELTA" "$LOWERER" "$T/lowerer" >/dev/null
python3 -B - "$FIXTURE" "$SOURCE" "$T/exact.omgc" <<'PY'
import importlib.util,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("f",sys.argv[1]); f=importlib.util.module_from_spec(spec); spec.loader.exec_module(f)
Path(sys.argv[3]).write_bytes(f.encode_source(Path(sys.argv[2]).read_text(encoding="ascii")))
PY
"$T/resolver" < "$T/exact.omgc" > "$T/exact.witness"
python3 -B - "$FIXTURE" "$T/exact.omgc" "$T/exact.witness" "$T/lower.frame" <<'PY'
import importlib.util,sys
from pathlib import Path
spec=importlib.util.spec_from_file_location("f",sys.argv[1]); f=importlib.util.module_from_spec(spec); spec.loader.exec_module(f)
Path(sys.argv[4]).write_bytes(f.pack_lowering(Path(sys.argv[2]).read_bytes(),Path(sys.argv[3]).read_bytes()))
PY
"$T/lowerer" < "$T/lower.frame" > "$T/exact.ckir5"
python3 - "$T/elf" <<'PY'
from pathlib import Path
import sys
Path(sys.argv[1]).write_bytes(b"opaque ELF")
PY
python3 "$PACKER" "$T/exact.omgc" "$T/exact.witness" "$T/exact.ckir5" "$T/elf" --result 70 > "$T/exact.rfn"

observe_one() { EXE=$1 EXPECTED=$2 FRAME=$3 LABEL=$4; set +e; "$EXE" < "$FRAME" > "$T/out" 2> "$T/err"; ACTUAL=$?; set -e; [ "$ACTUAL" -eq "$EXPECTED" ] || { echo "OMGRFN7 R3: $LABEL got $ACTUAL expected $EXPECTED" >&2; sed -n '1,10p' "$T/err" >&2; exit 1; }; [ ! -s "$T/out" ] || { echo "OMGRFN7 R3: $LABEL published stdout" >&2; exit 1; }; }
observe() { observe_one "$T/native" "$1" "$2" "$3 native"; observe_one "$T/self" "$1" "$2" "$3 self"; }
observe 0 "$T/exact.rfn" "canonical produced OMGRSW3/CKIR5 join"

python3 - "$T/exact.rfn" "$T/witness.rfn" "$T/ckir.rfn" "$T/source.rfn" "$T/elf.rfn" "$T/result.rfn" "$T/v6.rfn" "$T/w-case.rfn" "$T/w-payload.rfn" "$T/c-case.rfn" "$T/c-payload.rfn" "$T/c-op14.rfn" "$T/c-arm.rfn" <<'PY'
from pathlib import Path
import struct,sys
r=bytearray(Path(sys.argv[1]).read_bytes()); _,_,_,oc,ow,ck,el,_,_=struct.unpack_from("<8s8I",r); w=40+oc; c=w+ow; e=c+ck
x=bytearray(r); x[w+64]^=1; Path(sys.argv[2]).write_bytes(x)
x=bytearray(r); x[c+36]^=1; Path(sys.argv[3]).write_bytes(x)
x=bytearray(r); x[40+oc-1]^=1; Path(sys.argv[4]).write_bytes(x)
x=bytearray(r); x[e]^=1; Path(sys.argv[5]).write_bytes(x)
x=bytearray(r); struct.pack_into("<II",x,32,71,71); Path(sys.argv[6]).write_bytes(x)
x=bytearray(r); x[6]=ord("6"); struct.pack_into("<I",x,8,6); Path(sys.argv[7]).write_bytes(x)
wc=struct.unpack_from("<17I",r,w+16); wa=w+84
widths=(36,48,28,28,24,24,24,24,28,24,40,24,40,24)
counts=(wc[1],wc[2],wc[3],wc[4],wc[5],wc[6],wc[7],wc[12],wc[13],wc[14],wc[8],wc[9],wc[10],wc[11])
wo=[]
for count,width in zip(counts,widths): wo.append(wa); wa+=count*width
x=bytearray(r); struct.pack_into("<I",x,wo[8]+4,1); Path(sys.argv[8]).write_bytes(x)
x=bytearray(r); struct.pack_into("<I",x,wo[9]+12,0); Path(sys.argv[9]).write_bytes(x)
cc=struct.unpack_from("<19I",r,c+24); ca=c+100
cwidths=(24,20,16,20,20,16,36,20,32,20,24,4,40,4,52,24,12)
co=[]
for count,width in zip(cc[:17],cwidths): co.append(ca); ca+=count*width
x=bytearray(r); struct.pack_into("<I",x,co[4]+8,1); Path(sys.argv[10]).write_bytes(x)
x=bytearray(r); struct.pack_into("<I",x,co[5]+12,0); Path(sys.argv[11]).write_bytes(x)
op14=next(i for i in range(cc[12]) if r[co[12]+i*40+12]==14)
x=bytearray(r); struct.pack_into("<I",x,co[12]+op14*40+36,1); Path(sys.argv[12]).write_bytes(x)
x=bytearray(r); struct.pack_into("<I",x,co[15]+8,1); Path(sys.argv[13]).write_bytes(x)
PY
observe 251 "$T/witness.rfn" "OMGRSW3 sum-count mutation"
observe 251 "$T/ckir.rfn" "CKIR5 sum-count mutation"
observe 251 "$T/w-case.rfn" "OMGRSW3 case-owner mutation"
observe 251 "$T/w-payload.rfn" "OMGRSW3 payload-type mutation"
observe 251 "$T/c-case.rfn" "CKIR5 case-ordinal mutation"
observe 251 "$T/c-payload.rfn" "CKIR5 payload-type mutation"
observe 251 "$T/c-op14.rfn" "CKIR5 ConstructCase reserved mutation"
observe 251 "$T/c-arm.rfn" "CKIR5 CaseDispatch arm-case mutation"
observe 0 "$T/source.rfn" "OMGCOMP source opaque"
observe 0 "$T/elf.rfn" "ELF opaque"
observe 0 "$T/result.rfn" "claimed result opaque"
observe 251 "$T/v6.rfn" "OMGRFN6 outer cross-pair"

ELAPSED=$(( $(date +%s)-STARTED ))
echo "OMGRFN7 responsibility 3: exact OMGRSW3/CKIR5 sums/cases/payloads, nominal types, copy/layout, opcode-14 and CaseDispatch arm/selected-payload envelopes, cross-pairs and phase opacity passed native/self"
echo "OMGRFN7 responsibility 3 resources: ${PROCEDURES}/128 procedures; ${MAX_LOCALS}/32 locals; ${TAPE_BYTES}/262140 tape; ${BC1_TAPE}+4/${HOLE_SIZE} self-Beta; elapsed=${ELAPSED}s"
